use crate::error::AppResult;

/// Application-level setup. The main window is declared in `tauri.conf.json`.
/// v0.1 will wire the tray icon, global hotkey and hot-corner detection here.
pub fn setup(_app: &mut tauri::App) -> AppResult<()> {
    Ok(())
}
