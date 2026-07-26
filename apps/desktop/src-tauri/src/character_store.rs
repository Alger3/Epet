use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use reqwest::{Url, blocking::Client, redirect::Policy};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use thiserror::Error;

use crate::package::{LoadedPackage, MAX_COMPRESSED_BYTES, PackageError, load_epet};

static UNIQUE_COUNTER: AtomicU64 = AtomicU64::new(0);
const DOWNLOAD_SPACE_SAFETY_BYTES: u64 = 16 * 1024 * 1024;
const INSTALL_SPACE_SAFETY_BYTES: u64 = 64 * 1024 * 1024;
const STALE_TEMPORARY_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Error)]
pub enum CharacterStoreError {
    #[error("角色库文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("角色库索引操作失败：{0}")]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Package(#[from] PackageError),
    #[error("角色包下载失败：{0}")]
    Download(#[from] reqwest::Error),
    #[error("角色库操作被拒绝：{0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterVersion {
    pub package_version: String,
    pub package_sha256: String,
    pub package_size: u64,
    pub installed_at: String,
    pub source_url: Option<String>,
    pub current: bool,
    pub local_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterLibraryItem {
    pub id: String,
    pub name: String,
    pub subject_kind: String,
    pub built_in: bool,
    pub current_package_sha256: Option<String>,
    pub current_version: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub versions: Vec<CharacterVersion>,
    pub local_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCharacterDefinition {
    pub id: String,
    pub name: String,
    pub subject_kind: String,
    pub subject_label: String,
    pub description: String,
    pub asset_url: String,
    pub animation: RuntimeAtlasDefinition,
    pub hitboxes: Vec<RuntimeHitbox>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAtlasDefinition {
    pub image_url: String,
    pub canvas: RuntimeSize,
    pub frames: HashMap<String, RuntimeFrame>,
    pub actions: HashMap<String, RuntimeAction>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFrame {
    pub frame: RuntimeRect,
    pub source_size: RuntimeAtlasSize,
    pub sprite_source: RuntimeRect,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeAtlasSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAction {
    pub frames: Vec<String>,
    pub frame_duration_ms: Vec<u64>,
    pub r#loop: bool,
    pub fallback: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHitbox {
    pub shape: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct DownloadedPackage {
    path: PathBuf,
}

impl DownloadedPackage {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DownloadedPackage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn download_to_temporary(
    temporary_root: &Path,
    source_url: &str,
) -> Result<DownloadedPackage, CharacterStoreError> {
    let url = validate_download_url(source_url)?;
    fs::create_dir_all(temporary_root)?;
    ensure_free_space(
        temporary_root,
        MAX_COMPRESSED_BYTES + DOWNLOAD_SPACE_SAFETY_BYTES,
    )?;

    let redirect_policy = Policy::custom(|attempt| {
        if attempt.previous().len() > 3 {
            attempt.error("角色包重定向次数超过 3 次")
        } else if attempt.url().scheme() != "https" {
            attempt.error("角色包重定向只允许 HTTPS")
        } else {
            attempt.follow()
        }
    });
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .redirect(redirect_policy)
        .user_agent(concat!("Epet/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut response = client.get(url).send()?.error_for_status()?;

    if response
        .content_length()
        .is_some_and(|length| length == 0 || length > MAX_COMPRESSED_BYTES)
    {
        return rejected("远程角色包大小超出 1-30 MB 限制");
    }

    let temporary_path = temporary_root.join(format!("{}.epet", unique_name("download")));
    let download_result = (|| {
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        let copied = std::io::copy(
            &mut response.by_ref().take(MAX_COMPRESSED_BYTES + 1),
            &mut output,
        )?;
        if copied == 0 || copied > MAX_COMPRESSED_BYTES {
            return rejected("下载后的角色包大小超出 1-30 MB 限制");
        }
        output.flush()?;
        output.sync_all()?;
        Ok(())
    })();
    if let Err(error) = download_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }

    Ok(DownloadedPackage {
        path: temporary_path,
    })
}

pub fn validate_expected_sha256(value: &str) -> Result<(), CharacterStoreError> {
    validate_sha256(value)
}

pub fn install_package(
    connection: &mut Connection,
    library_root: &Path,
    package_path: &Path,
    expected_sha256: Option<&str>,
    source_url: Option<&str>,
) -> Result<CharacterLibraryItem, CharacterStoreError> {
    let package = load_epet(package_path, expected_sha256)?;
    let package_size = package_path.metadata()?.len();
    let pet_id = package.manifest.pet_id.clone();
    let package_sha256 = package.package_sha256.clone();
    fs::create_dir_all(library_root)?;
    let declared_size = package
        .manifest
        .files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.size))
        .ok_or_else(|| CharacterStoreError::Invalid("角色包声明大小溢出".to_owned()))?;
    ensure_free_space(
        library_root,
        package_size
            .saturating_add(declared_size)
            .saturating_add(INSTALL_SPACE_SAFETY_BYTES),
    )?;
    let staging_root = library_root.join(".staging");
    fs::create_dir_all(&staging_root)?;
    let staging_directory = staging_root.join(unique_name("install"));
    fs::create_dir(&staging_directory)?;

    let staged_content = staging_directory.join("content");
    if let Err(error) = package.extract_to(&staged_content) {
        let _ = fs::remove_dir_all(&staging_directory);
        return Err(error.into());
    }
    if let Err(error) = copy_package_file(package_path, &staging_directory.join("package.epet")) {
        let _ = fs::remove_dir_all(&staging_directory);
        return Err(error);
    }

    let character_root = library_root.join(&pet_id);
    let versions_root = character_root.join("versions");
    fs::create_dir_all(&versions_root)?;
    let final_directory = versions_root.join(&package_sha256);
    let mut created_final = false;

    if final_directory.exists() {
        if let Err(error) = verify_existing_version(&final_directory, &package) {
            let _ = fs::remove_dir_all(&staging_directory);
            return Err(error);
        }
        fs::remove_dir_all(&staging_directory)?;
    } else {
        if let Err(error) = fs::rename(&staging_directory, &final_directory) {
            let _ = fs::remove_dir_all(&staging_directory);
            remove_empty_parents(&versions_root, &character_root);
            return Err(error.into());
        }
        created_final = true;
    }

    let storage_key = format!("{pet_id}/versions/{package_sha256}");
    let indexed =
        index_installed_package(connection, &package, package_size, &storage_key, source_url);

    match indexed {
        Ok(()) => get_character(connection, &pet_id),
        Err(error) => {
            if created_final {
                let _ = fs::remove_dir_all(&final_directory);
                remove_empty_parents(&versions_root, &character_root);
            }
            Err(error)
        }
    }
}

pub fn list_characters(
    connection: &Connection,
) -> Result<Vec<CharacterLibraryItem>, CharacterStoreError> {
    let mut statement = connection.prepare(
        "SELECT id, COALESCE(custom_name, name), subject_kind, built_in, current_package_sha256,
                created_at, COALESCE(updated_at, created_at)
         FROM characters
         ORDER BY built_in DESC, COALESCE(updated_at, created_at) DESC, name ASC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)? != 0,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;
    let records = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    records
        .into_iter()
        .map(
            |(id, name, subject_kind, built_in, current_hash, created_at, updated_at)| {
                let versions = list_versions(connection, &id, current_hash.as_deref())?;
                let current_version = versions
                    .iter()
                    .find(|version| version.current)
                    .map(|version| version.package_version.clone());
                Ok(CharacterLibraryItem {
                    id,
                    name,
                    subject_kind,
                    built_in,
                    current_package_sha256: current_hash,
                    current_version,
                    created_at,
                    updated_at,
                    versions,
                    local_available: built_in,
                })
            },
        )
        .collect()
}

pub fn load_runtime_definition(
    connection: &Connection,
    library_root: &Path,
    character_id: &str,
) -> Result<RuntimeCharacterDefinition, CharacterStoreError> {
    validate_character_id(character_id)?;
    let package_sha256: String = connection
        .query_row(
            "SELECT current_package_sha256 FROM characters
             WHERE id = ?1 AND built_in = 0 AND current_package_sha256 IS NOT NULL",
            [character_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| CharacterStoreError::Invalid("角色不存在或没有可用的当前版本".to_owned()))?;
    let package = load_epet(
        &library_root
            .join(character_id)
            .join("versions")
            .join(&package_sha256)
            .join("package.epet"),
        Some(&package_sha256),
    )?;
    if package.manifest.pet_id != character_id {
        return rejected("角色包身份与角色库索引不一致");
    }
    runtime_definition_from_package(&package)
}

pub fn load_runtime_hitboxes(
    connection: &Connection,
    library_root: &Path,
    character_id: &str,
) -> Result<Vec<RuntimeHitbox>, CharacterStoreError> {
    Ok(load_runtime_definition(connection, library_root, character_id)?.hitboxes)
}

pub fn activate_version(
    connection: &mut Connection,
    library_root: &Path,
    character_id: &str,
    package_sha256: &str,
) -> Result<CharacterLibraryItem, CharacterStoreError> {
    validate_character_id(character_id)?;
    validate_sha256(package_sha256)?;
    let verified = load_epet(
        &library_root
            .join(character_id)
            .join("versions")
            .join(package_sha256)
            .join("package.epet"),
        Some(package_sha256),
    )?;
    if verified.manifest.pet_id != character_id {
        return rejected("旧版本文件与角色索引身份不一致");
    }
    let transaction = connection.transaction()?;
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM character_versions
           WHERE character_id = ?1 AND package_sha256 = ?2
         )",
        params![character_id, package_sha256],
        |row| row.get(0),
    )?;
    if !exists {
        return rejected("指定的角色旧版本不存在");
    }
    transaction.execute(
        "UPDATE characters
         SET current_package_sha256 = ?2, updated_at = datetime('now')
         WHERE id = ?1 AND built_in = 0",
        params![character_id, package_sha256],
    )?;
    transaction.commit()?;
    get_character(connection, character_id)
}

pub fn delete_version(
    connection: &mut Connection,
    library_root: &Path,
    character_id: &str,
    package_sha256: &str,
) -> Result<CharacterLibraryItem, CharacterStoreError> {
    validate_character_id(character_id)?;
    validate_sha256(package_sha256)?;
    let current: Option<String> = connection
        .query_row(
            "SELECT current_package_sha256 FROM characters
             WHERE id = ?1 AND built_in = 0",
            [character_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let Some(current) = current else {
        return rejected("角色不存在或属于内置角色");
    };
    if current == package_sha256 {
        return rejected("当前使用的版本不能删除，请先回滚到其他版本");
    }

    let source_directory = library_root
        .join(character_id)
        .join("versions")
        .join(package_sha256);
    let trash_directory = library_root
        .join(".trash")
        .join("versions")
        .join(character_id)
        .join(package_sha256);
    let moved = move_to_trash(&source_directory, &trash_directory)?;
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => {
            if moved {
                restore_from_trash(&trash_directory, &source_directory)?;
            }
            return Err(error.into());
        }
    };
    let affected = match transaction.execute(
        "DELETE FROM character_versions
         WHERE character_id = ?1 AND package_sha256 = ?2",
        params![character_id, package_sha256],
    ) {
        Ok(affected) => affected,
        Err(error) => {
            if moved {
                restore_from_trash(&trash_directory, &source_directory)?;
            }
            return Err(error.into());
        }
    };
    if affected == 0 {
        if moved {
            restore_from_trash(&trash_directory, &source_directory)?;
        }
        return rejected("指定的角色版本不存在");
    }
    if let Err(error) = transaction.commit() {
        if moved {
            restore_from_trash(&trash_directory, &source_directory)?;
        }
        return Err(error.into());
    }
    if moved {
        let _ = fs::remove_dir_all(&trash_directory);
    }
    get_character(connection, character_id)
}

pub fn delete_character(
    connection: &mut Connection,
    library_root: &Path,
    character_id: &str,
    active_character_id: &str,
) -> Result<(), CharacterStoreError> {
    validate_character_id(character_id)?;
    if character_id == active_character_id {
        return rejected("当前正在使用此角色，请先切换到其他角色");
    }

    let built_in: Option<bool> = connection
        .query_row(
            "SELECT built_in FROM characters WHERE id = ?1",
            [character_id],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()?;
    match built_in {
        None => return rejected("角色不存在"),
        Some(true) => return rejected("内置角色不能删除"),
        Some(false) => {}
    }

    let source_directory = library_root.join(character_id);
    let trash_directory = library_root
        .join(".trash")
        .join("characters")
        .join(character_id);
    let moved = move_to_trash(&source_directory, &trash_directory)?;
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => {
            if moved {
                restore_from_trash(&trash_directory, &source_directory)?;
            }
            return Err(error.into());
        }
    };
    if let Err(error) = transaction.execute("DELETE FROM characters WHERE id = ?1", [character_id])
    {
        if moved {
            restore_from_trash(&trash_directory, &source_directory)?;
        }
        return Err(error.into());
    }
    if let Err(error) = transaction.commit() {
        if moved {
            restore_from_trash(&trash_directory, &source_directory)?;
        }
        return Err(error.into());
    }
    if moved {
        let _ = fs::remove_dir_all(trash_directory);
    }
    Ok(())
}

pub fn cleanup_stale_storage(
    connection: &Connection,
    library_root: &Path,
    temporary_root: &Path,
) -> Result<(), CharacterStoreError> {
    cleanup_stale_storage_with_age(
        connection,
        library_root,
        temporary_root,
        STALE_TEMPORARY_AGE,
    )
}

fn validate_download_url(source_url: &str) -> Result<Url, CharacterStoreError> {
    if source_url.len() > 2_048 {
        return rejected("下载地址过长");
    }
    let url = Url::parse(source_url)
        .map_err(|_| CharacterStoreError::Invalid("下载地址不是有效的绝对 URL".to_owned()))?;
    if url.scheme() != "https" {
        return rejected("角色包下载只允许 HTTPS");
    }
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return rejected("下载地址不得包含凭据、片段或空主机名");
    }
    Ok(url)
}

fn runtime_definition_from_package(
    package: &LoadedPackage,
) -> Result<RuntimeCharacterDefinition, CharacterStoreError> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let atlas_bytes = package
        .file(&package.manifest.atlas.image)
        .ok_or_else(|| CharacterStoreError::Invalid("Atlas 图片不存在".to_owned()))?;
    let atlas_mime = mime_for_image_path(&package.manifest.atlas.image)?;
    let thumbnail_path = package
        .thumbnail_path()
        .ok_or_else(|| CharacterStoreError::Invalid("静态降级缩略图不存在".to_owned()))?;
    let thumbnail_bytes = package
        .file(thumbnail_path)
        .ok_or_else(|| CharacterStoreError::Invalid("静态降级缩略图无法读取".to_owned()))?;
    let thumbnail_mime = mime_for_image_path(thumbnail_path)?;

    let frames = package
        .atlas
        .frames
        .iter()
        .map(|(name, frame)| {
            (
                name.clone(),
                RuntimeFrame {
                    frame: RuntimeRect {
                        x: frame.frame.x,
                        y: frame.frame.y,
                        w: frame.frame.w,
                        h: frame.frame.h,
                    },
                    source_size: RuntimeAtlasSize {
                        w: frame.source_size.w,
                        h: frame.source_size.h,
                    },
                    sprite_source: RuntimeRect {
                        x: frame.sprite_source.x,
                        y: frame.sprite_source.y,
                        w: frame.sprite_source.w,
                        h: frame.sprite_source.h,
                    },
                },
            )
        })
        .collect();
    let actions = package
        .manifest
        .actions
        .iter()
        .map(|(name, action)| {
            (
                name.clone(),
                RuntimeAction {
                    frames: action.frames.clone(),
                    frame_duration_ms: action.frame_duration_ms.clone(),
                    r#loop: action.r#loop,
                    fallback: action
                        .fallback
                        .clone()
                        .or_else(|| action.next_action.clone()),
                },
            )
        })
        .collect();
    let hitboxes = package
        .manifest
        .hitboxes
        .iter()
        .map(|hitbox| RuntimeHitbox {
            shape: hitbox.shape.clone(),
            x: hitbox.x,
            y: hitbox.y,
            width: hitbox.w,
            height: hitbox.h,
        })
        .collect();

    Ok(RuntimeCharacterDefinition {
        id: package.manifest.pet_id.clone(),
        name: package.manifest.name.clone(),
        subject_kind: "pet_cat".to_owned(),
        subject_label: "猫咪".to_owned(),
        description: format!(
            "已安装角色包 v{}，动作由经过校验的 Sprite Atlas 提供。",
            package.manifest.package_version
        ),
        asset_url: format!(
            "data:{thumbnail_mime};base64,{}",
            STANDARD.encode(thumbnail_bytes)
        ),
        animation: RuntimeAtlasDefinition {
            image_url: format!("data:{atlas_mime};base64,{}", STANDARD.encode(atlas_bytes)),
            canvas: RuntimeSize {
                width: package.manifest.canvas.width,
                height: package.manifest.canvas.height,
            },
            frames,
            actions,
        },
        hitboxes,
    })
}

fn mime_for_image_path(path: &str) -> Result<&'static str, CharacterStoreError> {
    if path.ends_with(".png") {
        Ok("image/png")
    } else if path.ends_with(".webp") {
        Ok("image/webp")
    } else {
        rejected("角色运行时只支持 PNG/WebP 图片")
    }
}

fn copy_package_file(source: &Path, destination: &Path) -> Result<(), CharacterStoreError> {
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let copied = std::io::copy(&mut input, &mut output)?;
    if copied == 0 || copied > MAX_COMPRESSED_BYTES {
        return rejected("待安装角色包大小无效");
    }
    output.flush()?;
    output.sync_all()?;
    Ok(())
}

fn verify_existing_version(
    final_directory: &Path,
    package: &LoadedPackage,
) -> Result<(), CharacterStoreError> {
    let existing = load_epet(
        &final_directory.join("package.epet"),
        Some(&package.package_sha256),
    )?;
    if existing.manifest.pet_id != package.manifest.pet_id
        || existing.manifest.package_version != package.manifest.package_version
    {
        return rejected("角色库中同哈希目录的身份或版本不一致");
    }
    Ok(())
}

fn index_installed_package(
    connection: &mut Connection,
    package: &LoadedPackage,
    package_size: u64,
    storage_key: &str,
    source_url: Option<&str>,
) -> Result<(), CharacterStoreError> {
    let package_size = i64::try_from(package_size)
        .map_err(|_| CharacterStoreError::Invalid("角色包大小无法写入索引".to_owned()))?;
    let transaction = connection.transaction()?;

    let existing_character: Option<bool> = transaction
        .query_row(
            "SELECT built_in FROM characters WHERE id = ?1",
            [&package.manifest.pet_id],
            |row| Ok(row.get::<_, i64>(0)? != 0),
        )
        .optional()?;
    if existing_character == Some(true) {
        return rejected("角色包 ID 与内置角色冲突");
    }

    let version_hash: Option<String> = transaction
        .query_row(
            "SELECT package_sha256 FROM character_versions
             WHERE character_id = ?1 AND package_version = ?2",
            params![package.manifest.pet_id, package.manifest.package_version],
            |row| row.get(0),
        )
        .optional()?;
    if version_hash
        .as_deref()
        .is_some_and(|hash| hash != package.package_sha256)
    {
        return rejected("同一角色版本已经对应另一份内容，拒绝覆盖不可变版本");
    }

    transaction.execute(
        "INSERT INTO characters (
           id, name, subject_kind, asset_key, built_in, created_at,
           current_package_sha256, updated_at
         ) VALUES (?1, ?2, 'pet_cat', ?3, 0, ?4, ?5, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           current_package_sha256 = excluded.current_package_sha256,
           updated_at = excluded.updated_at
         WHERE characters.built_in = 0",
        params![
            package.manifest.pet_id,
            package.manifest.name,
            format!("package:{}", package.manifest.pet_id),
            package.manifest.created_at,
            package.package_sha256,
        ],
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO character_versions (
           character_id, package_version, package_sha256, storage_key,
           package_size, source_url, installed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
        params![
            package.manifest.pet_id,
            package.manifest.package_version,
            package.package_sha256,
            storage_key,
            package_size,
            source_url,
        ],
    )?;
    transaction.commit()?;
    Ok(())
}

fn get_character(
    connection: &Connection,
    character_id: &str,
) -> Result<CharacterLibraryItem, CharacterStoreError> {
    list_characters(connection)?
        .into_iter()
        .find(|character| character.id == character_id)
        .ok_or_else(|| CharacterStoreError::Invalid("角色索引写入后无法读取".to_owned()))
}

fn list_versions(
    connection: &Connection,
    character_id: &str,
    current_hash: Option<&str>,
) -> Result<Vec<CharacterVersion>, CharacterStoreError> {
    let mut statement = connection.prepare(
        "SELECT package_version, package_sha256, package_size, installed_at, source_url
         FROM character_versions
         WHERE character_id = ?1
         ORDER BY installed_at DESC, package_version DESC",
    )?;
    let rows = statement.query_map([character_id], |row| {
        let hash = row.get::<_, String>(1)?;
        let package_size = row.get::<_, i64>(2)?;
        Ok(CharacterVersion {
            package_version: row.get(0)?,
            current: current_hash == Some(hash.as_str()),
            package_sha256: hash,
            package_size: u64::try_from(package_size).unwrap_or_default(),
            installed_at: row.get(3)?,
            source_url: row.get(4)?,
            local_available: true,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CharacterStoreError::from)
}

fn validate_character_id(character_id: &str) -> Result<(), CharacterStoreError> {
    let Some(body) = character_id.strip_prefix("pet_") else {
        return rejected("只允许管理已安装的 pet_ 角色");
    };
    if !(8..=64).contains(&body.len())
        || !body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return rejected("角色 ID 格式无效");
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), CharacterStoreError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return rejected("版本 SHA-256 格式无效");
    }
    Ok(())
}

fn move_to_trash(source: &Path, trash: &Path) -> Result<bool, CharacterStoreError> {
    if !source.exists() {
        return Ok(false);
    }
    if trash.exists() {
        return rejected("存在尚未恢复的角色删除事务，请重启应用后重试");
    }
    if let Some(parent) = trash.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(source, trash)?;
    Ok(true)
}

fn restore_from_trash(trash: &Path, destination: &Path) -> Result<(), CharacterStoreError> {
    if !trash.exists() {
        return Ok(());
    }
    if destination.exists() {
        return rejected("删除回滚目标已存在，已保留隔离目录供恢复");
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(trash, destination)?;
    Ok(())
}

fn remove_empty_parents(versions_root: &Path, character_root: &Path) {
    let _ = fs::remove_dir(versions_root);
    let _ = fs::remove_dir(character_root);
}

fn cleanup_stale_storage_with_age(
    connection: &Connection,
    library_root: &Path,
    temporary_root: &Path,
    minimum_age: Duration,
) -> Result<(), CharacterStoreError> {
    recover_interrupted_deletions(connection, library_root)?;
    remove_stale_children(&library_root.join(".staging"), minimum_age, |_| true)?;
    remove_stale_children(temporary_root, minimum_age, |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("epet")
    })?;

    if !library_root.is_dir() {
        return Ok(());
    }
    for character_entry in fs::read_dir(library_root)? {
        let character_entry = character_entry?;
        if !character_entry.file_type()?.is_dir() {
            continue;
        }
        let character_id = character_entry.file_name().to_string_lossy().into_owned();
        if validate_character_id(&character_id).is_err() {
            continue;
        }
        let versions_root = character_entry.path().join("versions");
        if !versions_root.is_dir() {
            continue;
        }
        for version_entry in fs::read_dir(&versions_root)? {
            let version_entry = version_entry?;
            if !version_entry.file_type()?.is_dir() {
                continue;
            }
            let package_sha256 = version_entry.file_name().to_string_lossy().into_owned();
            if validate_sha256(&package_sha256).is_err()
                || !is_older_than(&version_entry.path(), minimum_age)?
            {
                continue;
            }
            let indexed: bool = connection.query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM character_versions
                   WHERE character_id = ?1 AND package_sha256 = ?2
                 )",
                params![character_id, package_sha256],
                |row| row.get(0),
            )?;
            if !indexed {
                fs::remove_dir_all(version_entry.path())?;
            }
        }
    }
    Ok(())
}

fn recover_interrupted_deletions(
    connection: &Connection,
    library_root: &Path,
) -> Result<(), CharacterStoreError> {
    let character_trash = library_root.join(".trash").join("characters");
    if character_trash.is_dir() {
        for entry in fs::read_dir(&character_trash)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let character_id = entry.file_name().to_string_lossy().into_owned();
            if validate_character_id(&character_id).is_err() {
                continue;
            }
            let indexed: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM characters WHERE id = ?1)",
                [&character_id],
                |row| row.get(0),
            )?;
            if indexed {
                restore_from_trash(&entry.path(), &library_root.join(&character_id))?;
            } else {
                fs::remove_dir_all(entry.path())?;
            }
        }
    }

    let version_trash = library_root.join(".trash").join("versions");
    if version_trash.is_dir() {
        for character_entry in fs::read_dir(&version_trash)? {
            let character_entry = character_entry?;
            if !character_entry.file_type()?.is_dir() {
                continue;
            }
            let character_id = character_entry.file_name().to_string_lossy().into_owned();
            if validate_character_id(&character_id).is_err() {
                continue;
            }
            for version_entry in fs::read_dir(character_entry.path())? {
                let version_entry = version_entry?;
                if !version_entry.file_type()?.is_dir() {
                    continue;
                }
                let package_sha256 = version_entry.file_name().to_string_lossy().into_owned();
                if validate_sha256(&package_sha256).is_err() {
                    continue;
                }
                let indexed: bool = connection.query_row(
                    "SELECT EXISTS(
                       SELECT 1 FROM character_versions
                       WHERE character_id = ?1 AND package_sha256 = ?2
                     )",
                    params![character_id, package_sha256],
                    |row| row.get(0),
                )?;
                if indexed {
                    restore_from_trash(
                        &version_entry.path(),
                        &library_root
                            .join(&character_id)
                            .join("versions")
                            .join(&package_sha256),
                    )?;
                } else {
                    fs::remove_dir_all(version_entry.path())?;
                }
            }
        }
    }
    remove_empty_trash_parents(library_root);
    Ok(())
}

fn remove_empty_trash_parents(library_root: &Path) {
    let trash_root = library_root.join(".trash");
    let _ = fs::remove_dir(trash_root.join("characters"));
    let version_root = trash_root.join("versions");
    if let Ok(entries) = fs::read_dir(&version_root) {
        for entry in entries.flatten() {
            let _ = fs::remove_dir(entry.path());
        }
    }
    let _ = fs::remove_dir(version_root);
    let _ = fs::remove_dir(trash_root);
}

fn remove_stale_children(
    root: &Path,
    minimum_age: Duration,
    allowed: impl Fn(&Path) -> bool,
) -> Result<(), CharacterStoreError> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !allowed(&path) || !is_older_than(&path, minimum_age)? {
            continue;
        }
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn is_older_than(path: &Path, minimum_age: Duration) -> Result<bool, CharacterStoreError> {
    let modified = path.metadata()?.modified()?;
    Ok(std::time::SystemTime::now()
        .duration_since(modified)
        .is_ok_and(|age| age >= minimum_age))
}

#[cfg(windows)]
fn ensure_free_space(path: &Path, required_bytes: u64) -> Result<(), CharacterStoreError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{Win32::Storage::FileSystem::GetDiskFreeSpaceExW, core::PCWSTR};

