use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::models::{AppConfig, Note, RepoWatch, TodoList};
use crate::storage::Storage;

/// Version of the ZIP package format produced by this application. Bump this
/// (and add a migration) whenever the package layout changes; import rejects
/// packages with a higher version instead of silently dropping fields.
pub const PACKAGE_SCHEMA_VERSION: u32 = 1;

const BACKUP_SUBDIR: &str = "backups";
const BACKUP_PREFIX: &str = "maydolist-backup-";
/// Keep at most this many timestamped backups inside the data dir.
const MAX_BACKUPS: usize = 10;

/// Package-level manifest. Lives at the root of every export / backup ZIP.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    #[serde(default)]
    pub package_schema_version: u32,
    pub app_version: String,
    pub created_at: String,
    pub tool: String,
    pub summary: PackageSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub config: bool,
    pub notes: usize,
    pub todos: usize,
    pub github_watchlist: bool,
    pub github_cache: usize,
}

/// Content overview returned to the UI before an import is confirmed.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackagePreview {
    pub package_schema_version: u32,
    pub app_version: String,
    pub created_at: String,
    pub has_config: bool,
    pub has_watchlist: bool,
    pub notes: usize,
    pub todos: usize,
    pub github_cache: usize,
    /// Rebuildable cache files that failed validation and were skipped.
    pub skipped_cache: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportInfo {
    pub path: String,
    pub notes: usize,
    pub todos: usize,
    pub github_cache: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportInfo {
    pub path: String,
    pub backup_path: String,
    pub notes: usize,
    pub todos: usize,
    pub github_cache: usize,
    pub skipped_cache: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub created_at: String,
}

/// Backup / export / import / restore. Export packages contain config, Todo
/// lists, notes and the GitHub watchlist; the GitHub cache is optional and
/// rebuildable. Import validates the package in a staging directory, creates
/// an automatic backup of the current data, then swaps the domain files
/// atomically — a failed import never touches the current data.
pub struct BackupService {
    storage: Arc<Storage>,
}

impl BackupService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// Write a package ZIP to `target`. `include_cache` controls whether the
    /// rebuildable `github/cache` snapshots are included.
    pub fn export_to(&self, target: &Path, include_cache: bool) -> AppResult<ExportInfo> {
        let target = with_zip_extension(target);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let summary = write_package(&target, include_cache, &self.storage)?;
        Ok(ExportInfo {
            path: target.display().to_string(),
            notes: summary.notes,
            todos: summary.todos,
            github_cache: summary.github_cache,
        })
    }

    /// Validate a package and return a preview without touching current data.
    pub fn inspect(&self, package: &Path) -> AppResult<PackagePreview> {
        let staging =
            std::env::temp_dir().join(format!("maydolist-inspect-{}", uuid::Uuid::new_v4()));
        let result = validate_and_extract(package, &staging);
        fs::remove_dir_all(&staging).ok();
        result
    }

    /// Import a package: validate in a staging dir, auto-backup the current
    /// data, then atomically swap the domain files. The current data is never
    /// modified when validation or the swap fails.
    pub fn import_from(&self, package: &Path) -> AppResult<ImportInfo> {
        let data_dir = self.storage.data_dir();
        let parent = data_dir
            .parent()
            .ok_or_else(|| AppError::Internal("data directory has no parent".into()))?;
        let name = data_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let staging = parent.join(format!("{name}-import-{}", uuid::Uuid::new_v4()));

        let preview = match validate_and_extract(package, &staging) {
            Ok(preview) => preview,
            Err(err) => {
                fs::remove_dir_all(&staging).ok();
                return Err(err);
            }
        };
        if let Err(err) = normalize_staged_config(&staging, &data_dir) {
            fs::remove_dir_all(&staging).ok();
            return Err(err);
        }
        // A package with zero notes / todos / cache has no directory entries;
        // create the skeleton so the atomic swap always has the four entries.
        for sub in ["notes", "todos", "github/cache"] {
            fs::create_dir_all(staging.join(sub))?;
        }
        let backup = match self.create_backup() {
            Ok(backup) => backup,
            Err(err) => {
                fs::remove_dir_all(&staging).ok();
                return Err(err);
            }
        };
        if let Err(err) = self.storage.replace_domain(&staging) {
            fs::remove_dir_all(&staging).ok();
            return Err(err);
        }
        fs::remove_dir_all(&staging).ok();
        Ok(ImportInfo {
            path: package.display().to_string(),
            backup_path: backup.path,
            notes: preview.notes,
            todos: preview.todos,
            github_cache: preview.github_cache,
            skipped_cache: preview.skipped_cache,
        })
    }

    /// Create a timestamped backup ZIP inside `<data_dir>/backups` and prune
    /// old backups, keeping at most `MAX_BACKUPS`.
    pub fn create_backup(&self) -> AppResult<BackupInfo> {
        let backups = self.backup_dir();
        fs::create_dir_all(&backups)?;
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let path = backups.join(format!("{BACKUP_PREFIX}{stamp}.zip"));
        write_package(&path, true, &self.storage)?;
        let info = backup_info(&path)?;
        prune_backups(&backups)?;
        Ok(info)
    }

    /// Recent backups, newest first.
    pub fn list_backups(&self) -> AppResult<Vec<BackupInfo>> {
        let dir = self.backup_dir();
        if !dir.is_dir() {
            return Ok(vec![]);
        }
        let mut infos = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !name.starts_with(BACKUP_PREFIX) || !name.ends_with(".zip") {
                continue;
            }
            infos.push(backup_info(&path)?);
        }
        infos.sort_by(|a, b| b.name.cmp(&a.name));
        Ok(infos)
    }

    /// Open the data directory in the system file manager.
    pub fn open_data_dir(&self) -> AppResult<()> {
        let dir = self.storage.data_dir();
        #[cfg(windows)]
        {
            use std::process::Command;
            Command::new("explorer")
                .arg(&dir)
                .spawn()
                .map_err(|e| AppError::Internal(format!("failed to open data dir: {e}")))?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = dir;
            Err(AppError::InvalidInput("打开数据目录仅支持 Windows".into()))
        }
    }

    fn backup_dir(&self) -> PathBuf {
        self.storage.data_dir().join(BACKUP_SUBDIR)
    }
}

