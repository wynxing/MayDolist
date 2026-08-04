use crate::error::{AppError, AppResult};
use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
pub const ENTITY_CHANGED: &str = "entity-changed";
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityChangedPayload {
    pub domain: String,
    pub entity_id: String,
    pub operation: String,
    pub timestamp: String,
}
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}
pub fn emit_entity_changed(
    app: &AppHandle,
    domain: &str,
    id: &str,
    operation: &str,
) -> AppResult<()> {
    app.emit(
        ENTITY_CHANGED,
        EntityChangedPayload {
            domain: domain.into(),
            entity_id: id.into(),
            operation: operation.into(),
            timestamp: now_rfc3339(),
        },
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}