    let wide_path = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut available = 0_u64;
    // SAFETY: wide_path is NUL-terminated and remains alive for the duration of the call.
    unsafe { GetDiskFreeSpaceExW(PCWSTR(wide_path.as_ptr()), Some(&mut available), None, None) }
        .map_err(|error| {
            CharacterStoreError::Invalid(format!("无法检查角色库可用空间：{error}"))
        })?;
    if available < required_bytes {
        return rejected(format!(
            "磁盘空间不足：至少需要 {} MB 可用空间",
            required_bytes.div_ceil(1024 * 1024)
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn ensure_free_space(_path: &Path, _required_bytes: u64) -> Result<(), CharacterStoreError> {
    Ok(())
}

fn unique_name(prefix: &str) -> String {
    let counter = UNIQUE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{prefix}-{}-{nanos}-{counter}", std::process::id())
}

fn rejected<T>(message: impl Into<String>) -> Result<T, CharacterStoreError> {
    Err(CharacterStoreError::Invalid(message.into()))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::*;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE characters (
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   subject_kind TEXT NOT NULL,
                   asset_key TEXT NOT NULL UNIQUE,
                   built_in INTEGER NOT NULL,
                   created_at TEXT NOT NULL,
                   current_package_sha256 TEXT,
                   updated_at TEXT,
                   custom_name TEXT
                 );
                 CREATE TABLE character_versions (
                   character_id TEXT NOT NULL,
                   package_version TEXT NOT NULL,
                   package_sha256 TEXT NOT NULL,
                   storage_key TEXT NOT NULL UNIQUE,
                   package_size INTEGER NOT NULL,
                   source_url TEXT,
                   installed_at TEXT NOT NULL,
                   PRIMARY KEY (character_id, package_sha256),
                   UNIQUE (character_id, package_version),
                   FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
                 );
                 INSERT INTO characters (
                   id, name, subject_kind, asset_key, built_in, created_at, updated_at
                 ) VALUES (
                   'builtin-orange-tabby', '橘子', 'pet_cat',
                   'builtin-orange-tabby', 1, datetime('now'), datetime('now')
                 );",
            )
            .unwrap();
        connection
    }

    fn package_bytes(version: &str, image_suffix: u8) -> Vec<u8> {
        let mut image = b"\x89PNG\r\n\x1a\n".to_vec();
        image.push(image_suffix);
        let thumbnail = b"\x89PNG\r\n\x1a\nthumb".to_vec();
        let license = br#"{"license":"CC0-1.0"}"#.to_vec();
        let frame = json!({
            "frame": {"x": 0, "y": 0, "w": 64, "h": 64},
            "rotated": false,
            "source_size": {"w": 64, "h": 64},
            "sprite_source": {"x": 0, "y": 0, "w": 64, "h": 64}
        });
        let atlas = serde_json::to_vec(&json!({
            "schema_version": 1,
            "image": "pet.png",
            "size": {"w": 64, "h": 64},
            "frames": {
                "idle_000": frame,
                "walk_000": frame,
                "sleep_000": frame,
                "tap_000": frame
            }
        }))
        .unwrap();
        let files = [
            ("atlas/pet.png", &image),
            ("atlas/pet.json", &atlas),
            ("thumbnail.png", &thumbnail),
            ("license.json", &license),
        ]
        .into_iter()
        .map(|(path, bytes)| {
            json!({
                "path": path,
                "size": bytes.len(),
                "sha256": format!("{:x}", Sha256::digest(bytes))
            })
        })
        .collect::<Vec<_>>();
        let actions = ["idle", "walk", "sleep", "tap"]
            .into_iter()
            .map(|name| {
                (
                    name.to_owned(),
                    json!({
                        "frames": [format!("{name}_000")],
                        "frame_duration_ms": [100],
                        "loop": name != "tap",
                        "next_action": if name == "tap" { Some("idle") } else { None },
                        "fallback": if name == "idle" { None } else { Some("idle") }
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let manifest = serde_json::to_vec(&json!({
            "schema_version": 1,
            "package_version": version,
            "min_runtime_version": "0.2.0",
            "pet_id": "pet_storetest",
            "name": "Store Test",
            "species": "cat",
            "renderer": "sprite_atlas",
            "created_at": "2026-07-25T00:00:00Z",
            "canvas": {"width": 64, "height": 64},
            "atlas": {
                "image": "atlas/pet.png",
                "data": "atlas/pet.json",
                "max_texture_size": 4096
            },
            "default_scale": 1.0,
            "anchors": {"foot": [0.5, 0.9], "drag": [0.5, 0.3]},
            "hitboxes": [
                {"id": "body", "shape": "ellipse", "x": 0.1, "y": 0.1, "w": 0.8, "h": 0.8}
            ],
            "actions": actions,
            "generation": {
                "pipeline_version": "1.0.0",
                "template_version": "test-1"
            },
            "files": files
        }))
        .unwrap();

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, content) in [
            ("manifest.json", manifest),
            ("atlas/pet.png", image),
            ("atlas/pet.json", atlas),
            ("thumbnail.png", thumbnail),
            ("license.json", license),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(&content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn write_package(root: &Path, name: &str, version: &str, suffix: u8) -> PathBuf {
        let path = root.join(format!("{name}.epet"));
        fs::write(&path, package_bytes(version, suffix)).unwrap();
        path
    }

    #[test]
    fn installs_updates_retains_rolls_back_and_deletes_versions() {
        let temporary = tempdir().unwrap();
        let library = temporary.path().join("characters");
        let first_path = write_package(temporary.path(), "first", "1.0.0", 1);
        let second_path = write_package(temporary.path(), "second", "1.1.0", 2);
        let mut connection = database();

        let first = install_package(&mut connection, &library, &first_path, None, None).unwrap();
        assert_eq!(first.current_version.as_deref(), Some("1.0.0"));
        let first_hash = first.current_package_sha256.unwrap();

        let second = install_package(&mut connection, &library, &second_path, None, None).unwrap();
        assert_eq!(second.current_version.as_deref(), Some("1.1.0"));
        assert_eq!(second.versions.len(), 2);
        let second_hash = second.current_package_sha256.unwrap();
        assert!(
            library
                .join("pet_storetest/versions")
                .join(&first_hash)
                .is_dir()
        );
        assert!(
            library
                .join("pet_storetest/versions")
                .join(&second_hash)
                .is_dir()
        );
        let runtime = load_runtime_definition(&connection, &library, "pet_storetest").unwrap();
        assert_eq!(runtime.id, "pet_storetest");
        assert!(
            runtime
                .animation
                .image_url
                .starts_with("data:image/png;base64,")
        );
        assert_eq!(runtime.animation.actions["idle"].frames, ["idle_000"]);
        assert_eq!(runtime.hitboxes.len(), 1);

        let rolled_back =
            activate_version(&mut connection, &library, "pet_storetest", &first_hash).unwrap();
        assert_eq!(rolled_back.current_version.as_deref(), Some("1.0.0"));

        let after_delete =
            delete_version(&mut connection, &library, "pet_storetest", &second_hash).unwrap();
        assert_eq!(after_delete.versions.len(), 1);
        assert!(
            !library
                .join("pet_storetest/versions")
                .join(second_hash)
                .exists()
        );

        delete_character(
            &mut connection,
            &library,
            "pet_storetest",
            "builtin-orange-tabby",
        )
        .unwrap();
        assert!(!library.join("pet_storetest").exists());
        assert!(
            list_characters(&connection)
                .unwrap()
                .iter()
                .all(|character| character.id != "pet_storetest")
        );
    }

    #[test]
    fn index_failure_rolls_back_new_immutable_directory() {
        let temporary = tempdir().unwrap();
        let library = temporary.path().join("characters");
        let first_path = write_package(temporary.path(), "first", "1.0.0", 1);
        let conflicting_path = write_package(temporary.path(), "conflict", "1.0.0", 9);
        let mut connection = database();
        let first = install_package(&mut connection, &library, &first_path, None, None).unwrap();

        let conflicting_hash = load_epet(&conflicting_path, None).unwrap().package_sha256;
        let error =
            install_package(&mut connection, &library, &conflicting_path, None, None).unwrap_err();
        assert!(error.to_string().contains("拒绝覆盖不可变版本"));
        assert!(
            !library
                .join("pet_storetest/versions")
                .join(conflicting_hash)
                .exists()
        );
        let indexed = get_character(&connection, "pet_storetest").unwrap();
        assert_eq!(indexed.current_package_sha256, first.current_package_sha256);
        assert_eq!(indexed.versions.len(), 1);
    }

    #[test]
    fn remote_download_requires_https_and_no_embedded_credentials() {
        for url in [
            "http://example.com/character.epet",
            "https://user:secret@example.com/character.epet",
            "https://example.com/character.epet#fragment",
        ] {
            assert!(validate_download_url(url).is_err());
        }
        assert!(validate_download_url("https://example.com/character.epet?token=short").is_ok());
    }

    #[test]
    fn startup_cleanup_only_removes_staging_downloads_and_unindexed_versions() {
        let temporary = tempdir().unwrap();
        let library = temporary.path().join("characters");
        let downloads = temporary.path().join("downloads");
        let package_path = write_package(temporary.path(), "first", "1.0.0", 1);
        let mut connection = database();
        let installed =
            install_package(&mut connection, &library, &package_path, None, None).unwrap();
        let installed_hash = installed.current_package_sha256.unwrap();

        fs::create_dir_all(library.join(".staging/interrupted")).unwrap();
        fs::create_dir_all(&downloads).unwrap();
        fs::write(downloads.join("interrupted.epet"), b"partial").unwrap();
        fs::write(downloads.join("keep.txt"), b"unrelated").unwrap();
        let orphan_hash = "f".repeat(64);
        fs::create_dir_all(library.join("pet_storetest/versions").join(&orphan_hash)).unwrap();
        let installed_directory = library.join("pet_storetest/versions").join(&installed_hash);
        let interrupted_delete = library
            .join(".trash/versions/pet_storetest")
            .join(&installed_hash);
        fs::create_dir_all(interrupted_delete.parent().unwrap()).unwrap();
        fs::rename(&installed_directory, &interrupted_delete).unwrap();
        let committed_delete = library
            .join(".trash/versions/pet_storetest")
            .join("e".repeat(64));
        fs::create_dir_all(&committed_delete).unwrap();

        cleanup_stale_storage_with_age(&connection, &library, &downloads, Duration::ZERO).unwrap();

        assert!(!library.join(".staging/interrupted").exists());
        assert!(!downloads.join("interrupted.epet").exists());
        assert!(downloads.join("keep.txt").exists());
        assert!(installed_directory.exists());
        assert!(!interrupted_delete.exists());
        assert!(!committed_delete.exists());
        assert!(
            !library
                .join("pet_storetest/versions")
                .join(orphan_hash)
                .exists()
        );
    }
}