fn with_zip_extension(target: &Path) -> PathBuf {
    if target.extension().and_then(|v| v.to_str()) == Some("zip") {
        target.to_path_buf()
    } else {
        let mut path = target.as_os_str().to_os_string();
        path.push(".zip");
        PathBuf::from(path)
    }
}

fn write_package(
    target: &Path,
    include_cache: bool,
    storage: &Storage,
) -> AppResult<PackageSummary> {
    let data_dir = storage.data_dir();
    let notes_files = json_files(&data_dir.join("notes"));
    let todos_files = json_files(&data_dir.join("todos"));
    let cache_files = if include_cache {
        json_files(&data_dir.join("github/cache"))
    } else {
        vec![]
    };
    let watch_path = data_dir.join("github/watchlist.json");
    let has_watchlist = watch_path.is_file();
    let summary = PackageSummary {
        config: true,
        notes: notes_files.len(),
        todos: todos_files.len(),
        github_watchlist: has_watchlist,
        github_cache: cache_files.len(),
    };

    let file = File::create(target)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let manifest = PackageManifest {
        package_schema_version: PACKAGE_SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created_at: Utc::now().to_rfc3339(),
        tool: "maydolist".into(),
        summary,
    };
    add_entry(
        &mut zip,
        &options,
        "manifest.json",
        &serde_json::to_vec_pretty(&manifest)
            .map_err(|e| AppError::Storage(format!("serialize manifest: {e}")))?,
    )?;

    let config = storage.load_config()?;
    add_entry(
        &mut zip,
        &options,
        "config.json",
        &serde_json::to_vec_pretty(&config)
            .map_err(|e| AppError::Storage(format!("serialize config: {e}")))?,
    )?;
    add_json_files(&mut zip, &options, &notes_files, "notes")?;
    add_json_files(&mut zip, &options, &todos_files, "todos")?;
    if has_watchlist {
        add_entry(
            &mut zip,
            &options,
            "github/watchlist.json",
            &fs::read(&watch_path)?,
        )?;
    }
    add_json_files(&mut zip, &options, &cache_files, "github/cache")?;

    zip.finish()
        .map_err(|e| AppError::Storage(format!("finalize package: {e}")))?;
    Ok(manifest.summary)
}

