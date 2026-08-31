use serde::Serialize;
use ts_rs::TS;

use super::TodoSource;

/// One command offered by the global command palette. The static list and its
/// matching keywords live in Rust so ordering and matching stay testable and
/// consistent across windows; the frontend only renders what it receives.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaletteCommand {
    /// Stable command id consumed by the frontend action dispatcher.
    pub id: String,
    /// Display label shown in the palette list.
    pub label: String,
    /// Short hint describing what the command does.
    pub hint: String,
    /// Search keywords (Chinese + English aliases).
    pub keywords: Vec<String>,
}

/// One incomplete Todo item matched by the palette search.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaletteTodo {
    pub id: String,
    pub title: String,
    pub list_id: String,
    pub list_title: String,
    /// True when the owning list is the system capture inbox.
    pub inbox: bool,
    pub updated_at: String,
    /// Optional GitHub source for the "open source" action.
    pub source: Option<TodoSource>,
    /// Optional due date (`YYYY-MM-DD` or RFC3339), carried for display.
    pub due_date: Option<String>,
}

/// One Note matched by the palette search (title or full-text content).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaletteNote {
    pub id: String,
    pub title: String,
    /// Truncated first-line preview of the note content.
    pub preview: String,
    pub pinned: bool,
    pub floating: bool,
    pub updated_at: String,
}

/// One cached GitHub issue / PR matched by the palette search. Only local
/// snapshot caches are read; the palette never triggers network requests.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaletteGithub {
    /// `"pr"` or `"issue"`.
    pub kind: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub updated_at: String,
}

/// Read-only aggregation of command matches and per-domain search results.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PaletteSearchResult {
    pub query: String,
    pub commands: Vec<PaletteCommand>,
    pub todos: Vec<PaletteTodo>,
    pub notes: Vec<PaletteNote>,
    pub github: Vec<PaletteGithub>,
    /// True when GitHub results come from a stale / offline local cache.
    pub github_offline: bool,
}
