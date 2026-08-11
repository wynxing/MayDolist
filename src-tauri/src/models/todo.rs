use serde::{Deserialize, Serialize};

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