fn add_entry(
    zip: &mut zip::ZipWriter<File>,
    options: &zip::write::SimpleFileOptions,
    name: &str,
    bytes: &[u8],
) -> AppResult<()> {
    zip.start_file(name, *options)
        .map_err(|e| AppError::Storage(format!("zip write {name}: {e}")))?;
    zip.write_all(bytes)
        .map_err(|e| AppError::Storage(format!("zip write {name}: {e}")))?;
    Ok(())
}

fn add_json_files(
    zip: &mut zip::ZipWriter<File>,
    options: &zip::write::SimpleFileOptions,
    files: &[PathBuf],
    prefix: &str,
) -> AppResult<()> {
    for path in files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        add_entry(zip, options, &format!("{prefix}/{name}"), &fs::read(path)?)?;
    }
    Ok(())
}

/// Sorted list of `.json` files directly inside `dir` (never recursive, so
/// `logs/` / `backups/` can never leak into a package).
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|v| v.to_str()) == Some("json") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Validate every entry of a package and extract the validated tree into
/// `staging`. Path traversal, absolute paths, unknown files, duplicate
/// entries, an unsupported package version and invalid core JSON are all
/// rejected; rebuildable GitHub cache files with invalid JSON are skipped.
fn validate_and_extract(package: &Path, staging: &Path) -> AppResult<PackagePreview> {
    let file =
        File::open(package).map_err(|e| AppError::InvalidInput(format!("无法打开导入包: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::InvalidInput(format!("不是有效的 ZIP 包: {e}")))?;

    let mut seen = HashSet::new();
    let mut manifest: Option<PackageManifest> = None;
    let mut has_config = false;
    let mut has_watchlist = false;
    let mut notes = 0usize;
    let mut todos = 0usize;
    let mut github_cache = 0usize;
    let mut skipped_cache = 0usize;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| AppError::InvalidInput(format!("读取导入包失败: {e}")))?;
        let raw_name = entry.name().to_string();
        let rel = normalize_entry_name(&raw_name)?;
        let key = rel.to_string_lossy().replace('\\', "/");
        if !seen.insert(key.clone()) {
            return Err(AppError::InvalidInput(format!("导入包包含重复条目: {key}")));
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| AppError::InvalidInput(format!("读取导入包失败: {e}")))?;

        if key == "manifest.json" {
            let parsed: PackageManifest = serde_json::from_slice(&bytes).map_err(|_| {
                AppError::InvalidInput("导入包 manifest.json 不是有效的 JSON".into())
            })?;
            validate_manifest(&parsed)?;
            manifest = Some(parsed);
            write_staged(staging, &rel, &bytes)?;
        } else if key == "config.json" {
            serde_json::from_slice::<AppConfig>(&bytes).map_err(|_| {
                AppError::InvalidInput("导入包 config.json 不是有效的配置文件".into())
            })?;
            has_config = true;
            write_staged(staging, &rel, &bytes)?;
        } else if key.starts_with("notes/") {
            validate_entity_name(&rel, "notes")?;
            serde_json::from_slice::<Note>(&bytes)
                .map_err(|_| AppError::InvalidInput(format!("导入包 {key} 不是有效的便签")))?;
            notes += 1;
            write_staged(staging, &rel, &bytes)?;
        } else if key.starts_with("todos/") {
            validate_entity_name(&rel, "todos")?;
            serde_json::from_slice::<TodoList>(&bytes)
                .map_err(|_| AppError::InvalidInput(format!("导入包 {key} 不是有效的待办列表")))?;
            todos += 1;
            write_staged(staging, &rel, &bytes)?;
        } else if key == "github/watchlist.json" {
            serde_json::from_slice::<Vec<RepoWatch>>(&bytes).map_err(|_| {
                AppError::InvalidInput("导入包 watchlist.json 不是有效的追踪列表".into())
            })?;
            has_watchlist = true;
            write_staged(staging, &rel, &bytes)?;
        } else if key.starts_with("github/cache/") {
            validate_cache_name(&rel)?;
            if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() {
                github_cache += 1;
                write_staged(staging, &rel, &bytes)?;
            } else {
                // Rebuildable cache: a corrupt file must not block a restore.
                skipped_cache += 1;
            }
        } else {
            return Err(AppError::InvalidInput(format!("导入包包含未知条目: {key}")));
        }
    }

    let manifest =
        manifest.ok_or_else(|| AppError::InvalidInput("导入包缺少 manifest.json".into()))?;
    if !has_config {
        return Err(AppError::InvalidInput("导入包缺少 config.json".into()));
    }
    Ok(PackagePreview {
        package_schema_version: manifest.package_schema_version,
        app_version: manifest.app_version,
        created_at: manifest.created_at,
        has_config,
        has_watchlist,
        notes,
        todos,
        github_cache,
        skipped_cache,
    })
}

fn validate_manifest(manifest: &PackageManifest) -> AppResult<()> {
    if manifest.package_schema_version == 0
        || manifest.package_schema_version > PACKAGE_SCHEMA_VERSION
    {
        return Err(AppError::InvalidInput(format!(
            "导入包 schema 版本 {} 不受支持（当前支持版本 {}）",
            manifest.package_schema_version, PACKAGE_SCHEMA_VERSION
        )));
    }
    Ok(())
}

/// Normalize a raw ZIP entry name to a safe relative path. Rejects absolute
/// paths, drive letters, `..` traversal, empty parts and backslash tricks.
fn normalize_entry_name(raw: &str) -> AppResult<PathBuf> {
    if raw.is_empty() {
        return Err(AppError::InvalidInput("导入包包含空条目名".into()));
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err(AppError::InvalidInput(format!("导入包包含非法路径: {raw}")));
    }
    let normalized = raw.replace('\\', "/");
    let bytes = normalized.as_bytes();
    if normalized.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(AppError::InvalidInput(format!("导入包包含非法路径: {raw}")));
    }
    let mut path = PathBuf::new();
    for part in normalized.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains('\\')
            || part.contains(':')
        {
            return Err(AppError::InvalidInput(format!("导入包包含非法路径: {raw}")));
        }
        path.push(part);
    }
    Ok(path)
}

