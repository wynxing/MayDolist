use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ActionSignal, GithubSyncMetadata, RepeatRule, TodoSource};

/// Per-section load state of the Focus read-only projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum FocusSectionState {
    Ready,
    Error,
}

/// One incomplete Todo item shown in the Focus view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusTodo {
    pub id: String,
    pub title: String,
    pub list_id: String,
    pub list_title: String,
    /// True when the owning list is the system capture inbox (`kind=inbox`).
    pub inbox: bool,
    pub updated_at: String,
    /// Optional external source (GitHub PR / issue) for the "open source"
    /// action in the Focus view; `None` for plain todos.
    pub source: Option<TodoSource>,
    /// GitHub source state when this Todo has been synchronized.
    pub github_sync: Option<GithubSyncMetadata>,
    /// Optional due date (`YYYY-MM-DD` or RFC3339); used for grouping.
    pub due_date: Option<String>,
    /// Optional reminder time (RFC3339) carried over for display.
    pub remind_at: Option<String>,
    /// Optional repeat rule carried over for display ("每 X 重复").
    pub repeat: Option<RepeatRule>,
}

/// One due-state group of the Focus todo section. Keys are stable:
/// `overdue` / `today` / `soon` / `none`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusTodoGroup {
    pub key: String,
    pub title: String,
    pub count: usize,
    pub items: Vec<FocusTodo>,
}

/// Todo section of the Focus projection, grouped by due state. Non-empty
/// groups only, in priority order (overdue → today → soon → none).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusTodoSection {
    pub state: FocusSectionState,
    pub error: Option<String>,
    /// Total incomplete todos before the display cap.
    pub total: usize,
    pub groups: Vec<FocusTodoGroup>,
}

/// One pinned or recently-updated Note shown in the Focus view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusNote {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub floating: bool,
    pub updated_at: String,
    /// Truncated first-line preview of the note content.
    pub preview: String,
}

/// One open GitHub issue or pull request that needs action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusGithub {
    /// `"pr"` or `"issue"` — the source kind for display and dedup.
    pub kind: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub draft: bool,
    pub url: String,
    pub updated_at: String,
    pub pinned: bool,
    pub matches: Vec<String>,
    /// Stable action signals of the source item (empty for old caches).
    pub signals: Vec<ActionSignal>,
}

/// One Focus section (todo / note / github) with per-domain state so a single
/// failing domain never blocks the others.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusSection<T> {
    pub state: FocusSectionState,
    /// Human-readable failure message. `Some` with items present means a
    /// partial failure (e.g. one snapshot could not be read); the UI shows
    /// both the cached items and the banner.
    pub error: Option<String>,
    /// Matched item count before the display cap, so the UI can offer
    /// "and N more — open the module".
    pub total: usize,
    /// True when GitHub shows cached/stale data (offline, unauthenticated or a
    /// previous refresh failed).
    pub offline_cache: bool,
    pub items: Vec<T>,
}

/// Read-only aggregation of todos, notes and GitHub items for the Focus view.
/// It never writes back into the domain stores; domain file formats stay
/// untouched.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusOverview {
    pub generated_at: String,
    pub todo: FocusTodoSection,
    pub note: FocusSection<FocusNote>,
    pub github: FocusSection<FocusGithub>,
}
