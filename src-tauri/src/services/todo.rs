use std::sync::Mutex;

use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{TodoItem, TodoList};

pub trait TodoService: Send + Sync {
    fn list(&self) -> Vec<TodoList>;
    fn create_list(&self, title: String) -> AppResult<TodoList>;
    fn create_item(&self, list_id: &str, title: String) -> AppResult<TodoItem>;
    fn update_item(
        &self,
        id: &str,
        title: Option<String>,
        completed: Option<bool>,
    ) -> AppResult<TodoItem>;
    fn soft_delete_item(&self, id: &str) -> AppResult<()>;
}

/// In-memory mock. Business data is not persisted in the skeleton phase.
pub struct MockTodoService {
    lists: Mutex<Vec<TodoList>>,
}

impl MockTodoService {
    pub fn seeded() -> Self {
        Self {
            lists: Mutex::new(vec![
                TodoList {
                    id: "list-personal".into(),
                    title: "个人".into(),
                    items: vec![
                        TodoItem {
                            id: "todo-1".into(),
                            title: "买牛奶".into(),
                            completed: false,
                            deleted: false,
                            created_at: "2026-08-01T09:00:00Z".into(),
                        },
                        TodoItem {
                            id: "todo-2".into(),
                            title: "预约体检".into(),
                            completed: true,
                            deleted: false,
                            created_at: "2026-08-01T09:05:00Z".into(),
                        },
                    ],
                },
                TodoList {
                    id: "list-work".into(),
                    title: "工作".into(),
                    items: vec![TodoItem {
                        id: "todo-3".into(),
                        title: "评审 PR #1234".into(),
                        completed: false,
                        deleted: false,
                        created_at: "2026-08-02T14:00:00Z".into(),
                    }],
                },
            ]),
        }
    }
}

impl TodoService for MockTodoService {
    fn list(&self) -> Vec<TodoList> {
        let lists = self.lists.lock().unwrap();
        let mut result = lists.clone();
        for list in &mut result {
            list.items.retain(|item| !item.deleted);
        }
        result
    }

    fn create_list(&self, title: String) -> AppResult<TodoList> {
        let mut lists = self.lists.lock().unwrap();
        let list = TodoList {
            id: Uuid::new_v4().to_string(),
            title,
            items: Vec::new(),
        };
        lists.push(list.clone());
        Ok(list)
    }

    fn create_item(&self, list_id: &str, title: String) -> AppResult<TodoItem> {
        let mut lists = self.lists.lock().unwrap();
        let list = lists
            .iter_mut()
            .find(|list| list.id == list_id)
            .ok_or_else(|| AppError::NotFound(format!("todo list {list_id}")))?;
        let item = TodoItem {
            id: Uuid::new_v4().to_string(),
            title,
            completed: false,
            deleted: false,
            created_at: now_rfc3339(),
        };
        list.items.push(item.clone());
        Ok(item)
    }

    fn update_item(
        &self,
        id: &str,
        title: Option<String>,
        completed: Option<bool>,
    ) -> AppResult<TodoItem> {
        let mut lists = self.lists.lock().unwrap();
        let item = lists
            .iter_mut()
            .flat_map(|list| list.items.iter_mut())
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(format!("todo item {id}")))?;
        if let Some(title) = title {
            if title.trim().is_empty() {
                return Err(AppError::InvalidInput("title must not be empty".into()));
            }
            item.title = title;
        }
        if let Some(completed) = completed {
            item.completed = completed;
        }
        Ok(item.clone())
    }

    fn soft_delete_item(&self, id: &str) -> AppResult<()> {
        let mut lists = self.lists.lock().unwrap();
        let item = lists
            .iter_mut()
            .flat_map(|list| list.items.iter_mut())
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(format!("todo item {id}")))?;
        item.deleted = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_crud_flow() {
        let service = MockTodoService::seeded();
        assert_eq!(service.list().len(), 2);

        let list = service.create_list("购物".into()).unwrap();
        let item = service.create_item(&list.id, "纸巾".into()).unwrap();
        assert_eq!(service.list().iter().find(|l| l.id == list.id).unwrap().items.len(), 1);

        let updated = service.update_item(&item.id, Some("抽纸".into()), Some(true)).unwrap();
        assert_eq!(updated.title, "抽纸");
        assert!(updated.completed);

        service.soft_delete_item(&item.id).unwrap();
        assert!(service.list().iter().find(|l| l.id == list.id).unwrap().items.is_empty());
    }

    #[test]
    fn todo_missing_ids_error() {
        let service = MockTodoService::seeded();
        assert!(matches!(
            service.create_item("nope", "x".into()),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            service.soft_delete_item("nope"),
            Err(AppError::NotFound(_))
        ));
    }
}