fn validate_entity_name(rel: &Path, subdir: &str) -> AppResult<()> {
    let name = rel
        .file_name()
        .ok_or_else(|| AppError::InvalidInput(format!("导入包 {subdir} 条目非法")))?
        .to_string_lossy();
    let stem = name.strip_suffix(".json").unwrap_or(&name);
    if !name.ends_with(".json") || uuid::Uuid::parse_str(stem).is_err() {
        return Err(AppError::InvalidInput(format!(
            "导入包包含非法的 {subdir} 文件名: {name}"
        )));
    }
    Ok(())
}

fn validate_cache_name(rel: &Path) -> AppResult<()> {
    let name = rel
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("导入包 github/cache 条目非法".into()))?
        .to_string_lossy();
    let stem = name.strip_suffix(".json").unwrap_or(&name);
    if !name.ends_with(".json")
        || stem.is_empty()
        || !stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(AppError::InvalidInput(format!(
            "导入包包含非法的缓存文件名: {name}"
        )));
    }
    Ok(())
}

fn write_staged(staging: &Path, rel: &Path, bytes: &[u8]) -> AppResult<()> {
    let target = staging.join(rel);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, bytes)?;
    Ok(())
}

/// Rewrite the staged config so the imported `dataDir` always points at the
/// current data directory (the value in the package refers to the exporting
/// machine) and the schema version is current.
fn normalize_staged_config(staging: &Path, data_dir: &Path) -> AppResult<()> {
    let path = staging.join("config.json");
    let mut config: AppConfig = serde_json::from_slice(&fs::read(&path)?)
        .map_err(|_| AppError::InvalidInput("导入包 config.json 不是有效的配置文件".into()))?;
    config.sanitize();
    config.data_dir = data_dir.display().to_string();
    fs::write(
        &path,
        serde_json::to_vec_pretty(&config)
            .map_err(|e| AppError::Storage(format!("serialize config: {e}")))?,
    )?;
    Ok(())
}

