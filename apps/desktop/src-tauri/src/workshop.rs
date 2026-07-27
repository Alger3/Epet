use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{
    ColorType, DynamicImage, GenericImageView, ImageEncoder, ImageFormat, ImageReader, Limits,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_CLEANED_PHOTO_BYTES: usize = 8 * 1024 * 1024;
const MAX_DECODE_DIMENSION: u32 = 4_096;
const MAX_OUTPUT_DIMENSION: u32 = 1_536;
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;
const AUTHORIZATION_VERSION: &str = "human-avatar-authorization-v1";
static DRAFT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum WorkshopError {
    #[error("照片文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("工坊数据库操作失败：{0}")]
    Database(#[from] rusqlite::Error),
    #[error("照片无法解码或处理：{0}")]
    Image(#[from] image::ImageError),
    #[error("工坊操作被拒绝：{0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkshopSnapshot {
    pub drafts: Vec<CreationDraft>,
    pub generation_service_configured: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationDraft {
    pub id: String,
    pub subject_kind: String,
    pub display_name: Option<String>,
    pub authorization_confirmed: bool,
    pub authorization_version: Option<String>,
    pub status: String,
    pub snapshot_version: u64,
    pub progress_percent: Option<u8>,
    pub server_job_id: Option<String>,
    pub server_expires_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
    pub created_at: String,
    pub updated_at: String,
    pub photos: Vec<DraftPhoto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftPhoto {
    pub role: String,
    pub original_name: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
    pub sha256: String,
    pub crop_x: f64,
    pub crop_y: f64,
    pub crop_width: f64,
    pub crop_height: f64,
    pub quality_status: String,
    pub quality_messages: Vec<String>,
    pub preview_data_url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationDraftUpdate {
    pub status: String,
    pub progress_percent: Option<u8>,
    pub server_job_id: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
}

pub struct SavePhotoInput<'a> {
    pub draft_id: &'a str,
    pub role: &'a str,
    pub original_name: &'a str,
    pub encoded_bytes: &'a [u8],
    pub crop_x: f64,
    pub crop_y: f64,
    pub crop_width: f64,
    pub crop_height: f64,
}

struct ProcessedPhoto {
    bytes: Vec<u8>,
    mime_type: &'static str,
    extension: &'static str,
    width: u32,
    height: u32,
    sha256: String,
    quality_status: &'static str,
    quality_messages: Vec<String>,
}

pub fn snapshot(
    connection: &Connection,
    drafts_root: &Path,
) -> Result<WorkshopSnapshot, WorkshopError> {
    let mut statement = connection.prepare(
        "SELECT id FROM creation_drafts
         ORDER BY updated_at DESC, created_at DESC",
    )?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let drafts = ids
        .into_iter()
        .map(|id| get_draft(connection, drafts_root, &id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WorkshopSnapshot {
        drafts,
        generation_service_configured: true,
    })
}

pub fn create_draft(
    connection: &mut Connection,
    drafts_root: &Path,
    subject_kind: &str,
    display_name: &str,
    authorization_confirmed: bool,
) -> Result<CreationDraft, WorkshopError> {
    validate_subject_kind(subject_kind)?;
    let display_name = sanitize_display_name(display_name)?;
    if subject_kind == "human_avatar" && !authorization_confirmed {
        return rejected("创建人物草稿前必须确认成年人授权声明");
    }
    fs::create_dir_all(drafts_root)?;
    let draft_id = unique_draft_id();
    fs::create_dir(drafts_root.join(&draft_id))?;
    let authorization_version = (subject_kind == "human_avatar").then_some(AUTHORIZATION_VERSION);
    let result = connection.execute(
        "INSERT INTO creation_drafts (
           id, subject_kind, display_name, authorization_confirmed, authorization_version,
           status, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, 'editing', datetime('now'), datetime('now'))",
        params![
            draft_id,
            subject_kind,
            display_name,
            i64::from(authorization_confirmed),
            authorization_version
        ],
    );
    if let Err(error) = result {
        let _ = fs::remove_dir_all(drafts_root.join(&draft_id));
        return Err(error.into());
    }
    get_draft(connection, drafts_root, &draft_id)
}

pub fn save_photo(
    connection: &mut Connection,
    drafts_root: &Path,
    input: SavePhotoInput<'_>,
) -> Result<CreationDraft, WorkshopError> {
    validate_draft_id(input.draft_id)?;
    validate_photo_role(input.role)?;
    validate_crop(
        input.crop_x,
        input.crop_y,
        input.crop_width,
        input.crop_height,
    )?;
    let original_name = sanitize_original_name(input.original_name)?;
    ensure_draft_editable(connection, input.draft_id)?;
    let processed = process_cleaned_photo(input.encoded_bytes)?;

    let draft_directory = drafts_root.join(input.draft_id);
    let photos_directory = draft_directory.join("photos");
    fs::create_dir_all(&photos_directory)?;
    let final_path = photos_directory.join(format!("{}.{}", processed.sha256, processed.extension));
    let created_file = if final_path.exists() {
        false
    } else {
        let temporary_path = draft_directory.join(format!(
            ".photo-{}-{}.tmp",
            std::process::id(),
            DRAFT_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        write_new_file(&temporary_path, &processed.bytes)?;
        if let Err(error) = fs::rename(&temporary_path, &final_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error.into());
        }
        true
    };

    let previous = connection
        .query_row(
            "SELECT sha256, mime_type FROM draft_photos
             WHERE draft_id = ?1 AND role = ?2",
            params![input.draft_id, input.role],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let quality_json = serde_json::to_string(&processed.quality_messages)
        .map_err(|error| WorkshopError::Invalid(error.to_string()))?;
    let storage_key = format!(
        "{}/photos/{}.{}",
        input.draft_id, processed.sha256, processed.extension
    );
    let transaction = connection.transaction()?;
    let database_result = transaction.execute(
        "INSERT INTO draft_photos (
           draft_id, role, original_name, storage_key, mime_type,
           width, height, byte_size, sha256,
           crop_x, crop_y, crop_width, crop_height,
           quality_status, quality_messages, created_at, updated_at
         ) VALUES (
           ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
           ?10, ?11, ?12, ?13, ?14, ?15, datetime('now'), datetime('now')
         )
         ON CONFLICT(draft_id, role) DO UPDATE SET
           original_name = excluded.original_name,
           storage_key = excluded.storage_key,
           mime_type = excluded.mime_type,
           width = excluded.width,
           height = excluded.height,
           byte_size = excluded.byte_size,
           sha256 = excluded.sha256,
           crop_x = excluded.crop_x,
           crop_y = excluded.crop_y,
           crop_width = excluded.crop_width,
           crop_height = excluded.crop_height,
           quality_status = excluded.quality_status,
           quality_messages = excluded.quality_messages,
           updated_at = excluded.updated_at",
        params![
            input.draft_id,
            input.role,
            original_name,
            storage_key,
            processed.mime_type,
            i64::from(processed.width),
            i64::from(processed.height),
            i64::try_from(processed.bytes.len()).unwrap_or(i64::MAX),
            processed.sha256,
            input.crop_x,
            input.crop_y,
            input.crop_width,
            input.crop_height,
            processed.quality_status,
            quality_json,
        ],
    );
    if let Err(error) = database_result {
        if created_file {
            let _ = fs::remove_file(&final_path);
        }
        return Err(error.into());
    }
    if let Err(error) = transaction.execute(
        "UPDATE creation_drafts
         SET status = CASE
             WHEN EXISTS(
               SELECT 1 FROM draft_photos
               WHERE draft_id = ?1 AND role = 'primary'
             ) THEN 'ready'
             ELSE 'editing'
           END,
           snapshot_version = snapshot_version + 1,
           error_code = NULL, error_message = NULL, retryable = 0,
           updated_at = datetime('now')
         WHERE id = ?1",
        [input.draft_id],
    ) {
        if created_file {
            let _ = fs::remove_file(&final_path);
        }
        return Err(error.into());
    }
    if let Err(error) = transaction.commit() {
        if created_file {
            let _ = fs::remove_file(&final_path);
        }
        return Err(error.into());
    }

    if let Some((old_hash, old_mime)) = previous
        && old_hash != processed.sha256
    {
        remove_photo_file_if_unreferenced(
            connection,
            &photos_directory,
            input.draft_id,
            &old_hash,
            &old_mime,
        )?;
    }
    get_draft(connection, drafts_root, input.draft_id)
}

pub fn remove_photo(
    connection: &mut Connection,
    drafts_root: &Path,
    draft_id: &str,
    role: &str,
) -> Result<CreationDraft, WorkshopError> {
    validate_draft_id(draft_id)?;
    validate_photo_role(role)?;
    ensure_draft_editable(connection, draft_id)?;
    let previous = connection
        .query_row(
            "SELECT sha256, mime_type FROM draft_photos
             WHERE draft_id = ?1 AND role = ?2",
            params![draft_id, role],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or_else(|| WorkshopError::Invalid("照片不存在".to_owned()))?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "DELETE FROM draft_photos WHERE draft_id = ?1 AND role = ?2",
        params![draft_id, role],
    )?;
    transaction.execute(
        "UPDATE creation_drafts
         SET status = CASE
             WHEN EXISTS(
               SELECT 1 FROM draft_photos
               WHERE draft_id = ?1 AND role = 'primary'
             ) THEN 'ready'
             ELSE 'editing'
           END,
           snapshot_version = snapshot_version + 1,
           error_code = NULL, error_message = NULL, retryable = 0,
           updated_at = datetime('now')
         WHERE id = ?1",
        [draft_id],
    )?;
    transaction.commit()?;
    remove_photo_file_if_unreferenced(
        connection,
        &drafts_root.join(draft_id).join("photos"),
        draft_id,
        &previous.0,
        &previous.1,
    )?;
    get_draft(connection, drafts_root, draft_id)
}

fn remove_photo_file_if_unreferenced(
    connection: &Connection,
    photos_directory: &Path,
    draft_id: &str,
    sha256: &str,
    mime_type: &str,
) -> Result<(), WorkshopError> {
    let references: i64 = connection.query_row(
        "SELECT COUNT(*) FROM draft_photos WHERE draft_id = ?1 AND sha256 = ?2",
        params![draft_id, sha256],
        |row| row.get(0),
    )?;
    if references == 0 {
        let extension = if mime_type == "image/png" {
            "png"
        } else {
            "jpg"
        };
        match fs::remove_file(photos_directory.join(format!("{sha256}.{extension}"))) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub fn start_generation(
    connection: &mut Connection,
    drafts_root: &Path,
    draft_id: &str,
) -> Result<CreationDraft, WorkshopError> {
    validate_draft_id(draft_id)?;
    let draft = get_draft(connection, drafts_root, draft_id)?;
    if !draft.photos.iter().any(|photo| photo.role == "primary") {
        return rejected("请先添加并确认主照片");
    }
    if draft.subject_kind == "human_avatar" && !draft.authorization_confirmed {
        return rejected("人物草稿缺少有效的成年人授权确认");
    }
    connection.execute(
        "UPDATE creation_drafts
         SET status = 'submitting',
             snapshot_version = snapshot_version + 1,
             progress_percent = 0,
             server_job_id = NULL,
             error_code = NULL,
             error_message = NULL,
             retryable = 0,
             updated_at = datetime('now')
         WHERE id = ?1",
        [draft_id],
    )?;
    get_draft(connection, drafts_root, draft_id)
}

pub fn update_generation(
    connection: &mut Connection,
    drafts_root: &Path,
    draft_id: &str,
    update: GenerationDraftUpdate,
) -> Result<CreationDraft, WorkshopError> {
    validate_draft_id(draft_id)?;
    if !matches!(
        update.status.as_str(),
        "submitting"
            | "checking"
            | "queued"
            | "generating_portrait"
            | "awaiting_confirmation"
            | "generating_actions"
            | "packaging"
            | "completed"
            | "service_unavailable"
            | "failed"
            | "cancelled"
    ) {
        return rejected("无效的生成状态");
    }
    if update
        .server_job_id
        .as_ref()
        .is_some_and(|value| value.len() > 80 || !value.starts_with("gen_"))
    {
        return rejected("无效的服务端任务 ID");
    }
    connection.execute(
        "UPDATE creation_drafts
         SET status = ?2,
             snapshot_version = snapshot_version + 1,
             progress_percent = ?3,
             server_job_id = COALESCE(?4, server_job_id),
             error_code = ?5,
             error_message = ?6,
             retryable = ?7,
             updated_at = datetime('now')
         WHERE id = ?1",
        params![
            draft_id,
            update.status,
            update.progress_percent,
            update.server_job_id,
            update.error_code,
            update.error_message,
            i64::from(update.retryable)
        ],
    )?;
    get_draft(connection, drafts_root, draft_id)
}

pub fn cancel_draft(
    connection: &mut Connection,
    drafts_root: &Path,
    draft_id: &str,
) -> Result<CreationDraft, WorkshopError> {
    validate_draft_id(draft_id)?;
    let status: String = connection
        .query_row(
            "SELECT status FROM creation_drafts WHERE id = ?1",
            [draft_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| WorkshopError::Invalid("草稿不存在".to_owned()))?;
    if status == "completed" {
        return rejected("已完成的任务不能取消");
    }
    let draft_root = drafts_root.join(draft_id);
    let photos_root = draft_root.join("photos");
    let cancelled_root = draft_root.join(".cancelled-photos");
    if cancelled_root.exists() {
        return rejected("草稿存在未恢复的取消操作，请重启后再试");
    }
    let moved = if photos_root.exists() {
        fs::rename(&photos_root, &cancelled_root)?;
        true
    } else {
        false
    };
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => {
            if moved {
                let _ = fs::rename(&cancelled_root, &photos_root);
            }
            return Err(error.into());
        }
    };
    if let Err(error) =
        transaction.execute("DELETE FROM draft_photos WHERE draft_id = ?1", [draft_id])
    {
        if moved {
            let _ = fs::rename(&cancelled_root, &photos_root);
        }
        return Err(error.into());
    }
    if let Err(error) = transaction.execute(
        "UPDATE creation_drafts
         SET status = 'cancelled', snapshot_version = snapshot_version + 1,
             progress_percent = NULL, retryable = 0, updated_at = datetime('now')
         WHERE id = ?1 AND status != 'completed'",
        [draft_id],
    ) {
        if moved {
            let _ = fs::rename(&cancelled_root, &photos_root);
        }
        return Err(error.into());
    }
    if let Err(error) = transaction.commit() {
        if moved {
            let _ = fs::rename(&cancelled_root, &photos_root);
        }
        return Err(error.into());
    }
    if moved {
        let _ = fs::remove_dir_all(cancelled_root);
    }
    get_draft(connection, drafts_root, draft_id)
}

pub fn delete_draft(
    connection: &mut Connection,
    drafts_root: &Path,
    draft_id: &str,
) -> Result<(), WorkshopError> {
    validate_draft_id(draft_id)?;
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM creation_drafts WHERE id = ?1)",
        [draft_id],
        |row| row.get(0),
    )?;
    if !exists {
        return rejected("草稿不存在");
    }
    let source = drafts_root.join(draft_id);
    let trash = drafts_root.join(".trash").join(draft_id);
    if trash.exists() {
        return rejected("草稿存在未完成的删除恢复，请重启后再试");
    }
    let moved = if source.exists() {
        fs::create_dir_all(trash.parent().unwrap_or(drafts_root))?;
        fs::rename(&source, &trash)?;
        true
    } else {
        false
    };
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => {
            if moved {
                let _ = fs::rename(&trash, &source);
            }
            return Err(error.into());
        }
    };
    if let Err(error) = transaction.execute("DELETE FROM creation_drafts WHERE id = ?1", [draft_id])
    {
        if moved {
            let _ = fs::rename(&trash, &source);
        }
        return Err(error.into());
    }
    if let Err(error) = transaction.commit() {
        if moved {
            let _ = fs::rename(&trash, &source);
        }
        return Err(error.into());
    }
    if moved {
        let _ = fs::remove_dir_all(trash);
    }
    Ok(())
}

pub fn rename_character(
    connection: &Connection,
    character_id: &str,
    custom_name: &str,
) -> Result<(), WorkshopError> {
    validate_character_id(character_id)?;
    let custom_name = custom_name.trim();
    if custom_name.is_empty()
        || custom_name.chars().count() > 64
        || custom_name.chars().any(char::is_control)
    {
        return rejected("角色名称必须为 1-64 个可显示字符");
    }
    let affected = connection.execute(
        "UPDATE characters SET custom_name = ?2, updated_at = datetime('now')
         WHERE id = ?1 AND built_in = 0",
        params![character_id, custom_name],
    )?;
    if affected == 0 {
        return rejected("只能重命名已安装的非内置角色");
    }
    Ok(())
}

pub fn cleanup_storage(connection: &Connection, drafts_root: &Path) -> Result<(), WorkshopError> {
    if !drafts_root.is_dir() {
        return Ok(());
    }
    let trash_root = drafts_root.join(".trash");
    if trash_root.is_dir() {
        for entry in fs::read_dir(&trash_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let draft_id = entry.file_name().to_string_lossy().into_owned();
            if validate_draft_id(&draft_id).is_err() {
                continue;
            }
            let indexed: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM creation_drafts WHERE id = ?1)",
                [&draft_id],
                |row| row.get(0),
            )?;
            if indexed {
                let destination = drafts_root.join(&draft_id);
                if destination.exists() {
                    return rejected("草稿删除恢复目标已经存在");
                }
                fs::rename(entry.path(), destination)?;
            } else {
                fs::remove_dir_all(entry.path())?;
            }
        }
        let _ = fs::remove_dir(&trash_root);
    }
    for entry in fs::read_dir(drafts_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let draft_id = entry.file_name().to_string_lossy().into_owned();
        if validate_draft_id(&draft_id).is_err() {
            continue;
        }
        let draft_root = entry.path();
        let cancelled = draft_root.join(".cancelled-photos");
        if cancelled.is_dir() {
            let photo_count: i64 = connection.query_row(
                "SELECT COUNT(*) FROM draft_photos WHERE draft_id = ?1",
                [&draft_id],
                |row| row.get(0),
            )?;
            if photo_count > 0 {
                let destination = draft_root.join("photos");
                if destination.exists() {
                    return rejected("照片取消恢复目标已经存在");
                }
                fs::rename(&cancelled, destination)?;
            } else {
                fs::remove_dir_all(cancelled)?;
            }
        }
        for file in fs::read_dir(&draft_root)? {
            let file = file?;
            if file.file_type()?.is_file()
                && file.file_name().to_string_lossy().starts_with(".photo-")
            {
                fs::remove_file(file.path())?;
            }
        }
    }
    Ok(())
}

fn get_draft(
    connection: &Connection,
    drafts_root: &Path,
    draft_id: &str,
) -> Result<CreationDraft, WorkshopError> {
    validate_draft_id(draft_id)?;
    let mut draft = connection
        .query_row(
            "SELECT id, subject_kind, display_name, authorization_confirmed,
                    authorization_version, status, snapshot_version,
                    progress_percent, server_job_id, server_expires_at,
                    error_code, error_message, retryable, created_at, updated_at
             FROM creation_drafts WHERE id = ?1",
            [draft_id],
            |row| {
                let version = row.get::<_, i64>(6)?;
                let progress = row.get::<_, Option<i64>>(7)?;
                Ok(CreationDraft {
                    id: row.get(0)?,
                    subject_kind: row.get(1)?,
                    display_name: row.get(2)?,
                    authorization_confirmed: row.get::<_, i64>(3)? != 0,
                    authorization_version: row.get(4)?,
                    status: row.get(5)?,
                    snapshot_version: u64::try_from(version).unwrap_or_default(),
                    progress_percent: progress.and_then(|value| u8::try_from(value).ok()),
                    server_job_id: row.get(8)?,
                    server_expires_at: row.get(9)?,
                    error_code: row.get(10)?,
                    error_message: row.get(11)?,
                    retryable: row.get::<_, i64>(12)? != 0,
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
                    photos: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| WorkshopError::Invalid("草稿不存在".to_owned()))?;
    draft.photos = list_photos(connection, drafts_root, draft_id)?;
    Ok(draft)
}

fn list_photos(
    connection: &Connection,
    drafts_root: &Path,
    draft_id: &str,
) -> Result<Vec<DraftPhoto>, WorkshopError> {
    let mut statement = connection.prepare(
        "SELECT role, original_name, mime_type, width, height, byte_size,
                sha256, crop_x, crop_y, crop_width, crop_height,
                quality_status, quality_messages
         FROM draft_photos WHERE draft_id = ?1
         ORDER BY CASE role
           WHEN 'primary' THEN 0 WHEN 'supplemental_1' THEN 1 ELSE 2 END",
    )?;
    let rows = statement.query_map([draft_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, f64>(7)?,
            row.get::<_, f64>(8)?,
            row.get::<_, f64>(9)?,
            row.get::<_, f64>(10)?,
            row.get::<_, String>(11)?,
            row.get::<_, String>(12)?,
        ))
    })?;
    rows.map(|row| {
        let (
            role,
            original_name,
            mime_type,
            width,
            height,
            byte_size,
            sha256,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
            quality_status,
            quality_json,
        ) = row?;
        let extension = if mime_type == "image/png" {
            "png"
        } else {
            "jpg"
        };
        let bytes = fs::read(
            drafts_root
                .join(draft_id)
                .join("photos")
                .join(format!("{sha256}.{extension}")),
        )?;
        if bytes.len() > MAX_CLEANED_PHOTO_BYTES {
            return rejected("清理后照片大小异常");
        }
        let quality_messages = serde_json::from_str(&quality_json)
            .map_err(|error| WorkshopError::Invalid(error.to_string()))?;
        Ok(DraftPhoto {
            role,
            original_name,
            mime_type: mime_type.clone(),
            width: u32::try_from(width).unwrap_or_default(),
            height: u32::try_from(height).unwrap_or_default(),
            byte_size: u64::try_from(byte_size).unwrap_or_default(),
            sha256,
            crop_x,
            crop_y,
            crop_width,
            crop_height,
            quality_status,
            quality_messages,
            preview_data_url: format!("data:{mime_type};base64,{}", STANDARD.encode(bytes)),
        })
    })
    .collect()
}

fn process_cleaned_photo(bytes: &[u8]) -> Result<ProcessedPhoto, WorkshopError> {
    if bytes.is_empty() || bytes.len() > MAX_CLEANED_PHOTO_BYTES {
        return rejected("清理后照片大小必须在 1 字节到 8 MB 之间");
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = reader
        .format()
        .ok_or_else(|| WorkshopError::Invalid("无法识别照片内容格式".to_owned()))?;
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
    ) {
        return rejected("照片内容只允许 JPEG、PNG 或 WebP");
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_DIMENSION);
    limits.max_image_height = Some(MAX_DECODE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let mut image = reader.decode()?;
    let (width, height) = image.dimensions();
    if width < 256 || height < 256 {
        return rejected("裁剪结果至少需要 256×256 像素");
    }
    if width > MAX_OUTPUT_DIMENSION || height > MAX_OUTPUT_DIMENSION {
        image = image.thumbnail(MAX_OUTPUT_DIMENSION, MAX_OUTPUT_DIMENSION);
    }
    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8();
    let visible_pixels = rgba.pixels().filter(|pixel| pixel.0[3] > 16).count();
    let total_pixels = u64::from(width) * u64::from(height);
    if visible_pixels as u64 * 100 < total_pixels {
        return rejected("照片几乎完全透明，请选择可见主体照片");
    }
    let has_alpha = rgba.pixels().any(|pixel| pixel.0[3] < 255);
    let mut output = Vec::new();
    let (mime_type, extension) = if has_alpha {
        PngEncoder::new(&mut output).write_image(
            rgba.as_raw(),
            width,
            height,
            ColorType::Rgba8.into(),
        )?;
        ("image/png", "png")
    } else {
        let rgb = DynamicImage::ImageRgba8(rgba).to_rgb8();
        JpegEncoder::new_with_quality(&mut output, 90).encode(
            rgb.as_raw(),
            width,
            height,
            ColorType::Rgb8.into(),
        )?;
        ("image/jpeg", "jpg")
    };
    if output.len() > MAX_CLEANED_PHOTO_BYTES {
        return rejected("重新编码后的照片超过 8 MB");
    }
    let verified = image::load_from_memory(&output)?;
    if verified.dimensions() != (width, height) {
        return rejected("照片重新编码后的尺寸校验失败");
    }

    let mut quality_messages = Vec::new();
    if width < 768 || height < 768 {
        quality_messages.push("分辨率偏低，生成细节可能不足。".to_owned());
    }
    let ratio = f64::from(width.max(height)) / f64::from(width.min(height));
    if ratio > 2.2 {
        quality_messages.push("画面比例较狭长，请确认主体没有被裁掉。".to_owned());
    }
    let luma = verified.to_luma8();
    let average_luma = luma
        .pixels()
        .map(|pixel| u64::from(pixel.0[0]))
        .sum::<u64>() as f64
        / f64::from(width * height);
    if average_luma < 35.0 {
        quality_messages.push("画面整体偏暗，建议换用光线更好的照片。".to_owned());
    } else if average_luma > 235.0 {
        quality_messages.push("画面整体过亮，主体细节可能丢失。".to_owned());
    }
    let sharpness = approximate_sharpness(&luma);
    if sharpness < 3.0 {
        quality_messages.push("画面可能模糊，请检查脸部或猫咪头部是否清晰。".to_owned());
    }
    let quality_status = if quality_messages.is_empty() {
        "accepted"
    } else {
        "warning"
    };
    let sha256 = format!("{:x}", Sha256::digest(&output));
    Ok(ProcessedPhoto {
        bytes: output,
        mime_type,
        extension,
        width,
        height,
        sha256,
        quality_status,
        quality_messages,
    })
}

fn approximate_sharpness(image: &image::GrayImage) -> f64 {
    let (width, height) = image.dimensions();
    if width < 2 || height < 2 {
        return 0.0;
    }
    let step = (width.max(height) / 512).max(1) as usize;
    let mut difference = 0_u64;
    let mut samples = 0_u64;
    for y in (0..height - 1).step_by(step) {
        for x in (0..width - 1).step_by(step) {
            let current = i16::from(image.get_pixel(x, y).0[0]);
            let right = i16::from(image.get_pixel(x + 1, y).0[0]);
            let below = i16::from(image.get_pixel(x, y + 1).0[0]);
            difference += u64::from(current.abs_diff(right) + current.abs_diff(below));
            samples += 2;
        }
    }
    if samples == 0 {
        0.0
    } else {
        difference as f64 / samples as f64
    }
}

fn ensure_draft_editable(connection: &Connection, draft_id: &str) -> Result<(), WorkshopError> {
    let status: Option<String> = connection
        .query_row(
            "SELECT status FROM creation_drafts WHERE id = ?1",
            [draft_id],
            |row| row.get(0),
        )
        .optional()?;
    match status.as_deref() {
        Some("completed" | "cancelled") => rejected("已完成或已取消的草稿不能修改照片"),
        Some(_) => Ok(()),
        None => rejected("草稿不存在"),
    }
}

fn validate_subject_kind(value: &str) -> Result<(), WorkshopError> {
    if matches!(value, "pet_cat" | "human_avatar") {
        Ok(())
    } else {
        rejected("主体类型只允许 pet_cat 或 human_avatar")
    }
}

fn sanitize_display_name(value: &str) -> Result<String, WorkshopError> {
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return rejected("草稿名必须为 1-64 个可显示字符");
    }
    Ok(name.to_owned())
}

fn validate_photo_role(value: &str) -> Result<(), WorkshopError> {
    if matches!(value, "primary" | "supplemental_1" | "supplemental_2") {
        Ok(())
    } else {
        rejected("照片角色无效")
    }
}

fn validate_crop(x: f64, y: f64, width: f64, height: f64) -> Result<(), WorkshopError> {
    if [x, y, width, height]
        .into_iter()
        .any(|value| !value.is_finite())
        || x < 0.0
        || y < 0.0
        || width <= 0.0
        || height <= 0.0
        || x + width > 1.000_001
        || y + height > 1.000_001
    {
        return rejected("裁剪区域必须位于 0-1 归一化图像范围内");
    }
    Ok(())
}

fn validate_draft_id(value: &str) -> Result<(), WorkshopError> {
    if !value.starts_with("draft_")
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return rejected("草稿 ID 格式无效");
    }
    Ok(())
}

fn validate_character_id(value: &str) -> Result<(), WorkshopError> {
    if value.len() > 80
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return rejected("角色 ID 格式无效");
    }
    Ok(())
}

fn sanitize_original_name(value: &str) -> Result<String, WorkshopError> {
    let name = Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| WorkshopError::Invalid("原照片名称无效".to_owned()))?;
    if name.is_empty() || name.chars().count() > 128 || name.chars().any(char::is_control) {
        return rejected("原照片名称必须为 1-128 个可显示字符");
    }
    Ok(name.to_owned())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), WorkshopError> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()?;
    Ok(())
}

