use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, AppResult};

pub const EVENT_DATA_CHANGED: &str = "data-changed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataChangedPayload {
    pub domain: String,
    pub timestamp: String,
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Broadcast a data change to every open window after a successful write.
pub fn emit_data_changed(app: &AppHandle, domain: &str) -> AppResult<()> {
    let payload = DataChangedPayload {
        domain: domain.to_string(),
        timestamp: now_rfc3339(),
    };
    app.emit(EVENT_DATA_CHANGED, payload)
        .map_err(|e| AppError::Internal(format!("failed to emit event: {e}")))?;
    Ok(())
}
