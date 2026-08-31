use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::models::AppConfig;

/// Sub-directories created inside the data dir, matching docs/architecture.md.
pub const DEFAULT_SUBDIRECTORIES: &[&str] = &["notes", "todos", "github", "github/cache", "logs"];

/// File-system storage. All writes are serialized by an internal mutex and
/// performed atomically (temp file + rename) so a crash never leaves a
/// half-written JSON file.
pub struct Storage {
    data_dir: Mutex<PathBuf>,
    lock: Mutex<()>,
    /// In-memory copy of `config.json`. Background loops (reminders, GitHub
    /// polling, hot corner) read the config on every tick; the cache spares
    /// the disk read + parse. Refreshed by every `save_config`, dropped when
    /// the file is swapped from outside (import / migrate).
    config_cache: Mutex<Option<AppConfig>>,
}

impl Storage {
    /// Resolve the data dir from the environment (or the default location) and
    /// create the directory skeleton.
    pub fn new() -> AppResult<Self> {
        let storage = Self {
            data_dir: Mutex::new(resolve_data_dir()),
            lock: Mutex::new(()),
            config_cache: Mutex::new(None),
        };
        storage.ensure_dirs()?;
        Ok(storage)
    }

    /// Test helper: build storage rooted at an explicit directory.
    pub fn with_dir<P: AsRef<Path>>(dir: P) -> AppResult<Self> {
        let storage = Self {
            data_dir: Mutex::new(dir.as_ref().to_path_buf()),
            lock: Mutex::new(()),
            config_cache: Mutex::new(None),
        };
        storage.ensure_dirs()?;
        Ok(storage)
    }

    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.lock().expect("data dir lock").clone()
    }

    pub fn config_path(&self) -> PathBuf {
        self.data_dir().join("config.json")
    }

    pub fn ensure_dirs(&self) -> AppResult<()> {
        let data_dir = self.data_dir();
        fs::create_dir_all(&data_dir)?;
        for sub in DEFAULT_SUBDIRECTORIES {
            fs::create_dir_all(data_dir.join(sub))?;
        }
        Ok(())
    }

    /// Load the config, creating the default file on first run. A corrupt
    /// config is quarantined (renamed aside) and rebuilt from defaults.
    /// Served from the in-memory cache after the first successful load.
    pub fn load_config(&self) -> AppResult<AppConfig> {
        let cached = self
            .config_cache
            .lock()
            .map_err(|_| AppError::Internal("config cache lock poisoned".into()))?
            .clone();
        if let Some(config) = cached {
            return Ok(config);
        }
        let config = self.load_config_from_disk()?;
        let mut guard = self
            .config_cache
            .lock()
            .map_err(|_| AppError::Internal("config cache lock poisoned".into()))?;
        if guard.is_none() {
            *guard = Some(config.clone());
        }
        Ok(config)
    }

    fn load_config_from_disk(&self) -> AppResult<AppConfig> {
        let path = self.config_path();
        match self.read_json::<AppConfig>(&path) {
            Ok(Some(mut config)) => {
                if config.data_dir.is_empty() {
                    config.data_dir = self.data_dir().display().to_string();
                    self.save_config(&config)?;
                }
                if config.sanitize() {
                    self.save_config(&config)?;
                }
                Ok(config)
            }
            Ok(None) => {
                let config = AppConfig {
                    data_dir: self.data_dir().display().to_string(),
                    ..Default::default()
                };
                self.save_config(&config)?;
                Ok(config)
            }
            Err(AppError::CorruptFile { .. }) => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let backup = self.data_dir().join(format!("config.json.corrupt-{ts}"));
                fs::rename(&path, &backup).ok();
                let config = AppConfig {
                    data_dir: self.data_dir().display().to_string(),
                    ..Default::default()
                };
                self.save_config(&config)?;
                Ok(config)
            }
            Err(err) => Err(err),
        }
    }

    pub fn save_config(&self, config: &AppConfig) -> AppResult<()> {
        self.write_json(&self.config_path(), config)?;
        // Cache what a disk reload would produce: an out-of-range value gets
        // clamped by sanitize on load, so mirror that here instead of serving
        // the raw written value.
        let mut cached = config.clone();
        cached.sanitize();
        let mut guard = self
            .config_cache
            .lock()
            .map_err(|_| AppError::Internal("config cache lock poisoned".into()))?;
        *guard = Some(cached);
        Ok(())
    }

    /// Drop the cached config. Required whenever `config.json` is replaced
    /// without going through `save_config` (backup import, data dir migrate).
    pub fn invalidate_config_cache(&self) {
        if let Ok(mut guard) = self.config_cache.lock() {
            *guard = None;
        }
    }

    pub fn list_json<T: DeserializeOwned>(&self, subdir: &str) -> AppResult<Vec<T>> {
        let dir = self.data_dir().join(subdir);
        fs::create_dir_all(&dir)?;
        let mut values = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            match self.read_json(&path) {
                Ok(Some(value)) => values.push(value),
                Ok(None) => {}
                Err(AppError::CorruptFile { .. }) => {
                    self.quarantine(&path)?;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(values)
    }

    pub fn entity_path(&self, subdir: &str, id: &str) -> AppResult<PathBuf> {
        if uuid::Uuid::parse_str(id).is_err() {
            return Err(AppError::InvalidInput("invalid entity id".into()));
        }
        Ok(self.data_dir().join(subdir).join(format!("{id}.json")))
    }

    pub fn save_entity<T: Serialize>(&self, subdir: &str, id: &str, value: &T) -> AppResult<()> {
        self.write_json(&self.entity_path(subdir, id)?, value)
    }

    pub fn delete_entity(&self, subdir: &str, id: &str) -> AppResult<()> {
        let path = self.entity_path(subdir, id)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn quarantine(&self, path: &Path) -> AppResult<PathBuf> {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let name = format!(
            "{}.corrupt-{ts}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        let target = path.with_file_name(name);
        fs::rename(path, &target)?;
        Ok(target)
    }

    pub fn migrate(&self, target: &Path) -> AppResult<()> {
        let source = self.data_dir();
        let target = target.to_path_buf();
        if source == target {
            return Ok(());
        }
        if target.exists() && fs::read_dir(&target)?.next().is_some() {
            return Err(AppError::InvalidInput(
                "target data directory must be empty".into(),
            ));
        }
        let staging = target.with_extension(format!("migrating-{}", uuid::Uuid::new_v4()));
        copy_tree(&source, &staging)?;
        validate_json_tree(&staging)?;
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        fs::rename(&staging, &target)?;
        *self
            .data_dir
            .lock()
            .map_err(|_| AppError::Internal("data dir lock poisoned".into()))? = target.clone();
        // The moved tree carries its own config.json; the cached copy refers
        // to the old directory.
        self.invalidate_config_cache();
        write_bootstrap(&target)?;
        Ok(())
    }

    /// Atomically replace the domain files (`config.json`, `notes/`, `todos/`,
    /// `github/`) with the content of `staging`, holding the write lock for
    /// the whole operation so no concurrent write can race the swap. `staging`
    /// must contain exactly those entries as a valid JSON tree. On failure the
    /// original tree is restored; `logs/` and `backups/` are never touched.
    pub fn replace_domain(&self, staging: &Path) -> AppResult<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| AppError::Internal("storage lock poisoned".into()))?;
        validate_json_tree(staging)?;
        const ENTRIES: [&str; 4] = ["config.json", "notes", "todos", "github"];
        for entry in ENTRIES {
            if !staging.join(entry).exists() {
                return Err(AppError::InvalidInput(format!(
                    "staging is missing required entry: {entry}"
                )));
            }
        }
        let current = self.data_dir();
        let parent = current
            .parent()
            .ok_or_else(|| AppError::Internal("data dir has no parent".into()))?;
        let rollback = parent.join(format!(
            "{}-rollback-{}",
            current.file_name().unwrap_or_default().to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&rollback)?;

        // Phase 1: move the current entries aside.
        let mut moved: Vec<(&str, PathBuf)> = Vec::new();
        for entry in ENTRIES {
            let from = current.join(entry);
            let to = rollback.join(entry);
            if from.exists() {
                if let Err(err) = fs::rename(&from, &to) {
                    restore_moved(&current, &moved);
                    return Err(err.into());
                }
                moved.push((entry, to));
            }
        }
        // Phase 2: move the staged entries into place. Any failure restores
        // every entry that was moved aside.
        let mut placed: Vec<&str> = Vec::new();
        for entry in ENTRIES {
            let from = staging.join(entry);
            let to = current.join(entry);
            if let Err(err) = fs::rename(&from, &to) {
                for entry in &placed {
                    remove_path(&current.join(entry));
                }
                restore_moved(&current, &moved);
                return Err(err.into());
            }
            placed.push(entry);
        }
        // Success: drop the aside copies (the caller made a full ZIP backup
        // before the swap).
        fs::remove_dir_all(&rollback).ok();
        // The swap replaced config.json behind the cache's back.
        self.invalidate_config_cache();
        Ok(())
    }

    pub fn read_json<T: DeserializeOwned>(&self, path: &Path) -> AppResult<Option<T>> {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        match serde_json::from_str(&raw) {
            Ok(value) => Ok(Some(value)),
            Err(_err) => Err(AppError::CorruptFile {
                path: path.display().to_string(),
            }),
        }
    }

    pub fn write_json<T: Serialize + ?Sized>(&self, path: &Path, value: &T) -> AppResult<()> {
        let json = serde_json::to_string_pretty(value)
            .map_err(|e| AppError::Storage(format!("serialize error: {e}")))?;
        self.write_atomic(path, json.as_bytes())
    }

    /// Atomic write: write to `<name>.tmp` in the same directory, then rename
    /// over the destination. Serialized by the storage mutex.
    pub fn write_atomic(&self, path: &Path, bytes: &[u8]) -> AppResult<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| AppError::Internal("storage lock poisoned".into()))?;
        let parent = path
            .parent()
            .ok_or_else(|| AppError::Storage(format!("invalid path: {}", path.display())))?;
        fs::create_dir_all(parent)?;
        let tmp_path = atomic_tmp_path(path);
        {
            let mut file = fs::File::create(&tmp_path)?;
            file.write_all(bytes)?;
        }
        replace_file(&tmp_path, path)?;
        Ok(())
    }
}