fn unique_draft_id() -> String {
    let counter = DRAFT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("draft_{}_{}_{}", std::process::id(), nanos, counter)
}

fn rejected<T>(message: impl Into<String>) -> Result<T, WorkshopError> {
    Err(WorkshopError::Invalid(message.into()))
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};
    use tempfile::tempdir;

    use super::*;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE characters (
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   built_in INTEGER NOT NULL,
                   custom_name TEXT,
                   updated_at TEXT
                 );
                 CREATE TABLE creation_drafts (
                   id TEXT PRIMARY KEY,
                   subject_kind TEXT NOT NULL,
                   display_name TEXT,
                   authorization_confirmed INTEGER NOT NULL,
                   authorization_version TEXT,
                   status TEXT NOT NULL,
                   snapshot_version INTEGER NOT NULL DEFAULT 0,
                   progress_percent INTEGER,
                   server_job_id TEXT,
                   server_expires_at TEXT,
                   error_code TEXT,
                   error_message TEXT,
                   retryable INTEGER NOT NULL DEFAULT 0,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE draft_photos (
                   draft_id TEXT NOT NULL,
                   role TEXT NOT NULL,
                   original_name TEXT NOT NULL,
                   storage_key TEXT NOT NULL,
                   mime_type TEXT NOT NULL,
                   width INTEGER NOT NULL,
                   height INTEGER NOT NULL,
                   byte_size INTEGER NOT NULL,
                   sha256 TEXT NOT NULL,
                   crop_x REAL NOT NULL,
                   crop_y REAL NOT NULL,
                   crop_width REAL NOT NULL,
                   crop_height REAL NOT NULL,
                   quality_status TEXT NOT NULL,
                   quality_messages TEXT NOT NULL,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL,
                   PRIMARY KEY (draft_id, role),
                   FOREIGN KEY (draft_id) REFERENCES creation_drafts(id) ON DELETE CASCADE
                 );",
            )
            .unwrap();
        connection
    }

    fn png(width: u32, height: u32, alpha: u8) -> Vec<u8> {
        let mut image = RgbaImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgba([
                (x % 255) as u8,
                (y % 255) as u8,
                ((x + y) % 255) as u8,
                alpha,
            ]);
        }
        let mut output = Vec::new();
        PngEncoder::new(&mut output)
            .write_image(image.as_raw(), width, height, ColorType::Rgba8.into())
            .unwrap();
        output
    }

    #[test]
    fn human_draft_requires_versioned_authorization() {
        let root = tempdir().unwrap();
        let mut connection = database();
        assert!(
            create_draft(
                &mut connection,
                root.path(),
                "human_avatar",
                "未授权人物",
                false,
            )
            .unwrap_err()
            .to_string()
            .contains("授权")
        );
        let draft = create_draft(
            &mut connection,
            root.path(),
            "human_avatar",
            "  我的人物草稿  ",
            true,
        )
        .unwrap();
        assert_eq!(draft.display_name.as_deref(), Some("我的人物草稿"));
        assert!(draft.authorization_confirmed);
        assert_eq!(
            draft.authorization_version.as_deref(),
            Some(AUTHORIZATION_VERSION)
        );
    }

    #[test]
    fn draft_name_is_required_trimmed_and_bounded() {
        let root = tempdir().unwrap();
        let mut connection = database();

        for invalid_name in ["", "   ", "坏\n名字"] {
            assert!(
                create_draft(&mut connection, root.path(), "pet_cat", invalid_name, false,)
                    .unwrap_err()
                    .to_string()
                    .contains("草稿名")
            );
        }

        let too_long = "名".repeat(65);
        assert!(
            create_draft(&mut connection, root.path(), "pet_cat", &too_long, false,)
                .unwrap_err()
                .to_string()
                .contains("草稿名")
        );

        let draft =
            create_draft(&mut connection, root.path(), "pet_cat", "  橘子  ", false).unwrap();
        assert_eq!(draft.display_name.as_deref(), Some("橘子"));
    }

    #[test]
    fn photos_are_reencoded_hashed_persisted_and_roles_are_independent() {
        let root = tempdir().unwrap();
        let mut connection = database();
        let draft = create_draft(&mut connection, root.path(), "pet_cat", "橘子", false).unwrap();
        let source = png(900, 800, 255);
        let primary = save_photo(
            &mut connection,
            root.path(),
            SavePhotoInput {
                draft_id: &draft.id,
                role: "primary",
                original_name: r"C:\private\cat.png",
                encoded_bytes: &source,
                crop_x: 0.0,
                crop_y: 0.0,
                crop_width: 1.0,
                crop_height: 1.0,
            },
        )
        .unwrap();
        assert_eq!(primary.status, "ready");
        assert_eq!(primary.photos.len(), 1);
        assert_eq!(primary.photos[0].original_name, "cat.png");
        assert_eq!(primary.photos[0].mime_type, "image/jpeg");
        assert_eq!(primary.photos[0].sha256.len(), 64);

        let with_supplement = save_photo(
            &mut connection,
            root.path(),
            SavePhotoInput {
                draft_id: &draft.id,
                role: "supplemental_1",
                original_name: "side.webp",
                encoded_bytes: &source,
                crop_x: 0.1,
                crop_y: 0.1,
                crop_width: 0.8,
                crop_height: 0.8,
            },
        )
        .unwrap();
        assert_eq!(with_supplement.photos.len(), 2);

        let after_remove =
            remove_photo(&mut connection, root.path(), &draft.id, "supplemental_1").unwrap();
        assert_eq!(after_remove.status, "ready");
        assert_eq!(after_remove.photos.len(), 1);
        assert_eq!(after_remove.photos[0].role, "primary");
    }

    #[test]
    fn rejects_corrupt_oversized_crop_and_transparent_images() {
        assert!(process_cleaned_photo(b"not an image").is_err());
        assert!(process_cleaned_photo(&png(300, 300, 0)).is_err());
        assert!(validate_crop(0.8, 0.0, 0.4, 1.0).is_err());
    }

    #[test]
    fn reencoding_drops_appended_metadata_bytes() {
        let mut source = png(400, 400, 255);
        source.extend_from_slice(b"GPS_SECRET_METADATA");
        let processed = process_cleaned_photo(&source).unwrap();
        assert!(
            !processed
                .bytes
                .windows(b"GPS_SECRET_METADATA".len())
                .any(|window| window == b"GPS_SECRET_METADATA")
        );
        assert!(image::load_from_memory(&processed.bytes).is_ok());
    }

    #[test]
    fn generation_updates_and_cancelled_states_survive_snapshot_reload() {
        let root = tempdir().unwrap();
        let mut connection = database();
        let draft =
            create_draft(&mut connection, root.path(), "pet_cat", "测试猫咪", false).unwrap();
        let source = png(512, 512, 255);
        save_photo(
            &mut connection,
            root.path(),
            SavePhotoInput {
                draft_id: &draft.id,
                role: "primary",
                original_name: "cat.png",
                encoded_bytes: &source,
                crop_x: 0.0,
                crop_y: 0.0,
                crop_width: 1.0,
                crop_height: 1.0,
            },
        )
        .unwrap();
        let submitting = start_generation(&mut connection, root.path(), &draft.id).unwrap();
        assert_eq!(submitting.status, "submitting");
        let unavailable = update_generation(
            &mut connection,
            root.path(),
            &draft.id,
            GenerationDraftUpdate {
                status: "service_unavailable".to_owned(),
                progress_percent: None,
                server_job_id: None,
                error_code: Some("local_generation_failed".to_owned()),
                error_message: Some("local service stopped".to_owned()),
                retryable: true,
            },
        )
        .unwrap();
        assert_eq!(unavailable.status, "service_unavailable");
        assert!(unavailable.retryable);
        assert_eq!(snapshot(&connection, root.path()).unwrap().drafts.len(), 1);

        let cancelled = cancel_draft(&mut connection, root.path(), &draft.id).unwrap();
        assert_eq!(cancelled.status, "cancelled");
        assert!(cancelled.photos.is_empty());
        assert!(!root.path().join(&draft.id).join("photos").exists());
    }
}
