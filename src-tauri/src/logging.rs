use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// Minimal append-only file logger writing to `<data_dir>/logs/app.log`.
pub struct Logger {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Logger {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            path: dir.join("app.log"),
            lock: Mutex::new(()),
        }
    }

    pub fn log(&self, level: &str, message: &str) {
        let _guard = self.lock.lock().ok();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(file, "{} [{level}] {message}", crate::events::now_rfc3339());
        }
    }
}