fn atomic_tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> AppResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let src: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let dst: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        MoveFileExW(
            PCWSTR(src.as_ptr()),
            PCWSTR(dst.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|e| AppError::Storage(e.to_string()))
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> AppResult<()> {
    fs::rename(source, target).map_err(Into::into)
}

/// Default data dir: `%USERPROFILE%\Documents\MayDolist`, overridable via
/// `MAYDOLIST_DATA_DIR` (also used by tests).
pub fn resolve_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MAYDOLIST_DATA_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Some(path) = read_bootstrap() {
        return path;
    }
    #[cfg(windows)]
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile).join("Documents").join("MayDolist");
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("MayDolist-data")
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    data_dir: String,
}

pub fn bootstrap_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.wynn.maydolist")
        .join("bootstrap.json")
}

fn read_bootstrap() -> Option<PathBuf> {
    let raw = fs::read_to_string(bootstrap_path()).ok()?;
    let boot: Bootstrap = serde_json::from_str(&raw).ok()?;
    let path = PathBuf::from(boot.data_dir);
    path.is_dir().then_some(path)
}

fn write_bootstrap(data_dir: &Path) -> AppResult<()> {
    let path = bootstrap_path();
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    let raw = serde_json::to_vec_pretty(&Bootstrap {
        data_dir: data_dir.display().to_string(),
    })
    .map_err(|e| AppError::Storage(e.to_string()))?;
    fs::write(path, raw)?;
    Ok(())
}

