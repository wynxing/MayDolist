use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub completed: bool,
    /// Soft-delete flag: the row is kept on disk but filtered out of lists.
    pub deleted: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    pub id: String,
    pub title: String,
    pub items: Vec<TodoItem>,
}
