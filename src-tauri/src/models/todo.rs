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
    pub sort_order: i32,
    pub deleted: bool,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<TodoItem>,
}
