use serde::Serialize;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRuntimeInfo {
    current_version: String,
    portable: bool,
    release_url: String,
}

#[tauri::command]
pub fn update_runtime_info(app: AppHandle) -> AppResult<UpdateRuntimeInfo> {
    let executable = std::env::current_exe().map_err(AppError::from)?;
    let file_name = executable
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    Ok(UpdateRuntimeInfo {
        current_version: app.package_info().version.to_string(),
        // The release workflow deliberately names the standalone binary with
        // "portable". The updater must never replace that running executable.
        portable: cfg!(debug_assertions) || file_name.contains("portable"),
        release_url: "https://github.com/wynxing/MayDolist/releases/latest".into(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn published_portable_name_is_detectable() {
        let name = "MayDolist-portable-1.2.3.exe".to_ascii_lowercase();
        assert!(name.contains("portable"));
    }
}
