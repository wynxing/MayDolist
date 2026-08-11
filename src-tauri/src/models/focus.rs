use serde::{Deserialize, Serialize};

use super::{ActionSignal, TodoSource};

/// Per-section load state of the Focus read-only projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FocusSectionState {
    Ready,
    Error,
}

/// One incomplete Todo item shown in the Focus view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
}

/// One pinned or recently-updated Note shown in the Focus view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FocusOverview {
    pub generated_at: String,
    pub todo: FocusSection<FocusTodo>,
    pub note: FocusSection<FocusNote>,
    pub github: FocusSection<FocusGithub>,
}
