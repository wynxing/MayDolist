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
}

impl Storage {
    /// Resolve the data dir from the environment (or the default location) and
    /// create the directory skeleton.
    pub fn new() -> AppResult<Self> {
        let storage = Self {
            data_dir: Mutex::new(resolve_data_dir()),
            lock: Mutex::new(()),
        };
        storage.ensure_dirs()?;
        Ok(storage)
    }

    /// Test helper: build storage rooted at an explicit directory.
    pub fn with_dir<P: AsRef<Path>>(dir: P) -> AppResult<Self> {
        let storage = Self {
            data_dir: Mutex::new(dir.as_ref().to_path_buf()),
            lock: Mutex::new(()),
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
    pub fn load_config(&self) -> AppResult<AppConfig> {
        let path = self.config_path();
        match self.read_json::<AppConfig>(&path) {
            Ok(Some(mut config)) => {
                if config.data_dir.is_empty() {
                    config.data_dir = self.data_dir().display().to_string();
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
        self.write_json(&self.config_path(), config)
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
        write_bootstrap(&target)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::CONFIG_SCHEMA_VERSION;
    use std::sync::OnceLock;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "maydolist-storage-{}-{}",
            tag,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn first_load_creates_default_config_and_dirs() {
        let dir = temp_dir("first-load");
        let storage = Storage::with_dir(&dir).unwrap();
        let config = storage.load_config().unwrap();
        assert_eq!(config.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(config.hotkey, "Ctrl+Alt+M");
        assert!(storage.config_path().exists());
        for sub in DEFAULT_SUBDIRECTORIES {
            assert!(dir.join(sub).is_dir(), "missing dir: {sub}");
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn atomic_write_roundtrip() {
        let dir = temp_dir("roundtrip");
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
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_failure_preserves_original() {
        let dir = temp_dir("write-fail");
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
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn concurrent_writes_serialize() {
        let dir = temp_dir("concurrent");
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
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_config_is_quarantined_and_rebuilt() {
        let dir = temp_dir("corrupt");
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
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_data_dir_prefers_env() {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let custom = std::env::temp_dir().join("maydolist-env-test");
        std::env::set_var("MAYDOLIST_DATA_DIR", &custom);
        assert_eq!(resolve_data_dir(), custom);
        std::env::remove_var("MAYDOLIST_DATA_DIR");
        assert_ne!(resolve_data_dir(), custom);
        fs::remove_dir_all(&custom).ok();
    }
}
