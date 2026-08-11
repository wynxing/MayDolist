use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{TodoItem, TodoList, TodoSource};
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

    pub fn create_item(
        &self,
        list_id: &str,
        title: String,
        source: Option<TodoSource>,
    ) -> AppResult<TodoItem> {
        validate_title(&title)?;
        if let Some(source) = &source {
            source.validate()?;
        }
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
            source,
        };
        list.items.push(item.clone());
        list.updated_at = now_rfc3339();
        self.save(&list)?;
        Ok(item)
    }

    /// Create a Todo for a GitHub issue / PR. The item always lands in the
    /// capture inbox (reusing the idempotent `ensure_inbox` logic) and keeps
    /// a validated source reference. The same GitHub item may be converted
    /// more than once; dedup is intentionally left to a later version.
    pub fn create_item_from_github(
        &self,
        kind: &str,
        repo: &str,
        number: u64,
        title: &str,
        url: &str,
    ) -> AppResult<(TodoList, TodoItem)> {
        // Validate everything before touching storage so a rejected input can
        // never create a half-record (including a stray inbox).
        validate_title(title)?;
        let source = TodoSource {
            kind: kind.into(),
            repo: repo.into(),
            number,
            url: url.into(),
        };
        source.validate()?;
        let list = self.ensure_inbox()?;
        let item_title = format!("{} #{} {}", repo.trim(), number, title.trim());
        let item = self.create_item(&list.id, item_title, Some(source))?;
        Ok((list, item))
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
        let item = service
            .create_item(&inbox.id, " 修复登录 ".into(), None)
            .unwrap();
        assert_eq!(item.title, "修复登录");
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_item_from_github_lands_in_inbox_with_source() {
        let (dir, service) = temp_service("gh-to-todo");
        let (list, item) = service
            .create_item_from_github(
                "github-pr",
                "wynxing/MayDolist",
                19,
                "feat: 支持 GitHub 条目转 Todo",
                "https://github.com/wynxing/MayDolist/pull/19",
            )
            .unwrap();
        assert_eq!(list.kind.as_deref(), Some(INBOX_KIND));
        assert_eq!(
            item.title,
            "wynxing/MayDolist #19 feat: 支持 GitHub 条目转 Todo"
        );
        let source = item.source.as_ref().expect("source must be set");
        assert_eq!(source.kind, "github-pr");
        assert_eq!(source.repo, "wynxing/MayDolist");
        assert_eq!(source.number, 19);
        assert_eq!(source.url, "https://github.com/wynxing/MayDolist/pull/19");
        // Persisted and reloadable (survives a "restart").
        let stored = service.get(&list.id).unwrap();
        assert_eq!(stored.items.len(), 1);
        assert_eq!(stored.items[0].source, item.source);
        assert_eq!(service.list(false).unwrap().len(), 1, "inbox created once");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_item_from_github_supports_issues_and_allows_duplicates() {
        let (dir, service) = temp_service("gh-duplicates");
        let (_, first) = service
            .create_item_from_github(
                "github-issue",
                "owner/repo",
                7,
                "Bug: 登录失败",
                "https://github.com/owner/repo/issues/7",
            )
            .unwrap();
        assert_eq!(first.source.unwrap().kind, "github-issue");
        // The same GitHub item may be converted more than once; both items
        // are kept and both carry the source.
        let (_, second) = service
            .create_item_from_github(
                "github-issue",
                "owner/repo",
                7,
                "Bug: 登录失败",
                "https://github.com/owner/repo/issues/7",
            )
            .unwrap();
        assert_ne!(first.id, second.id);
        let inbox = service.ensure_inbox().unwrap();
        assert_eq!(inbox.items.len(), 2);
        assert!(inbox.items.iter().all(|v| v.source.is_some()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_item_from_github_rejects_invalid_input_without_partial_record() {
        let (dir, service) = temp_service("gh-reject");
        let bad_kind = service.create_item_from_github(
            "github-star",
            "owner/repo",
            1,
            "x",
            "https://github.com/owner/repo/issues/1",
        );
        assert!(bad_kind.is_err());
        let bad_url = service.create_item_from_github(
            "github-pr",
            "owner/repo",
            1,
            "x",
            "javascript:alert(1)",
        );
        assert!(bad_url.is_err());
        let empty_title = service.create_item_from_github(
            "github-pr",
            "owner/repo",
            1,
            "   ",
            "https://github.com/owner/repo/pull/1",
        );
        assert!(empty_title.is_err());
        assert_eq!(
            service.list(false).unwrap().len(),
            0,
            "no inbox/list may be created on failure"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_item_rejects_invalid_source_url() {
        let (dir, service) = temp_service("bad-source");
        let inbox = service.ensure_inbox().unwrap();
        let bad = service.create_item(
            &inbox.id,
            "危险来源".into(),
            Some(TodoSource {
                kind: "github-pr".into(),
                repo: "owner/repo".into(),
                number: 1,
                url: "file:///etc/passwd".into(),
            }),
        );
        assert!(bad.is_err());
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items.len(), 0, "no half-written item");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn todo_source_roundtrips_through_json_with_type_field() {
        let (dir, service) = temp_service("source-roundtrip");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "来源待办".into(),
                Some(TodoSource {
                    kind: "github-pr".into(),
                    repo: "owner/repo".into(),
                    number: 42,
                    url: "https://github.com/owner/repo/pull/42".into(),
                }),
            )
            .unwrap();
        let raw =
            std::fs::read_to_string(dir.join("todos").join(format!("{}.json", inbox.id))).unwrap();
        assert!(raw.contains("\"source\": {"));
        assert!(raw.contains("\"type\": \"github-pr\""));
        let parsed: TodoList = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.items[0].source, item.source);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn old_todo_without_source_reads_as_none() {
        let (dir, service) = temp_service("legacy-source");
        let id = uuid::Uuid::new_v4().to_string();
        let legacy = format!(
            r#"{{
                "schemaVersion": 1,
                "id": "{id}",
                "title": "旧列表",
                "sortOrder": 0,
                "deleted": false,
                "createdAt": "2026-08-01T00:00:00Z",
                "updatedAt": "2026-08-01T00:00:00Z",
                "items": [
                    {{
                        "id": "{}",
                        "title": "旧待办",
                        "completed": false,
                        "deleted": false,
                        "sortOrder": 0,
                        "createdAt": "2026-08-01T00:00:00Z",
                        "updatedAt": "2026-08-01T00:00:00Z"
                    }}
                ]
            }}"#,
            uuid::Uuid::new_v4()
        );
        std::fs::write(dir.join("todos").join(format!("{id}.json")), legacy).unwrap();
        let lists = service.list(false).unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].items.len(), 1);
        assert_eq!(lists[0].items[0].title, "旧待办");
        assert_eq!(lists[0].items[0].source, None);
        // Old format without the field stays byte-compatible when re-saved.
        let raw = std::fs::read_to_string(dir.join("todos").join(format!("{id}.json"))).unwrap();
        assert!(!raw.contains("\"source\""));
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
