use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{TodoItem, TodoList};
use crate::storage::Storage;
use std::sync::Mutex;

/// Stable kind marker for the default capture inbox list.
pub const INBOX_KIND: &str = "inbox";
/// Default title of the capture inbox. Existing lists with this exact title
/// are adopted as the inbox so old data never gets a duplicate.
pub const INBOX_TITLE: &str = "收件箱";

pub struct TodoService {
    storage: Arc<Storage>,
    /// Serializes inbox lookup/create so concurrent captures can never create
    /// duplicate "收件箱" lists.
    inbox_lock: Mutex<()>,
}

impl TodoService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            inbox_lock: Mutex::new(()),
        }
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
            kind: None,
            sort_order: self.list(true)?.len() as i32,
            deleted: false,
            created_at: now.clone(),
            updated_at: now,
            items: vec![],
        };
        self.save(&list)?;
        Ok(list)
    }

    /// Return the capture inbox, creating it exactly once. Prefers the stable
    /// `kind` marker, then adopts an existing list titled "收件箱" (marking it
    /// so the adoption is remembered), and only creates a new list when neither
    /// exists.
    pub fn ensure_inbox(&self) -> AppResult<TodoList> {
        let _guard = self
            .inbox_lock
            .lock()
            .map_err(|_| AppError::Internal("todo inbox lock poisoned".into()))?;
        let lists = self.list(false)?;
        if let Some(list) = lists.iter().find(|v| v.kind.as_deref() == Some(INBOX_KIND)) {
            return Ok(list.clone());
        }
        if let Some(list) = lists.iter().find(|v| v.title.trim() == INBOX_TITLE) {
            let mut list = list.clone();
            list.kind = Some(INBOX_KIND.into());
            list.updated_at = now_rfc3339();
            self.save(&list)?;
            return Ok(list);
        }
        let mut list = self.create_list(INBOX_TITLE.into())?;
        list.kind = Some(INBOX_KIND.into());
        list.updated_at = now_rfc3339();
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
        let mut source_list_id = None;
        for list in &mut lists {
            if let Some(pos) = list.items.iter().position(|v| v.id == id) {
                found = Some(list.items.remove(pos));
                source_list_id = Some(list.id.clone());
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
            let changed =
                list.id == target_list_id || source_list_id.as_deref() == Some(list.id.as_str());
            if !changed {
                continue;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::now_rfc3339;
    use std::sync::Arc;

    fn temp_service(tag: &str) -> (std::path::PathBuf, TodoService) {
        let dir =
            std::env::temp_dir().join(format!("maydolist-todo-{}-{}", tag, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let storage = Arc::new(Storage::with_dir(&dir).unwrap());
        (dir, TodoService::new(storage))
    }

    #[test]
    fn ensure_inbox_creates_exactly_once() {
        let (dir, service) = temp_service("inbox-once");
        let first = service.ensure_inbox().unwrap();
        assert_eq!(first.title, INBOX_TITLE);
        assert_eq!(first.kind.as_deref(), Some(INBOX_KIND));
        let second = service.ensure_inbox().unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(service.list(false).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ensure_inbox_adopts_legacy_list_with_inbox_title() {
        let (dir, service) = temp_service("inbox-adopt");
        let legacy = service.create_list(INBOX_TITLE.into()).unwrap();
        assert_eq!(legacy.kind, None);
        let inbox = service.ensure_inbox().unwrap();
        assert_eq!(inbox.id, legacy.id);
        assert_eq!(inbox.kind.as_deref(), Some(INBOX_KIND));
        assert_eq!(service.list(false).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ensure_inbox_ignores_deleted_inbox_and_creates_new_one() {
        let (dir, service) = temp_service("inbox-deleted");
        let legacy = service.create_list(INBOX_TITLE.into()).unwrap();
        service.update_list(&legacy.id, None, Some(true)).unwrap();
        let inbox = service.ensure_inbox().unwrap();
        assert_ne!(inbox.id, legacy.id);
        assert_eq!(inbox.kind.as_deref(), Some(INBOX_KIND));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn inbox_created_by_ensure_is_usable_for_items() {
        let (dir, service) = temp_service("inbox-items");
        let inbox = service.ensure_inbox().unwrap();
        let item = service.create_item(&inbox.id, " 修复登录 ".into()).unwrap();
        assert_eq!(item.title, "修复登录");
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_kind_roundtrips_through_json() {
        let (dir, service) = temp_service("kind-roundtrip");
        let mut list = service.create_list("普通列表".into()).unwrap();
        assert_eq!(list.kind, None);
        list.kind = Some(INBOX_KIND.into());
        list.updated_at = now_rfc3339();
        service.save(&list).unwrap();
        let stored = service.get(&list.id).unwrap();
        assert_eq!(stored.kind.as_deref(), Some(INBOX_KIND));
        let raw =
            std::fs::read_to_string(dir.join("todos").join(format!("{}.json", list.id))).unwrap();
        assert!(raw.contains("\"kind\": \"inbox\""));
        std::fs::remove_dir_all(&dir).ok();
    }
}
