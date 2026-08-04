use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{TodoItem, TodoList};
use crate::storage::Storage;

pub struct TodoService {
    storage: Arc<Storage>,
}

impl TodoService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub fn list(&self, include_deleted: bool) -> AppResult<Vec<TodoList>> {
        let mut lists: Vec<TodoList> = self.storage.list_json("todos")?;
        if !include_deleted {
            lists.retain(|list| !list.deleted);
            for list in &mut lists {
                list.items.retain(|item| !item.deleted);
            }
        }
        lists.sort_by_key(|list| list.sort_order);
        for list in &mut lists {
            list.items.sort_by_key(|item| item.sort_order);
        }
        Ok(lists)
    }

    pub fn create_list(&self, title: String) -> AppResult<TodoList> {
        validate_title(&title)?;
        let now = now_rfc3339();
        let list = TodoList {
            schema_version: 1,
            id: Uuid::new_v4().to_string(),
            title: title.trim().into(),
            sort_order: self.list(true)?.len() as i32,
            deleted: false,
            created_at: now.clone(),
            updated_at: now,
            items: vec![],
        };
        self.save(&list)?;
        Ok(list)
    }

    pub fn update_list(
        &self,
        id: &str,
        title: Option<String>,
        deleted: Option<bool>,
    ) -> AppResult<TodoList> {
        let mut list = self.get(id)?;
        if let Some(title) = title {
            validate_title(&title)?;
            list.title = title.trim().into();
        }
        if let Some(deleted) = deleted {
            list.deleted = deleted;
        }
        list.updated_at = now_rfc3339();
        self.save(&list)?;
        Ok(list)
    }

    pub fn reorder_lists(&self, ids: &[String]) -> AppResult<Vec<TodoList>> {
        let mut lists = self.list(true)?;
        for (order, id) in ids.iter().enumerate() {
            let list = lists
                .iter_mut()
                .find(|v| &v.id == id)
                .ok_or_else(|| AppError::NotFound(format!("todo list {id}")))?;
            list.sort_order = order as i32;
            list.updated_at = now_rfc3339();
            self.save(list)?;
        }
        self.list(false)
    }

    pub fn create_item(&self, list_id: &str, title: String) -> AppResult<TodoItem> {
        validate_title(&title)?;
        let mut list = self.get(list_id)?;
        let now = now_rfc3339();
        let item = TodoItem {
            id: Uuid::new_v4().to_string(),
            title: title.trim().into(),
            completed: false,
            deleted: false,
            sort_order: list.items.len() as i32,
            created_at: now.clone(),
            updated_at: now,
        };
        list.items.push(item.clone());
        list.updated_at = now_rfc3339();
        self.save(&list)?;
        Ok(item)
    }

    pub fn update_item(
        &self,
        id: &str,
        title: Option<String>,
        completed: Option<bool>,
        deleted: Option<bool>,
    ) -> AppResult<TodoItem> {
        let mut lists = self.list(true)?;
        for list in &mut lists {
            if let Some(item) = list.items.iter_mut().find(|v| v.id == id) {
                if let Some(title) = title {
                    validate_title(&title)?;
                    item.title = title.trim().into();
                }
                if let Some(value) = completed {
                    item.completed = value;
                }
                if let Some(value) = deleted {
                    item.deleted = value;
                }
                item.updated_at = now_rfc3339();
                let result = item.clone();
                list.updated_at = now_rfc3339();
                self.save(list)?;
                return Ok(result);
            }
        }
        Err(AppError::NotFound(format!("todo item {id}")))
    }

    pub fn move_item(&self, id: &str, target_list_id: &str, index: usize) -> AppResult<TodoItem> {
        let mut lists = self.list(true)?;
        let mut found = None;
        for list in &mut lists {
            if let Some(pos) = list.items.iter().position(|v| v.id == id) {
                found = Some(list.items.remove(pos));
                break;
            }
        }
        let item = found.ok_or_else(|| AppError::NotFound(format!("todo item {id}")))?;
        let target = lists
            .iter_mut()
            .find(|v| v.id == target_list_id)
            .ok_or_else(|| AppError::NotFound(format!("todo list {target_list_id}")))?;
        let at = index.min(target.items.len());
        target.items.insert(at, item.clone());
        for list in &mut lists {
            for (order, row) in list.items.iter_mut().enumerate() {
                row.sort_order = order as i32;
            }
            list.updated_at = now_rfc3339();
            self.save(list)?;
        }
        Ok(item)
    }

    pub fn reorder_items(&self, list_id: &str, ids: &[String]) -> AppResult<TodoList> {
        let mut list = self.get(list_id)?;
        for (order, id) in ids.iter().enumerate() {
            let item = list
                .items
                .iter_mut()
                .find(|v| &v.id == id)
                .ok_or_else(|| AppError::NotFound(format!("todo item {id}")))?;
            item.sort_order = order as i32;
        }
        list.updated_at = now_rfc3339();
        self.save(&list)?;
        Ok(list)
    }

    pub fn permanent_delete(&self, kind: &str, id: &str) -> AppResult<()> {
        if kind == "todoList" {
            return self.storage.delete_entity("todos", id);
        }
        let mut lists = self.list(true)?;
        for list in &mut lists {
            if let Some(pos) = list.items.iter().position(|v| v.id == id) {
                list.items.remove(pos);
                self.save(list)?;
                return Ok(());
            }
        }
        Err(AppError::NotFound(id.into()))
    }

    fn get(&self, id: &str) -> AppResult<TodoList> {
        self.list(true)?
            .into_iter()
            .find(|v| v.id == id)
            .ok_or_else(|| AppError::NotFound(format!("todo list {id}")))
    }
    fn save(&self, list: &TodoList) -> AppResult<()> {
        self.storage.save_entity("todos", &list.id, list)
    }
}

fn validate_title(title: &str) -> AppResult<()> {
    if title.trim().is_empty() {
        Err(AppError::InvalidInput("title must not be empty".into()))
    } else {
        Ok(())
    }
}