fn backup_info(path: &Path) -> AppResult<BackupInfo> {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let created_at = name
        .strip_prefix(BACKUP_PREFIX)
        .and_then(|v| v.strip_suffix(".zip"))
        .and_then(|stamp| {
            chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S")
                .ok()
                .map(|v| v.format("%Y-%m-%d %H:%M:%S").to_string())
        })
        .unwrap_or_else(|| {
            fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| {
                    let dt: chrono::DateTime<Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M:%S").to_string()
                })
                .unwrap_or_default()
        });
    Ok(BackupInfo {
        name,
        path: path.display().to_string(),
        size,
        created_at,
    })
}

fn prune_backups(dir: &Path) -> AppResult<()> {
    let mut names: Vec<String> = fs::read_dir(dir)?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name.starts_with(BACKUP_PREFIX) && name.ends_with(".zip")).then_some(name)
        })
        .collect();
    names.sort_by(|a, b| b.cmp(a));
    for old in names.into_iter().skip(MAX_BACKUPS) {
        fs::remove_file(dir.join(old)).ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::WindowBounds;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("maydolist-backup-{}-{}", tag, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_note(id: &str) -> Note {
        Note {
            schema_version: 1,
            id: id.into(),
            title: format!("note {id}"),
            content: "content".into(),
            tags: vec![],
            color: "yellow".into(),
            pinned: false,
            floating: false,
            collapsed: false,
            always_on_top: false,
            window_bounds: Some(WindowBounds {
                x: 0.0,
                y: 0.0,
                width: 360.0,
                height: 280.0,
            }),
            deleted: false,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-01T00:00:00Z".into(),
        }
    }

    fn sample_todo_list(id: &str) -> TodoList {
        TodoList {
            schema_version: 1,
            id: id.into(),
            title: format!("list {id}"),
            kind: None,
            sort_order: 0,
            deleted: false,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-01T00:00:00Z".into(),
            items: vec![],
        }
    }

    fn sample_watchlist() -> Vec<RepoWatch> {
        vec![RepoWatch {
            full_name: "wynxing/MayDolist".into(),
            filters: vec!["mine".into()],
            collapsed: false,
            ignored: vec![],
            pinned: vec![],
            signal_filters: vec![],
        }]
    }

    fn populate(storage: &Storage) {
        storage
            .save_config(&AppConfig {
                theme: "dark".into(),
                ..Default::default()
            })
            .unwrap();
        storage
            .save_entity(
                "notes",
                "11111111-1111-4111-8111-111111111111",
                &sample_note("11111111-1111-4111-8111-111111111111"),
            )
            .unwrap();
        storage
            .save_entity(
                "notes",
                "22222222-2222-4222-8222-222222222222",
                &sample_note("22222222-2222-4222-8222-222222222222"),
            )
            .unwrap();
        storage
            .save_entity(
                "todos",
                "33333333-3333-4333-8333-333333333333",
                &sample_todo_list("33333333-3333-4333-8333-333333333333"),
            )
            .unwrap();
        storage
            .write_json(
                &storage.data_dir().join("github/watchlist.json"),
                &sample_watchlist(),
            )
            .unwrap();
        storage
            .write_json(
                &storage.data_dir().join("github/cache/wynxing_MayDolist.json"),
                &serde_json::json!({ "repo": "wynxing/MayDolist", "fetchedAt": "2026-08-01T00:00:00Z" }),
            )
            .unwrap();
    }

    fn zip_entries(path: &Path) -> Vec<String> {
        let file = File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect()
    }

    #[test]
    fn export_package_contains_manifest_and_expected_layout() {
        let dir = temp_dir("export-layout");
        let storage = Storage::with_dir(&dir).unwrap();
        populate(&storage);
        let zip_path = dir.join("out.zip");

        let info = BackupService::new(Arc::new(storage))
            .export_to(&zip_path, true)
            .unwrap();
        assert_eq!(info.notes, 2);
        assert_eq!(info.todos, 1);
        assert_eq!(info.github_cache, 1);

        let entries = zip_entries(&zip_path);
        assert!(entries.contains(&"manifest.json".to_string()));
        assert!(entries.contains(&"config.json".to_string()));
        assert!(entries.contains(&"notes/11111111-1111-4111-8111-111111111111.json".to_string()));
        assert!(entries.contains(&"todos/33333333-3333-4333-8333-333333333333.json".to_string()));
        assert!(entries.contains(&"github/watchlist.json".to_string()));
        assert!(entries.contains(&"github/cache/wynxing_MayDolist.json".to_string()));
        assert!(!entries.iter().any(|e| e.starts_with("logs/")));
        assert!(!entries.iter().any(|e| e.starts_with("backups/")));

        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut manifest_bytes = Vec::new();
        archive
            .by_name("manifest.json")
            .unwrap()
            .read_to_end(&mut manifest_bytes)
            .unwrap();
        let manifest: PackageManifest = serde_json::from_slice(&manifest_bytes).unwrap();
        assert_eq!(manifest.package_schema_version, PACKAGE_SCHEMA_VERSION);
        assert_eq!(manifest.tool, "maydolist");
        assert_eq!(manifest.summary.notes, 2);
        assert!(manifest.summary.github_watchlist);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_without_cache_omits_cache_entries() {
        let dir = temp_dir("export-no-cache");
        let storage = Storage::with_dir(&dir).unwrap();
        populate(&storage);
        let zip_path = dir.join("out-no-cache.zip");
        BackupService::new(Arc::new(storage))
            .export_to(&zip_path, false)
            .unwrap();
        let entries = zip_entries(&zip_path);
        assert!(!entries.iter().any(|e| e.starts_with("github/cache/")));
        assert!(entries.contains(&"github/watchlist.json".to_string()));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_appends_zip_extension_when_missing() {
        let dir = temp_dir("export-ext");
        let storage = Storage::with_dir(&dir).unwrap();
        populate(&storage);
        let path = dir.join("package");
        let info = BackupService::new(Arc::new(storage))
            .export_to(&path, true)
            .unwrap();
        assert!(info.path.ends_with(".zip"));
        assert!(Path::new(&info.path).is_file());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_and_import_roundtrip_restores_data() {
        let source_dir = temp_dir("roundtrip-source");
        let source = Storage::with_dir(&source_dir).unwrap();
        populate(&source);
        let zip_path = source_dir.join("backup.zip");
        BackupService::new(Arc::new(source))
            .export_to(&zip_path, true)
            .unwrap();

        let target_dir = temp_dir("roundtrip-target");
        let target_storage = Arc::new(Storage::with_dir(&target_dir).unwrap());
        let target = BackupService::new(target_storage.clone());
        let info = target.import_from(&zip_path).unwrap();
        assert_eq!(info.notes, 2);
        assert_eq!(info.todos, 1);
        assert_eq!(info.github_cache, 1);
        assert!(info.backup_path.ends_with(".zip"));

        let notes = target_storage.list_json::<Note>("notes").unwrap();
        assert_eq!(notes.len(), 2);
        let todos = target_storage.list_json::<TodoList>("todos").unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(
            target_storage
                .read_json::<Vec<RepoWatch>>(&target_dir.join("github/watchlist.json"))
                .unwrap()
                .unwrap()
                .len(),
            1
        );
        let config = target_storage.load_config().unwrap();
        assert_eq!(config.theme, "dark");
        assert_eq!(config.data_dir, target_dir.display().to_string());
        assert_eq!(config.schema_version, 2);
        fs::remove_dir_all(&source_dir).ok();
        fs::remove_dir_all(&target_dir).ok();
    }

    #[test]
    fn import_empty_package_succeeds_with_skeleton() {
        let source_dir = temp_dir("empty-source");
        let source = Storage::with_dir(&source_dir).unwrap();
        source
            .save_config(&AppConfig {
                theme: "light".into(),
                ..Default::default()
            })
            .unwrap();
        let zip_path = source_dir.join("empty.zip");
        let info = BackupService::new(Arc::new(source))
            .export_to(&zip_path, true)
            .unwrap();
        assert_eq!(info.notes, 0);
        assert_eq!(info.todos, 0);

        let target_dir = temp_dir("empty-target");
        let target_storage = Arc::new(Storage::with_dir(&target_dir).unwrap());
        let imported = BackupService::new(target_storage.clone())
            .import_from(&zip_path)
            .unwrap();
        assert_eq!(imported.notes, 0);
        assert_eq!(imported.todos, 0);
        assert!(target_storage
            .list_json::<Note>("notes")
            .unwrap()
            .is_empty());
        assert!(target_storage
            .list_json::<TodoList>("todos")
            .unwrap()
            .is_empty());
        assert_eq!(target_storage.load_config().unwrap().theme, "light");
        fs::remove_dir_all(&source_dir).ok();
        fs::remove_dir_all(&target_dir).ok();
    }

    #[test]
    fn inspect_reports_preview_and_rejects_newer_version() {
        let dir = temp_dir("inspect");
        let storage = Storage::with_dir(&dir).unwrap();
        populate(&storage);
        let zip_path = dir.join("pkg.zip");
        BackupService::new(Arc::new(storage))
            .export_to(&zip_path, true)
            .unwrap();

        let preview = BackupService::new(Arc::new(Storage::with_dir(&dir).unwrap()))
            .inspect(&zip_path)
            .unwrap();
        assert_eq!(preview.notes, 2);
        assert!(preview.has_config);
        assert!(preview.has_watchlist);

        // Rewrite manifest with an unsupported version.
        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut manifest_bytes = Vec::new();
        archive
            .by_name("manifest.json")
            .unwrap()
            .read_to_end(&mut manifest_bytes)
            .unwrap();
        let mut manifest: PackageManifest = serde_json::from_slice(&manifest_bytes).unwrap();
        manifest.package_schema_version = 99;
        let out = File::create(dir.join("newer.zip")).unwrap();
        let mut writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec_pretty(&manifest).unwrap())
            .unwrap();
        writer.finish().unwrap();

        let err = BackupService::new(Arc::new(Storage::with_dir(&dir).unwrap()))
            .inspect(&dir.join("newer.zip"))
            .unwrap_err();
        assert!(err.to_string().contains("版本"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_rejects_package_without_manifest_and_keeps_data() {
        let dir = temp_dir("no-manifest");
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        populate(&storage);
        let original_notes = storage.list_json::<Note>("notes").unwrap().len();

        let bad = dir.join("bad.zip");
        let out = File::create(&bad).unwrap();
        let mut writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("config.json", options).unwrap();
        writer
            .write_all(
                serde_json::to_vec_pretty(&AppConfig::default())
                    .unwrap()
                    .as_slice(),
            )
            .unwrap();
        writer.finish().unwrap();

        let err = BackupService::new(storage.clone())
            .import_from(&bad)
            .unwrap_err();
        assert!(err.to_string().contains("manifest"));
        assert_eq!(
            storage.list_json::<Note>("notes").unwrap().len(),
            original_notes
        );
        assert_eq!(storage.load_config().unwrap().theme, "dark");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_rejects_path_traversal() {
        let dir = temp_dir("traversal");
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        populate(&storage);

        let bad = dir.join("evil.zip");
        let out = File::create(&bad).unwrap();
        let mut writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("../evil.json", options).unwrap();
        writer.write_all(b"{}").unwrap();
        writer.finish().unwrap();

        let err = BackupService::new(storage.clone())
            .import_from(&bad)
            .unwrap_err();
        assert!(err.to_string().contains("非法路径"));
        assert_eq!(storage.load_config().unwrap().theme, "dark");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_rejects_invalid_core_json() {
        let dir = temp_dir("bad-core");
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        populate(&storage);
        let original_notes = storage.list_json::<Note>("notes").unwrap().len();

        let bad = dir.join("bad-core.zip");
        let out = File::create(&bad).unwrap();
        let mut writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("notes/44444444-4444-4444-8444-444444444444.json", options)
            .unwrap();
        writer.write_all(b"{not valid json").unwrap();
        writer.finish().unwrap();

        let err = BackupService::new(storage.clone())
            .import_from(&bad)
            .unwrap_err();
        assert!(err.to_string().contains("便签"));
        assert_eq!(
            storage.list_json::<Note>("notes").unwrap().len(),
            original_notes
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_skips_corrupt_cache_and_restores_core() {
        let dir = temp_dir("bad-cache");
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        populate(&storage);

        let pkg = dir.join("pkg.zip");
        BackupService::new(storage.clone())
            .export_to(&pkg, true)
            .unwrap();
        // Rebuild the package with a corrupt cache entry.
        let file = File::open(&pkg).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let out = File::create(dir.join("with-corrupt-cache.zip")).unwrap();
        let mut writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if name == "github/cache/wynxing_MayDolist.json" {
                bytes = b"{broken".to_vec();
            }
            writer.start_file(name, options).unwrap();
            writer.write_all(&bytes).unwrap();
        }
        writer.finish().unwrap();

        let target_dir = temp_dir("bad-cache-target");
        let target = BackupService::new(Arc::new(Storage::with_dir(&target_dir).unwrap()));
        let info = target
            .import_from(&dir.join("with-corrupt-cache.zip"))
            .unwrap();
        assert_eq!(info.skipped_cache, 1);
        assert_eq!(info.github_cache, 0);
        let target_storage = Storage::with_dir(&target_dir).unwrap();
        assert_eq!(target_storage.list_json::<Note>("notes").unwrap().len(), 2);
        assert_eq!(
            target_storage.list_json::<TodoList>("todos").unwrap().len(),
            1
        );
        fs::remove_dir_all(&dir).ok();
        fs::remove_dir_all(&target_dir).ok();
    }

    #[test]
    fn import_rejects_duplicate_entries() {
        let dir = temp_dir("dupes");
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        populate(&storage);

        // The same normalized path spelled with forward and back slashes:
        // `ZipArchive` sees two distinct raw names, our normalization sees a
        // duplicate and must reject it.
        let bad = dir.join("dupes.zip");
        let out = File::create(&bad).unwrap();
        let mut writer = zip::ZipWriter::new(out);
        let options = zip::write::SimpleFileOptions::default();
        for name in ["github/cache/a.json", "github\\cache\\a.json"] {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"{}").unwrap();
        }
        writer.finish().unwrap();

        let err = BackupService::new(storage.clone())
            .import_from(&bad)
            .unwrap_err();
        assert!(err.to_string().contains("重复"), "unexpected error: {err}");
        assert_eq!(storage.load_config().unwrap().theme, "dark");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backup_creates_zip_and_prunes_old_ones() {
        let dir = temp_dir("prune");
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        populate(&storage);
        let service = BackupService::new(storage.clone());

        let first = service.create_backup().unwrap();
        assert!(first.path.ends_with(".zip"));
        assert!(Path::new(&first.path).is_file());
        assert_eq!(zip_entries(Path::new(&first.path)).len(), 7);

        // Create more backups than MAX_BACKUPS (names differ by uuid suffix).
        for _ in 0..MAX_BACKUPS + 2 {
            let backups = dir.join("backups");
            fs::create_dir_all(&backups).unwrap();
            fs::copy(
                &first.path,
                backups.join(format!(
                    "{BACKUP_PREFIX}{}-{}.zip",
                    Local::now().format("%Y%m%d-%H%M%S"),
                    uuid::Uuid::new_v4().simple()
                )),
            )
            .unwrap();
        }
        prune_backups(&dir.join("backups")).unwrap();
        let listed = service.list_backups().unwrap();
        assert_eq!(listed.len(), MAX_BACKUPS);
        let names: Vec<String> = listed.iter().map(|b| b.name.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(names, sorted, "backups must be newest first");
        fs::remove_dir_all(&dir).ok();
    }
}
