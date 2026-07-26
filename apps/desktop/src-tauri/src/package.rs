use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Seek},
    path::Path,
};

use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::ZipArchive;

const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_COMPRESSED_BYTES: u64 = 30 * 1024 * 1024;
const MAX_UNCOMPRESSED_BYTES: u64 = 100 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FILES: usize = 100;
const MAX_COMPRESSION_RATIO: u64 = 20;

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("无法读取角色包：{0}")]
    Io(#[from] std::io::Error),
    #[error("角色包不是有效的 ZIP：{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("角色包 JSON 无效：{0}")]
    Json(#[from] serde_json::Error),
    #[error("角色包校验失败：{0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PetManifest {
    pub schema_version: u32,
    pub package_version: String,
    pub min_runtime_version: String,
    pub pet_id: String,
    pub name: String,
    pub species: String,
    pub renderer: String,
    pub created_at: String,
    pub canvas: CanvasSize,
    pub atlas: AtlasReference,
    pub default_scale: f64,
    pub anchors: Anchors,
    pub hitboxes: Vec<ManifestHitbox>,
    pub actions: HashMap<String, ManifestAction>,
    pub generation: Generation,
    pub files: Vec<ManifestFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasReference {
    pub image: String,
    pub data: String,
    pub max_texture_size: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anchors {
    pub foot: [f64; 2],
    pub drag: [f64; 2],
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestHitbox {
    pub id: String,
    pub shape: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestAction {
    pub frames: Vec<String>,
    pub frame_duration_ms: Vec<u64>,
    pub r#loop: bool,
    pub next_action: Option<String>,
    pub fallback: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    pub pipeline_version: String,
    pub template_version: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Atlas {
    pub schema_version: u32,
    pub image: String,
    pub size: AtlasSize,
    pub frames: HashMap<String, AtlasFrame>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasSize {
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtlasFrame {
    pub frame: Rect,
    pub rotated: bool,
    pub source_size: AtlasSize,
    pub sprite_source: Rect,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug)]
pub struct LoadedPackage {
    pub package_sha256: String,
    pub manifest: PetManifest,
    pub atlas: Atlas,
    files: HashMap<String, Vec<u8>>,
}

impl LoadedPackage {
    pub fn file(&self, package_path: &str) -> Option<&[u8]> {
        self.files.get(package_path).map(Vec::as_slice)
    }

    pub fn thumbnail_path(&self) -> Option<&str> {
        ["thumbnail.webp", "thumbnail.png"]
            .into_iter()
            .find(|path| self.files.contains_key(*path))
    }

    pub fn extract_to(&self, destination: &Path) -> Result<(), PackageError> {
        if destination.exists() {
            return invalid("解包目标已经存在");
        }
        std::fs::create_dir(destination)?;

        let result = (|| {
            for (package_path, content) in &self.files {
                validate_package_path(package_path)?;
                let mut output = destination.to_path_buf();
                for segment in package_path.split('/') {
                    output.push(segment);
                }
                if let Some(parent) = output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output)?;
                use std::io::Write;
                file.write_all(content)?;
                file.sync_all()?;
            }
            Ok(())
        })();

        if result.is_err() {
            let _ = std::fs::remove_dir_all(destination);
        }
        result
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub package_sha256: String,
    pub schema_version: u32,
    pub package_version: String,
    pub pet_id: String,
    pub name: String,
    pub action_names: Vec<String>,
    pub frame_count: usize,
    pub file_count: usize,
}

impl From<&LoadedPackage> for PackageSummary {
    fn from(package: &LoadedPackage) -> Self {
        let mut action_names = package.manifest.actions.keys().cloned().collect::<Vec<_>>();
        action_names.sort();
        Self {
            package_sha256: package.package_sha256.clone(),
            schema_version: package.manifest.schema_version,
            package_version: package.manifest.package_version.clone(),
            pet_id: package.manifest.pet_id.clone(),
            name: package.manifest.name.clone(),
            action_names,
            frame_count: package.atlas.frames.len(),
            file_count: package.files.len(),
        }
    }
}

pub fn load_epet(
    path: &Path,
    expected_sha256: Option<&str>,
) -> Result<LoadedPackage, PackageError> {
    if path.extension().and_then(|value| value.to_str()) != Some("epet") {
        return invalid("文件扩展名必须是 .epet");
    }

    let metadata = path.metadata()?;
    if !metadata.is_file() {
        return invalid("角色包路径不是普通文件");
    }
    if metadata.len() == 0 || metadata.len() > MAX_COMPRESSED_BYTES {
        return invalid(format!(
            "压缩包大小必须在 1 到 {MAX_COMPRESSED_BYTES} 字节之间"
        ));
    }

    let package_sha256 = hash_reader(File::open(path)?)?;
    if let Some(expected) = expected_sha256 {
        validate_sha256(expected, "包外 SHA-256")?;
        if !constant_time_eq(package_sha256.as_bytes(), expected.as_bytes()) {
            return invalid("包外 SHA-256 不匹配");
        }
    }

    let mut archive = ZipArchive::new(File::open(path)?)?;
    load_archive(&mut archive, package_sha256)
}

fn load_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    package_sha256: String,
) -> Result<LoadedPackage, PackageError> {
    if archive.is_empty() || archive.len() > MAX_FILES {
        return invalid(format!("文件数必须在 1 到 {MAX_FILES} 之间"));
    }

    let mut total_uncompressed = 0_u64;
    let mut canonical_names = HashSet::new();
    let mut files = HashMap::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        validate_archive_path(&name)?;
        if entry.is_dir() {
            return invalid(format!("不允许显式目录项：{name}"));
        }
        if entry.is_symlink() {
            return invalid(format!("不允许符号链接：{name}"));
        }

        let canonical = name.to_ascii_lowercase();
        if !canonical_names.insert(canonical) {
            return invalid(format!("文件路径重复或存在大小写冲突：{name}"));
        }
        validate_allowed_extension(&name)?;

        let size = entry.size();
        if size == 0 || size > MAX_FILE_BYTES {
            return invalid(format!("文件大小超限：{name}"));
        }
        total_uncompressed = total_uncompressed
            .checked_add(size)
            .ok_or_else(|| PackageError::Invalid("解压大小溢出".to_owned()))?;
        if total_uncompressed > MAX_UNCOMPRESSED_BYTES {
            return invalid("角色包解压后超过 100 MB");
        }

        let compressed = entry.compressed_size();
        if compressed == 0 || size > compressed.saturating_mul(MAX_COMPRESSION_RATIO) {
            return invalid(format!("文件压缩比超过 20:1：{name}"));
        }

        let capacity = usize::try_from(size)
            .map_err(|_| PackageError::Invalid(format!("文件大小无法分配：{name}")))?;
        let mut content = Vec::with_capacity(capacity);
        entry.read_to_end(&mut content)?;
        if content.len() as u64 != size {
            return invalid(format!("文件读取长度不一致：{name}"));
        }
        files.insert(name, content);
    }

    let manifest_bytes = files
        .get("manifest.json")
        .ok_or_else(|| PackageError::Invalid("缺少根目录 manifest.json".to_owned()))?;
    let manifest: PetManifest = serde_json::from_slice(manifest_bytes)?;
    validate_manifest(&manifest, &files)?;

    let atlas_bytes = files
        .get(&manifest.atlas.data)
        .ok_or_else(|| PackageError::Invalid("manifest 指定的 Atlas JSON 不存在".to_owned()))?;
    let atlas: Atlas = serde_json::from_slice(atlas_bytes)?;
    validate_atlas(&manifest, &atlas)?;
    validate_image_headers(&manifest, &files)?;

    Ok(LoadedPackage {
        package_sha256,
        manifest,
        atlas,
        files,
    })
}

fn validate_manifest(
    manifest: &PetManifest,
    files: &HashMap<String, Vec<u8>>,
) -> Result<(), PackageError> {
    if manifest.schema_version != 1 {
        return invalid("只支持 manifest schema_version 1");
    }
    let package_version = parse_version(&manifest.package_version, "package_version")?;
    let min_runtime = parse_version(&manifest.min_runtime_version, "min_runtime_version")?;
    let runtime = parse_version(RUNTIME_VERSION, "runtime version")?;
    if min_runtime > runtime {
        return invalid(format!("角色包需要运行时 {min_runtime}，当前为 {runtime}"));
    }
    if package_version.major == 0 && package_version.minor == 0 && package_version.patch == 0 {
        return invalid("package_version 不能是 0.0.0");
    }
    parse_version(
        &manifest.generation.pipeline_version,
        "generation.pipeline_version",
    )?;

    if !valid_identifier(&manifest.pet_id, "pet_", 8, 64) {
        return invalid("pet_id 不符合 pet_<8-64 个安全字符>");
    }
    if manifest.name.is_empty()
        || manifest.name.chars().count() > 64
        || manifest.name.trim() != manifest.name
        || manifest.name.chars().any(char::is_control)
    {
        return invalid("name 长度必须为 1-64 个字符");
    }
    if manifest.species != "cat" || manifest.renderer != "sprite_atlas" {
        return invalid("v1 仅支持 cat + sprite_atlas");
    }
    if manifest.created_at.is_empty()
        || manifest.created_at.len() > 40
        || !manifest.created_at.contains('T')
        || manifest.created_at.chars().any(char::is_control)
    {
        return invalid("created_at 必须是日期时间字符串");
    }
    validate_dimension(manifest.canvas.width, "canvas.width")?;
    validate_dimension(manifest.canvas.height, "canvas.height")?;
    validate_dimension(manifest.atlas.max_texture_size, "atlas.max_texture_size")?;
    validate_package_path(&manifest.atlas.image)?;
    validate_package_path(&manifest.atlas.data)?;
    if !(0.1..=3.0).contains(&manifest.default_scale) {
        return invalid("default_scale 必须在 0.1 到 3 之间");
    }
    validate_point(manifest.anchors.foot, "anchors.foot")?;
    validate_point(manifest.anchors.drag, "anchors.drag")?;
    if manifest.generation.template_version.is_empty()
        || manifest.generation.template_version.chars().count() > 64
        || manifest
            .generation
            .template_version
            .chars()
            .any(char::is_control)
    {
        return invalid("generation.template_version 长度必须为 1-64");
    }

    if manifest.hitboxes.is_empty() || manifest.hitboxes.len() > 16 {
        return invalid("hitboxes 数量必须为 1-16");
    }
    let mut hitbox_ids = HashSet::new();
    for hitbox in &manifest.hitboxes {
        if !valid_action_name(&hitbox.id) || !hitbox_ids.insert(hitbox.id.to_ascii_lowercase()) {
            return invalid(format!("无效或重复的碰撞区域 id：{}", hitbox.id));
        }
        if !matches!(hitbox.shape.as_str(), "rectangle" | "ellipse") {
            return invalid(format!("不支持的碰撞区域形状：{}", hitbox.shape));
        }
        if hitbox.x < 0.0
            || hitbox.y < 0.0
            || hitbox.w <= 0.0
            || hitbox.h <= 0.0
            || hitbox.x + hitbox.w > 1.0
            || hitbox.y + hitbox.h > 1.0
        {
            return invalid(format!("碰撞区域越过画布：{}", hitbox.id));
        }
    }

    if !manifest.actions.contains_key("idle")
        || manifest.actions.is_empty()
        || manifest.actions.len() > 32
    {
        return invalid("actions 必须包含 idle，且总数为 1-32");
    }
    for (name, action) in &manifest.actions {
        if !valid_action_name(name) {
            return invalid(format!("无效动作名：{name}"));
        }
        if action.frames.is_empty()
            || action.frames.len() > 240
            || action.frames.len() != action.frame_duration_ms.len()
        {
            return invalid(format!("动作 {name} 的帧与时长数量不一致"));
        }
        if action
            .frame_duration_ms
            .iter()
            .any(|duration| !(16..=10_000).contains(duration))
        {
            return invalid(format!("动作 {name} 存在无效帧时长"));
        }
        if !action.r#loop && action.next_action.is_none() && action.fallback.is_none() {
            return invalid(format!(
                "非循环动作 {name} 必须定义 next_action 或 fallback"
            ));
        }
        for target in [action.next_action.as_ref(), action.fallback.as_ref()]
            .into_iter()
            .flatten()
        {
            if !manifest.actions.contains_key(target) {
                return invalid(format!("动作 {name} 引用了不存在的动作：{target}"));
            }
        }
    }

    if manifest.files.len() < 4 || manifest.files.len() > 99 {
        return invalid("manifest.files 数量必须为 4-99");
    }
    let mut declared = HashSet::new();
    for file in &manifest.files {
        validate_package_path(&file.path)?;
        if file.path == "manifest.json" {
            return invalid("manifest.json 不得列入 files");
        }
        if !declared.insert(file.path.to_ascii_lowercase()) {
            return invalid(format!("manifest 中存在重复路径：{}", file.path));
        }
        if file.size == 0 || file.size > MAX_FILE_BYTES {
            return invalid(format!("manifest 文件大小无效：{}", file.path));
        }
        validate_sha256(&file.sha256, &file.path)?;
        let actual = files
            .get(&file.path)
            .ok_or_else(|| PackageError::Invalid(format!("缺少声明文件：{}", file.path)))?;
        if actual.len() as u64 != file.size {
            return invalid(format!("文件大小不匹配：{}", file.path));
        }
        let actual_hash = hash_bytes(actual);
        if !constant_time_eq(actual_hash.as_bytes(), file.sha256.as_bytes()) {
            return invalid(format!("文件 SHA-256 不匹配：{}", file.path));
        }
    }

    if files.len() != manifest.files.len() + 1 {
        return invalid("压缩包含有 manifest 未声明的额外文件");
    }
    for path in files.keys().filter(|path| path.as_str() != "manifest.json") {
        if !declared.contains(&path.to_ascii_lowercase()) {
            return invalid(format!("未声明文件：{path}"));
        }
    }
    for required in [
        manifest.atlas.image.as_str(),
        manifest.atlas.data.as_str(),
        "license.json",
    ] {
        if !files.contains_key(required) {
            return invalid(format!("缺少必需文件：{required}"));
        }
    }
    if ["thumbnail.webp", "thumbnail.png"]
        .into_iter()
        .filter(|path| files.contains_key(*path))
        .count()
        != 1
    {
        return invalid("角色包必须包含一个根目录 thumbnail.png 或 thumbnail.webp");
    }

    Ok(())
}

fn validate_atlas(manifest: &PetManifest, atlas: &Atlas) -> Result<(), PackageError> {
    if atlas.schema_version != 1 {
        return invalid("只支持 Atlas schema_version 1");
    }
    validate_dimension(atlas.size.w, "atlas.size.w")?;
    validate_dimension(atlas.size.h, "atlas.size.h")?;
    if atlas.size.w > manifest.atlas.max_texture_size
        || atlas.size.h > manifest.atlas.max_texture_size
    {
        return invalid("Atlas 尺寸超过 max_texture_size");
    }
    let expected_image_name = manifest.atlas.image.rsplit('/').next().unwrap_or_default();
    if atlas.image != expected_image_name {
        return invalid("Atlas JSON 的 image 与 manifest 不一致");
    }
    if atlas.frames.is_empty() || atlas.frames.len() > 10_000 {
        return invalid("Atlas 帧数量必须为 1-10000");
    }

    for (name, frame) in &atlas.frames {
        if !valid_frame_name(name) {
            return invalid(format!("无效帧名：{name}"));
        }
        if frame.rotated {
            return invalid(format!("v1 运行时不支持旋转帧：{name}"));
        }
        validate_rect_inside(&frame.frame, atlas.size.w, atlas.size.h, name)?;
        validate_dimension(frame.source_size.w, "frame.source_size.w")?;
        validate_dimension(frame.source_size.h, "frame.source_size.h")?;
        validate_rect_inside(
            &frame.sprite_source,
            frame.source_size.w,
            frame.source_size.h,
            name,
        )?;
    }
    for (action_name, action) in &manifest.actions {
        for frame in &action.frames {
            if !atlas.frames.contains_key(frame) {
                return invalid(format!("动作 {action_name} 缺少 Atlas 帧：{frame}"));
            }
        }
    }
    Ok(())
}

fn validate_image_headers(
    manifest: &PetManifest,
    files: &HashMap<String, Vec<u8>>,
) -> Result<(), PackageError> {
    for path in files
        .keys()
        .filter(|path| path.ends_with(".png") || path.ends_with(".webp"))
    {
        let bytes = &files[path];
        let valid = if path.ends_with(".png") {
            bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        } else {
            bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP"
        };
        if !valid {
            return invalid(format!("图片文件头无效：{path}"));
        }
    }
    if !files.contains_key(&manifest.atlas.image) {
        return invalid("Atlas 图片不存在");
    }
    Ok(())
}

fn validate_archive_path(path: &str) -> Result<(), PackageError> {
    validate_package_path(path)?;
    if path.contains('\\') || path.starts_with('/') || path.contains(':') {
        return invalid(format!("不安全的 ZIP 路径：{path}"));
    }
    Ok(())
}

fn validate_package_path(path: &str) -> Result<(), PackageError> {
    if path.is_empty()
        || path.len() > 240
        || path.contains('\0')
        || path.contains('\\')
        || path.starts_with('/')
        || path.contains("//")
        || path.contains("..")
        || path.contains(':')
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'/' | b'-'))
    {
        return invalid(format!("不安全的包内路径：{path}"));
    }
    if path
        .split('/')
        .any(|segment| segment.is_empty() || segment == ".")
    {
        return invalid(format!("不安全的包内路径：{path}"));
    }
    Ok(())
}

fn validate_allowed_extension(path: &str) -> Result<(), PackageError> {
    if path == "manifest.json"
        || path.ends_with(".json")
        || path.ends_with(".png")
        || path.ends_with(".webp")
    {
        Ok(())
    } else {
        invalid(format!("不允许的文件类型：{path}"))
    }
}

fn validate_sha256(value: &str, label: &str) -> Result<(), PackageError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return invalid(format!("{label} 不是小写十六进制 SHA-256"));
    }
    Ok(())
}

