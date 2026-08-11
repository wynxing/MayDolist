use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Optional source reference linking a Todo item back to an external item
/// (MVP: GitHub issues and pull requests). Old Todo data without the field
/// reads as `None` and behaves like a normal Todo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoSource {
    /// `"github-issue"` or `"github-pr"`, serialized as `type` per the #19
    /// data contract so the frontend reads `source.type`.
    #[serde(rename = "type")]
    pub kind: String,
    /// `owner/repo` of the source repository.
    pub repo: String,
    /// GitHub issue / PR number.
    pub number: u64,
    /// Canonical URL of the source item. Only http / https are allowed.
    pub url: String,
}

impl TodoSource {
    /// Validate a source reference before it is persisted. Only the MVP
    /// GitHub kinds are accepted, and the URL must be http / https so the
    /// "open source" action can never launch a non-browser scheme.
    pub fn validate(&self) -> AppResult<()> {
        if self.kind != "github-issue" && self.kind != "github-pr" {
            return Err(AppError::InvalidInput(format!(
                "unsupported source type: {}",
                self.kind
            )));
        }
        if self.repo.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "source repo must not be empty".into(),
            ));
        }
        if self.number == 0 {
            return Err(AppError::InvalidInput(
                "source number must be positive".into(),
            ));
        }
        let parsed = url::Url::parse(&self.url)
            .map_err(|_| AppError::InvalidInput("invalid source url".into()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::InvalidInput(
                "source url must be http or https".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub completed: bool,
    /// Soft-delete flag: the row is kept on disk but filtered out of lists.
    pub deleted: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
    /// Optional external source (e.g. GitHub PR / issue). Old data without
    /// the field reads as `None`; items without a source keep the old JSON
    /// shape (field is skipped when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<TodoSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    /// Optional stable marker for system-managed lists (e.g. "inbox"). Old
    /// data without the field reads as None and behaves like a normal list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub sort_order: i32,
    pub deleted: bool,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<TodoItem>,
}