fn copy_tree(source: &Path, target: &Path) -> AppResult<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let to = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

fn validate_json_tree(root: &Path) -> AppResult<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            validate_json_tree(&path)?;
        } else if path.extension().and_then(|v| v.to_str()) == Some("json") {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str::<serde_json::Value>(&raw).map_err(|_| AppError::CorruptFile {
                path: path.display().to_string(),
            })?;
        }
    }
    Ok(())
}

fn restore_moved(current: &Path, moved: &[(&str, PathBuf)]) {
    for (entry, rollback_path) in moved.iter().rev() {
        fs::rename(rollback_path, current.join(entry)).ok();
    }
}

fn remove_path(path: &Path) {
    if path.is_dir() {
        fs::remove_dir_all(path).ok();
    } else {
        fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::{CONFIG_SCHEMA_VERSION, GLASS_OPACITY_MAX, GLASS_OPACITY_MIN};
    use std::sync::OnceLock;

    fn temp_dir(tag: &str) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::Builder::new()
            .prefix(&format!("maydolist-storage-{tag}-"))
            .tempdir()
            .unwrap();
        let dir = tmp.path().to_path_buf();
        (tmp, dir)
    }

    #[test]
    fn first_load_creates_default_config_and_dirs() {
        let (_tmp, dir) = temp_dir("first-load");
        let storage = Storage::with_dir(&dir).unwrap();
        let config = storage.load_config().unwrap();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.hotkey, "Ctrl+Alt+M");
        assert!(storage.config_path().exists());
        for sub in DEFAULT_SUBDIRECTORIES {
            assert!(dir.join(sub).is_dir(), "missing dir: {sub}");
        }
    }

    #[test]
    fn atomic_write_roundtrip() {
        let (_tmp, dir) = temp_dir("roundtrip");
        let storage = Storage::with_dir(&dir).unwrap();
        let config = AppConfig {
            theme: "light".into(),
            ..Default::default()
        };
        storage.save_config(&config).unwrap();
        let loaded = storage.load_config().unwrap();
        assert_eq!(loaded.theme, "light");
        // No temp file left behind.
        assert!(!dir.join("config.json.tmp").exists());
    }

    #[test]
    fn write_failure_preserves_original() {
        let (_tmp, dir) = temp_dir("write-fail");
        let storage = Storage::with_dir(&dir).unwrap();
        let original = "{\"schemaVersion\":1,\"dataDir\":null,\"hotCorner\":\"top-right\",\"hotkey\":\"Ctrl+Alt+M\",\"theme\":\"original\",\"githubRefreshIntervalMinutes\":30}";
        fs::write(storage.config_path(), original).unwrap();
        // Block the temp path with a directory so the atomic write fails.
        fs::create_dir(dir.join("config.json.tmp")).unwrap();
        let config = AppConfig {
            theme: "replaced".into(),
            ..Default::default()
        };
        assert!(storage.save_config(&config).is_err());
        let after = fs::read_to_string(storage.config_path()).unwrap();
        assert_eq!(after, original);
    }

    #[test]
    fn concurrent_writes_serialize() {
        let (_tmp, dir) = temp_dir("concurrent");
        let storage = std::sync::Arc::new(Storage::with_dir(&dir).unwrap());
        let mut handles = Vec::new();
        for i in 0..8 {
            let storage = storage.clone();
            handles.push(std::thread::spawn(move || {
                let config = AppConfig {
                    theme: format!("theme-{i}"),
                    ..Default::default()
                };
                storage.save_config(&config).unwrap();
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let loaded = storage.load_config().unwrap();
        assert!(
            (0..8).any(|i| loaded.theme == format!("theme-{i}")),
            "final theme was {}",
            loaded.theme
        );
        assert!(!dir.join("config.json.tmp").exists());
    }

    #[test]
    fn corrupt_config_is_quarantined_and_rebuilt() {
        let (_tmp, dir) = temp_dir("corrupt");
        let storage = Storage::with_dir(&dir).unwrap();
        fs::write(storage.config_path(), "{not valid json").unwrap();
        let config = storage.load_config().unwrap();
        assert_eq!(config.data_dir, dir.display().to_string());
        assert_eq!(config.theme, AppConfig::default().theme);
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("config.json.corrupt-")
            })
            .collect();
        assert_eq!(backups.len(), 1, "corrupt file should be quarantined");
    }

    #[test]
    fn config_cache_serves_saved_value_until_invalidated() {
        let (_tmp, dir) = temp_dir("config-cache");
        let storage = Storage::with_dir(&dir).unwrap();
        let mut config = storage.load_config().unwrap();
        config.triage_later_days = 9;
        storage.save_config(&config).unwrap();
        // Overwrite the file behind the cache's back: reads keep serving the
        // cached (saved) value until the cache is explicitly dropped.
        let mut external = config.clone();
        external.triage_later_days = 1;
        fs::write(
            storage.config_path(),
            serde_json::to_vec_pretty(&external).unwrap(),
        )
        .unwrap();
        assert_eq!(storage.load_config().unwrap().triage_later_days, 9);
        storage.invalidate_config_cache();
        assert_eq!(storage.load_config().unwrap().triage_later_days, 1);
    }

    #[test]
    fn resolve_data_dir_prefers_env() {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("maydolist-env-test");
        std::env::set_var("MAYDOLIST_DATA_DIR", &custom);
        assert_eq!(resolve_data_dir(), custom);
        std::env::remove_var("MAYDOLIST_DATA_DIR");
        assert_ne!(resolve_data_dir(), custom);
    }

    #[test]
    fn glass_opacity_fields_roundtrip() {
        let (_tmp, dir) = temp_dir("glass-roundtrip");
        let storage = Storage::with_dir(&dir).unwrap();
        let config = AppConfig {
            main_window_glass_opacity: 0.62,
            floating_note_glass_opacity: 0.44,
            ..Default::default()
        };
        storage.save_config(&config).unwrap();
        let loaded = storage.load_config().unwrap();
        assert_eq!(loaded.main_window_glass_opacity, 0.62);
        assert_eq!(loaded.floating_note_glass_opacity, 0.44);
    }

    #[test]
    fn out_of_range_opacity_is_clamped_on_load() {
        let (_tmp, dir) = temp_dir("glass-clamp");
        let storage = Storage::with_dir(&dir).unwrap();
        let config = AppConfig {
            main_window_glass_opacity: 0.05,
            floating_note_glass_opacity: 2.0,
            ..Default::default()
        };
        // Simulate an externally written (legacy / hand-edited) config file.
        fs::write(
            storage.config_path(),
            serde_json::to_vec_pretty(&config).unwrap(),
        )
        .unwrap();
        storage.invalidate_config_cache();
        let loaded = storage.load_config().unwrap();
        assert_eq!(loaded.main_window_glass_opacity, GLASS_OPACITY_MIN);
        assert_eq!(loaded.floating_note_glass_opacity, GLASS_OPACITY_MAX);
        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(storage.config_path()).unwrap()).unwrap();
        assert_eq!(persisted["mainWindowGlassOpacity"], GLASS_OPACITY_MIN);
    }

    #[test]
    fn old_schema_config_is_upgraded_without_losing_settings() {
        let (_tmp, dir) = temp_dir("old-schema");
        let storage = Storage::with_dir(&dir).unwrap();
        let legacy = r#"{
            "schemaVersion": 1,
            "dataDir": null,
            "hotCorner": "top-right",
            "hotkey": "Ctrl+Alt+M",
            "theme": "dark",
            "githubRefreshIntervalMinutes": 30,
            "autostart": false,
            "firstRun": true
        }"#;
        fs::write(storage.config_path(), legacy).unwrap();
        let config = storage.load_config().unwrap();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.theme, "dark");
        assert_eq!(config.hot_corner, "top-right");
        assert_eq!(config.hotkey, "Ctrl+Alt+M");
        assert!(config.main_window_glass_opacity > 0.0);
        assert!(config.floating_note_glass_opacity > 0.0);
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("config.json.corrupt-")
            })
            .collect();
        assert_eq!(
            backups.len(),
            0,
            "valid legacy config must not be quarantined"
        );
        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(storage.config_path()).unwrap()).unwrap();
        assert_eq!(persisted["schemaVersion"], CONFIG_SCHEMA_VERSION);
        assert!(persisted.get("mainWindowGlassOpacity").is_some());
    }

    #[test]
    fn replace_domain_swaps_entries_atomically() {
        let (_tmp, dir) = temp_dir("replace-ok");
        let storage = Storage::with_dir(&dir).unwrap();
        storage
            .save_config(&AppConfig {
                theme: "old".into(),
                ..Default::default()
            })
            .unwrap();

        let staging = dir.join("staging");
        for sub in ["notes", "todos", "github/cache"] {
            fs::create_dir_all(staging.join(sub)).unwrap();
        }
        fs::write(
            staging.join("config.json"),
            r#"{"schemaVersion":2,"dataDir":"x","hotCorner":"top-right","hotkey":"Ctrl+Alt+M","theme":"new","githubRefreshIntervalMinutes":30,"autostart":false,"firstRun":false}"#,
        )
        .unwrap();
        fs::write(staging.join("notes/note-a.json"), r#"{"id":"a"}"#).unwrap();
        fs::write(
            staging.join("github/cache/owner_repo.json"),
            r#"{"repo":"owner/repo"}"#,
        )
        .unwrap();

        storage.replace_domain(&staging).unwrap();
        let persisted: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(storage.config_path()).unwrap()).unwrap();
        assert_eq!(persisted["theme"], "new");
        assert!(dir.join("notes/note-a.json").exists());
        assert!(dir.join("github/cache/owner_repo.json").exists());
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains("rollback"))
            .collect();
        assert!(leftovers.is_empty(), "rollback dir must be removed");
    }

    #[test]
    fn replace_domain_failure_keeps_original_data() {
        let (_tmp, dir) = temp_dir("replace-fail");
        let storage = Storage::with_dir(&dir).unwrap();
        storage
            .save_config(&AppConfig {
                theme: "old".into(),
                ..Default::default()
            })
            .unwrap();

        // Staging missing a required entry.
        let missing = dir.join("staging-missing");
        fs::create_dir_all(&missing).unwrap();
        fs::write(missing.join("config.json"), "{}").unwrap();
        assert!(storage.replace_domain(&missing).is_err());
        assert_eq!(storage.load_config().unwrap().theme, "old");

        // Staging containing invalid JSON.
        let invalid = dir.join("staging-invalid");
        for sub in ["notes", "todos", "github"] {
            fs::create_dir_all(invalid.join(sub)).unwrap();
        }
        fs::write(invalid.join("config.json"), "{broken").unwrap();
        assert!(storage.replace_domain(&invalid).is_err());
        assert_eq!(storage.load_config().unwrap().theme, "old");
        assert!(
            fs::read_to_string(storage.config_path())
                .unwrap()
                .contains("old"),
            "original config must be untouched"
        );
    }
}