fn parse_version(value: &str, label: &str) -> Result<Version, PackageError> {
    Version::parse(value).map_err(|_| PackageError::Invalid(format!("{label} 不是有效 SemVer")))
}

fn validate_dimension(value: u32, label: &str) -> Result<(), PackageError> {
    if !(1..=4096).contains(&value) {
        return invalid(format!("{label} 必须在 1-4096"));
    }
    Ok(())
}

fn validate_point(point: [f64; 2], label: &str) -> Result<(), PackageError> {
    if point.into_iter().any(|value| !(0.0..=1.0).contains(&value)) {
        return invalid(format!("{label} 必须使用 0-1 归一化坐标"));
    }
    Ok(())
}

fn validate_rect_inside(
    rect: &Rect,
    width: u32,
    height: u32,
    label: &str,
) -> Result<(), PackageError> {
    if rect.w == 0
        || rect.h == 0
        || rect.x.checked_add(rect.w).is_none_or(|right| right > width)
        || rect
            .y
            .checked_add(rect.h)
            .is_none_or(|bottom| bottom > height)
    {
        return invalid(format!("帧矩形越界：{label}"));
    }
    Ok(())
}

fn valid_action_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && value.len() <= 32
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_frame_name(value: &str) -> bool {
    let Some((prefix, suffix)) = value.rsplit_once('_') else {
        return false;
    };
    valid_action_name(prefix)
        && suffix.len() == 3
        && suffix.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_identifier(value: &str, prefix: &str, min_body: usize, max_body: usize) -> bool {
    let Some(body) = value.strip_prefix(prefix) else {
        return false;
    };
    (min_body..=max_body).contains(&body.len())
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn hash_reader(mut reader: impl Read) -> Result<String, std::io::Error> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
}

fn invalid<T>(message: impl Into<String>) -> Result<T, PackageError> {
    Err(PackageError::Invalid(message.into()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Cursor, Write},
    };

    use serde_json::json;
    use tempfile::tempdir;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use super::*;

    const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n";

    fn make_package(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, content) in entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(&content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn valid_entries() -> Vec<(String, Vec<u8>)> {
        let atlas = serde_json::to_vec(&json!({
            "schema_version": 1,
            "image": "pet.png",
            "size": {"w": 64, "h": 64},
            "frames": {
                "idle_000": frame_json(),
                "walk_000": frame_json(),
                "sleep_000": frame_json(),
                "tap_000": frame_json()
            }
        }))
        .unwrap();
        let license = br#"{"license":"CC0-1.0"}"#.to_vec();
        let image = PNG_HEADER.to_vec();
        let thumbnail = PNG_HEADER.to_vec();
        let declared = [
            ("atlas/pet.png", &image),
            ("atlas/pet.json", &atlas),
            ("thumbnail.png", &thumbnail),
            ("license.json", &license),
        ];
        let files = declared
            .iter()
            .map(|(path, bytes)| {
                json!({
                    "path": path,
                    "size": bytes.len(),
                    "sha256": hash_bytes(bytes)
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
            "package_version": "1.0.0",
            "min_runtime_version": "0.2.0",
            "pet_id": "pet_example01",
            "name": "Example Cat",
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
            "generation": {"pipeline_version": "1.0.0", "template_version": "cat-a-1"},
            "files": files
        }))
        .unwrap();

        vec![
            ("manifest.json".to_owned(), manifest),
            ("atlas/pet.png".to_owned(), image),
            ("atlas/pet.json".to_owned(), atlas),
            ("thumbnail.png".to_owned(), thumbnail),
            ("license.json".to_owned(), license),
        ]
    }

    fn frame_json() -> serde_json::Value {
        json!({
            "frame": {"x": 0, "y": 0, "w": 64, "h": 64},
            "rotated": false,
            "source_size": {"w": 64, "h": 64},
            "sprite_source": {"x": 0, "y": 0, "w": 64, "h": 64}
        })
    }

    fn write_package(entries: Vec<(String, Vec<u8>)>) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempdir().unwrap();
        let path = directory.path().join("character.epet");
        let borrowed = entries
            .iter()
            .map(|(name, content)| (name.as_str(), content.clone()))
            .collect();
        fs::write(&path, make_package(borrowed)).unwrap();
        (directory, path)
    }

    #[test]
    fn loads_valid_package_and_actions() {
        let (_directory, path) = write_package(valid_entries());
        let package = load_epet(&path, None).unwrap();
        assert_eq!(package.manifest.pet_id, "pet_example01");
        assert_eq!(package.atlas.frames.len(), 4);
        assert!(package.files.contains_key("atlas/pet.png"));
        assert_eq!(
            PackageSummary::from(&package).action_names,
            vec!["idle", "sleep", "tap", "walk"]
        );
    }

    #[test]
    fn rejects_package_hash_mismatch() {
        let (_directory, path) = write_package(valid_entries());
        let error = load_epet(&path, Some(&"0".repeat(64))).unwrap_err();
        assert!(error.to_string().contains("包外 SHA-256 不匹配"));
    }

    #[test]
    fn rejects_traversal_and_case_conflicts() {
        for extra_name in ["../evil.json", "ATLAS/PET.JSON"] {
            let mut entries = valid_entries();
            entries.push((extra_name.to_owned(), b"{}".to_vec()));
            let (_directory, path) = write_package(entries);
            let error = load_epet(&path, None).unwrap_err();
            assert!(
                error.to_string().contains("不安全") || error.to_string().contains("大小写冲突")
            );
        }
    }

    #[test]
    fn rejects_undeclared_and_tampered_files() {
        let mut extra = valid_entries();
        extra.push(("extra.json".to_owned(), b"{}".to_vec()));
        let (_directory, path) = write_package(extra);
        assert!(
            load_epet(&path, None)
                .unwrap_err()
                .to_string()
                .contains("额外文件")
        );

        let mut tampered = valid_entries();
        let image = tampered
            .iter_mut()
            .find(|(name, _)| name == "atlas/pet.png")
            .unwrap();
        image.1[7] ^= 1;
        let (_directory, path) = write_package(tampered);
        assert!(
            load_epet(&path, None)
                .unwrap_err()
                .to_string()
                .contains("SHA-256 不匹配")
        );
    }

    #[test]
    fn rejects_unsupported_runtime_version_and_compression_bombs() {
        let mut future = valid_entries();
        let manifest_entry = future
            .iter_mut()
            .find(|(name, _)| name == "manifest.json")
            .unwrap();
        let mut manifest: serde_json::Value = serde_json::from_slice(&manifest_entry.1).unwrap();
        manifest["min_runtime_version"] = json!("99.0.0");
        manifest_entry.1 = serde_json::to_vec(&manifest).unwrap();
        let (_directory, path) = write_package(future);
        assert!(
            load_epet(&path, None)
                .unwrap_err()
                .to_string()
                .contains("需要运行时")
        );

        let (_directory, path) =
            write_package(vec![("manifest.json".to_owned(), vec![0_u8; 4096])]);
        assert!(
            load_epet(&path, None)
                .unwrap_err()
                .to_string()
                .contains("压缩比超过")
        );
    }

    #[test]
    fn rejects_symbolic_links() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("character.epet");
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink("manifest.json", "target.json", SimpleFileOptions::default())
            .unwrap();
        fs::write(&path, writer.finish().unwrap().into_inner()).unwrap();
        assert!(
            load_epet(&path, None)
                .unwrap_err()
                .to_string()
                .contains("符号链接")
        );
    }

    #[test]
    fn rejects_missing_frames_and_out_of_bounds_frames() {
        for mutation in ["missing", "bounds"] {
            let mut entries = valid_entries();
            let atlas_entry = entries
                .iter_mut()
                .find(|(name, _)| name == "atlas/pet.json")
                .unwrap();
            let mut atlas: serde_json::Value = serde_json::from_slice(&atlas_entry.1).unwrap();
            if mutation == "missing" {
                atlas["frames"].as_object_mut().unwrap().remove("walk_000");
            } else {
                atlas["frames"]["walk_000"]["frame"]["x"] = json!(63);
            }
            atlas_entry.1 = serde_json::to_vec(&atlas).unwrap();

            let atlas_hash = hash_bytes(&atlas_entry.1);
            let atlas_size = atlas_entry.1.len();
            let manifest_entry = entries
                .iter_mut()
                .find(|(name, _)| name == "manifest.json")
                .unwrap();
            let mut manifest: serde_json::Value =
                serde_json::from_slice(&manifest_entry.1).unwrap();
            let declaration = manifest["files"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|file| file["path"] == "atlas/pet.json")
                .unwrap();
            declaration["size"] = json!(atlas_size);
            declaration["sha256"] = json!(atlas_hash);
            manifest_entry.1 = serde_json::to_vec(&manifest).unwrap();

            let (_directory, path) = write_package(entries);
            let error = load_epet(&path, None).unwrap_err().to_string();
            assert!(error.contains("缺少 Atlas 帧") || error.contains("帧矩形越界"));
        }
    }
}
