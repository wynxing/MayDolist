use std::sync::Arc;
use uuid::Uuid;

use chrono::Local;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::todo::{next_repeat_due, parse_due_date, validate_due_date};
use crate::models::{
    GithubSyncMetadata, GithubSyncState, RepeatRule, TodoItem, TodoList, TodoSource,
};
use crate::storage::Storage;
use std::sync::Mutex;

/// Stable kind marker for the default capture inbox list.
pub const INBOX_KIND: &str = "inbox";
/// Default title of the capture inbox. Existing lists with this exact title
/// are adopted as the inbox so old data never gets a duplicate.
pub const INBOX_TITLE: &str = "收件箱";

/// Optional due / reminder / repeat fields of a Todo item. Used by both
/// create and update so the service never grows unbounded argument lists.
/// `None` fields mean "absent" (no due date / no reminder / no repeat).
#[derive(Debug, Clone, Default)]
pub struct TodoSchedule {
    pub due_date: Option<String>,
    pub remind_at: Option<String>,
    pub repeat: Option<RepeatRule>,
    pub repeat_until: Option<String>,
}

pub struct TodoService {
    storage: Arc<Storage>,
    /// Serializes inbox lookup/create so concurrent captures can never create
    /// duplicate "收件箱" lists.
    inbox_lock: Mutex<()>,
    /// Full on-disk lists (including deleted). Invalidated on every write so
    /// the reminder loop and Focus/palette reads do not rescan JSON every tick.
    cache: Mutex<Option<Vec<TodoList>>>,
}

impl TodoService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            inbox_lock: Mutex::new(()),
            cache: Mutex::new(None),
        }
    }

    pub fn invalidate_cache(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            *guard = None;
        }
    }

    pub fn list(&self, include_deleted: bool) -> AppResult<Vec<TodoList>> {
        let mut lists = {
            let mut guard = self
                .cache
                .lock()
                .map_err(|_| AppError::Internal("todo cache lock poisoned".into()))?;
            if let Some(cached) = guard.as_ref() {
                cached.clone()
            } else {
                let loaded: Vec<TodoList> = self.storage.list_json("todos")?;
                *guard = Some(loaded.clone());
                loaded
            }
        };
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
        schedule: TodoSchedule,
    ) -> AppResult<TodoItem> {
        validate_title(&title)?;
        if let Some(source) = &source {
            source.validate()?;
        }
        validate_due_fields(
            &schedule.due_date,
            &schedule.remind_at,
            &schedule.repeat,
            &schedule.repeat_until,
        )?;
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
            github_sync: None,
            due_date: schedule.due_date,
            remind_at: schedule.remind_at,
            repeat: schedule.repeat,
            repeat_until: schedule.repeat_until,
            last_reminded_at: None,
        };
        list.items.push(item.clone());
        list.updated_at = now_rfc3339();
        self.save(&list)?;
        Ok(item)
    }

    /// Create a Todo for a GitHub issue / PR. The item lands in the capture
    /// inbox (reusing the idempotent `ensure_inbox` logic) and keeps a
    /// validated source reference. An incomplete item with the same
    /// `repo` + `number` is reused instead of creating a duplicate.
    pub fn create_item_from_github(
        &self,
        kind: &str,
        repo: &str,
        number: u64,
        title: &str,
        url: &str,
    ) -> AppResult<(TodoList, TodoItem, bool)> {
        validate_title(title)?;
        let source = TodoSource {
            kind: kind.into(),
            repo: repo.into(),
            number,
            url: url.into(),
        };
        source.validate()?;
        let lists = self.list(false)?;
        for list in &lists {
            if let Some(item) = list.items.iter().find(|item| {
                !item.deleted
                    && !item.completed
                    && item.source.as_ref().is_some_and(|existing| {
                        existing.repo == source.repo && existing.number == source.number
                    })
            }) {
                return Ok((list.clone(), item.clone(), false));
            }
        }
        let list = self.ensure_inbox()?;
        let item_title = format!("{} #{} {}", repo.trim(), number, title.trim());
        let item = self.create_item(&list.id, item_title, Some(source), TodoSchedule::default())?;
        Ok((list, item, true))
    }

    /// Complete a Todo item. When the item carries a repeat rule, the next
    /// instance is generated in the same list (same title, source and rule)
    /// and persisted in the same atomic write, so a crash can never leave a
    /// duplicate. `repeat_until` stops generation.
    pub fn update_item(
        &self,
        id: &str,
        title: Option<String>,
        completed: Option<bool>,
        deleted: Option<bool>,
        schedule: Option<TodoSchedule>,
    ) -> AppResult<TodoItem> {
        let mut lists = self.list(true)?;
        for list in &mut lists {
            let next_order = list.items.len() as i32;
            if let Some(item) = list.items.iter_mut().find(|v| v.id == id) {
                let was_completed = item.completed;
                if let Some(title) = title {
                    validate_title(&title)?;
                    item.title = title.trim().into();
                }
                if let Some(value) = completed {
                    item.completed = value;
                    if !value {
                        if let Some(sync) = item.github_sync.as_mut() {
                            if sync.auto_completed_at.is_some() {
                                sync.auto_completion_undone_at = Some(now_rfc3339());
                            }
                        }
                    }
                }
                if let Some(value) = deleted {
                    item.deleted = value;
                }
                if let Some(schedule) = schedule {
                    item.due_date = schedule.due_date;
                    item.remind_at = schedule.remind_at;
                    item.repeat = schedule.repeat;
                    item.repeat_until = schedule.repeat_until;
                }
                validate_due_fields(
                    &item.due_date,
                    &item.remind_at,
                    &item.repeat,
                    &item.repeat_until,
                )?;
                let next_instance = if completed == Some(true) && !was_completed {
                    build_next_instance(item, next_order)
                } else {
                    None
                };
                item.updated_at = now_rfc3339();
                let result = item.clone();
                if let Some(next) = next_instance {
                    list.items.push(next);
                }
                list.updated_at = now_rfc3339();
                self.save(list)?;
                return Ok(result);
            }
        }
        Err(AppError::NotFound(format!("todo item {id}")))
    }

    /// Apply a successfully fetched GitHub source state to a Todo. This path
    /// intentionally does not use `update_item`: an automatic completion must
    /// never create a repeat instance or otherwise alter local scheduling.
    pub fn sync_github_item(
        &self,
        id: &str,
        state: GithubSyncState,
        synced_at: &str,
        auto_complete: bool,
    ) -> AppResult<(TodoItem, bool, bool)> {
        let mut lists = self.list(true)?;
        for list in &mut lists {
            if let Some(item) = list.items.iter_mut().find(|v| v.id == id) {
                if item.source.is_none() {
                    return Err(AppError::InvalidInput(
                        "todo item has no GitHub source".into(),
                    ));
                }
                let previous = item.github_sync.clone();
                let mut metadata = previous.clone().unwrap_or(GithubSyncMetadata {
                    state: GithubSyncState::Unknown,
                    last_synced_at: None,
                    auto_completed_at: None,
                    auto_completion_reason: None,
                    auto_completion_undone_at: None,
                    sync_error: None,
                });
                let previous_state = metadata.state;
                let previous_error = metadata.sync_error.clone();
                let mut auto_completed = false;
                metadata.state = state;
                metadata.last_synced_at = Some(synced_at.into());
                metadata.sync_error = None;
                if state == GithubSyncState::Open {
                    metadata.auto_completion_undone_at = None;
                }
                if auto_complete
                    && !item.completed
                    && matches!(state, GithubSyncState::Closed | GithubSyncState::Merged)
                    && metadata.auto_completion_undone_at.is_none()
                {
                    item.completed = true;
                    metadata.auto_completed_at = Some(synced_at.into());
                    metadata.auto_completion_undone_at = None;
                    metadata.auto_completion_reason = Some(
                        match state {
                            GithubSyncState::Merged => "merged",
                            _ => "closed",
                        }
                        .into(),
                    );
                    auto_completed = true;
                }
                let changed = previous.is_none()
                    || previous_state != state
                    || previous_error.is_some()
                    || auto_completed;
                item.github_sync = Some(metadata);
                if changed {
                    item.updated_at = synced_at.into();
                    list.updated_at = now_rfc3339();
                    let result = item.clone();
                    self.save(list)?;
                    return Ok((result, true, auto_completed));
                }
                // Keep the latest successful timestamp in memory and on disk,
                // but do not make the UI refresh on every periodic poll.
                let result = item.clone();
                self.save(list)?;
                return Ok((result, false, false));
            }
        }
        Err(AppError::NotFound(format!("todo item {id}")))
    }

    /// Record a failed source lookup without guessing that the source closed.
    /// Existing known state is retained; a source never checked successfully
    /// is represented as `unknown`.
    pub fn record_github_sync_error(
        &self,
        id: &str,
        error: &str,
        recorded_at: &str,
    ) -> AppResult<(TodoItem, bool)> {
        let mut lists = self.list(true)?;
        for list in &mut lists {
            if let Some(item) = list.items.iter_mut().find(|v| v.id == id) {
                if item.source.is_none() {
                    return Err(AppError::InvalidInput(
                        "todo item has no GitHub source".into(),
                    ));
                }
                let previous = item.github_sync.clone();
                let mut metadata = previous.clone().unwrap_or(GithubSyncMetadata {
                    state: GithubSyncState::Unknown,
                    last_synced_at: None,
                    auto_completed_at: None,
                    auto_completion_reason: None,
                    auto_completion_undone_at: None,
                    sync_error: None,
                });
                let changed = previous.is_none() || metadata.sync_error.as_deref() != Some(error);
                metadata.sync_error = Some(error.into());
                item.github_sync = Some(metadata);
                if changed {
                    item.updated_at = recorded_at.into();
                    list.updated_at = now_rfc3339();
                    let result = item.clone();
                    self.save(list)?;
                    return Ok((result, true));
                }
                return Ok((item.clone(), false));
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

    /// Record that a reminder for `remind_at` was delivered or suppressed.
    /// Missing items degrade to a no-op so the scheduler never crashes.
    pub fn mark_reminded(&self, id: &str, remind_at: &str) -> AppResult<()> {
        let mut lists = self.list(true)?;
        for list in &mut lists {
            if let Some(item) = list.items.iter_mut().find(|v| v.id == id) {
                if item.last_reminded_at.as_deref() == Some(remind_at) {
                    return Ok(());
                }
                item.last_reminded_at = Some(remind_at.into());
                item.updated_at = now_rfc3339();
                list.updated_at = now_rfc3339();
                self.save(list)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn get(&self, id: &str) -> AppResult<TodoList> {
        self.list(true)?
            .into_iter()
            .find(|v| v.id == id)
            .ok_or_else(|| AppError::NotFound(format!("todo list {id}")))
    }
    fn save(&self, list: &TodoList) -> AppResult<()> {
        self.storage.save_entity("todos", &list.id, list)?;
        self.invalidate_cache();
        Ok(())
    }
}

fn validate_title(title: &str) -> AppResult<()> {
    if title.trim().is_empty() {
        Err(AppError::InvalidInput("title must not be empty".into()))
    } else {
        Ok(())
    }
}

/// Validate the optional due / reminder / repeat fields before persisting.
/// `remindAt` only makes sense with `dueDate`; `repeatUntil` only makes sense
/// with `repeat`. Invalid dates are rejected (never crash, never persisted).
fn validate_due_fields(
    due_date: &Option<String>,
    remind_at: &Option<String>,
    repeat: &Option<RepeatRule>,
    repeat_until: &Option<String>,
) -> AppResult<()> {
    if let Some(value) = due_date {
        validate_due_date(value)?;
    }
    if let Some(value) = remind_at {
        if due_date.is_none() {
            return Err(AppError::InvalidInput(
                "remindAt requires dueDate to be set".into(),
            ));
        }
        validate_due_date(value)?;
    }
    if let Some(value) = repeat_until {
        if repeat.is_none() {
            return Err(AppError::InvalidInput(
                "repeatUntil requires repeat to be set".into(),
            ));
        }
        validate_due_date(value)?;
    }
    Ok(())
}

/// Build the next repeat instance from a freshly-completed `item`. The anchor
/// is the item's due date (falling back to today when unset); the next
/// occurrence is always strictly after today. Returns `None` when the rule
/// ended (`repeat_until`) so no duplicate is ever generated.
fn build_next_instance(item: &TodoItem, sort_order: i32) -> Option<TodoItem> {
    let rule = item.repeat?;
    let anchor = item
        .due_date
        .as_deref()
        .and_then(parse_due_date)
        .unwrap_or_else(|| Local::now().date_naive());
    let today = Local::now().date_naive();
    let until = item.repeat_until.as_deref().and_then(parse_due_date);
    let next_due = next_repeat_due(rule, anchor, today, until)?;
    let now = now_rfc3339();
    Some(TodoItem {
        id: Uuid::new_v4().to_string(),
        title: item.title.clone(),
        completed: false,
        deleted: false,
        sort_order,
        created_at: now.clone(),
        updated_at: now,
        source: item.source.clone(),
        github_sync: item.github_sync.clone(),
        due_date: Some(next_due.format("%Y-%m-%d").to_string()),
        remind_at: None,
        repeat: Some(rule),
        repeat_until: item.repeat_until.clone(),
        last_reminded_at: None,
    })
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

    fn schedule(
        due: Option<&str>,
        remind: Option<&str>,
        repeat: Option<RepeatRule>,
        until: Option<&str>,
    ) -> TodoSchedule {
        TodoSchedule {
            due_date: due.map(str::to_string),
            remind_at: remind.map(str::to_string),
            repeat,
            repeat_until: until.map(str::to_string),
        }
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
            .create_item(
                &inbox.id,
                " 修复登录 ".into(),
                None,
                TodoSchedule::default(),
            )
            .unwrap();
        assert_eq!(item.title, "修复登录");
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_item_from_github_lands_in_inbox_with_source() {
        let (dir, service) = temp_service("gh-to-todo");
        let (list, item, created) = service
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
        assert!(created);
        // Persisted and reloadable (survives a "restart").
        let stored = service.get(&list.id).unwrap();
        assert_eq!(stored.items.len(), 1);
        assert_eq!(stored.items[0].source, item.source);
        assert_eq!(service.list(false).unwrap().len(), 1, "inbox created once");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_item_from_github_dedups_by_repo_and_number() {
        let (dir, service) = temp_service("gh-dedup");
        let (_, first, created_first) = service
            .create_item_from_github(
                "github-issue",
                "owner/repo",
                7,
                "Bug: 登录失败",
                "https://github.com/owner/repo/issues/7",
            )
            .unwrap();
        assert!(created_first);
        assert_eq!(first.source.unwrap().kind, "github-issue");
        let (_, second, created_second) = service
            .create_item_from_github(
                "github-issue",
                "owner/repo",
                7,
                "Bug: 登录失败",
                "https://github.com/owner/repo/issues/7",
            )
            .unwrap();
        assert!(!created_second);
        assert_eq!(first.id, second.id);
        let inbox = service.ensure_inbox().unwrap();
        assert_eq!(inbox.items.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn github_closed_source_auto_completes_without_spawning_repeat() {
        let (dir, service) = temp_service("gh-sync-close");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "跟进 PR".into(),
                Some(TodoSource {
                    kind: "github-pr".into(),
                    repo: "owner/repo".into(),
                    number: 8,
                    url: "https://github.com/owner/repo/pull/8".into(),
                }),
                TodoSchedule {
                    due_date: Some("2026-08-15".into()),
                    remind_at: None,
                    repeat: Some(RepeatRule::Weekly),
                    repeat_until: None,
                },
            )
            .unwrap();
        let (updated, changed, auto_completed) = service
            .sync_github_item(
                &item.id,
                GithubSyncState::Closed,
                "2026-08-17T10:00:00Z",
                true,
            )
            .unwrap();
        assert!(changed);
        assert!(auto_completed);
        assert!(updated.completed);
        assert_eq!(
            updated.github_sync.as_ref().unwrap().state,
            GithubSyncState::Closed
        );
        assert_eq!(
            updated
                .github_sync
                .as_ref()
                .unwrap()
                .auto_completion_reason
                .as_deref(),
            Some("closed")
        );
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(
            stored.items.len(),
            1,
            "sync completion must not create a repeat item"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn github_merged_source_auto_completes_and_reopen_does_not_reopen_todo() {
        let (dir, service) = temp_service("gh-sync-merge-reopen");
        let (_, item, _) = service
            .create_item_from_github(
                "github-pr",
                "owner/repo",
                9,
                "合并后跟进",
                "https://github.com/owner/repo/pull/9",
            )
            .unwrap();
        let (closed, _, auto_completed) = service
            .sync_github_item(
                &item.id,
                GithubSyncState::Merged,
                "2026-08-17T10:00:00Z",
                true,
            )
            .unwrap();
        assert!(auto_completed);
        assert!(closed.completed);
        let (reopened, _, _) = service
            .sync_github_item(
                &item.id,
                GithubSyncState::Open,
                "2026-08-18T10:00:00Z",
                true,
            )
            .unwrap();
        assert!(
            reopened.completed,
            "reopening GitHub must not reopen a local Todo"
        );
        assert_eq!(
            reopened.github_sync.as_ref().unwrap().state,
            GithubSyncState::Open
        );
        assert!(reopened
            .github_sync
            .as_ref()
            .unwrap()
            .auto_completed_at
            .is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn github_sync_error_never_auto_completes_or_guesses_closed() {
        let (dir, service) = temp_service("gh-sync-error");
        let (_, item, _) = service
            .create_item_from_github(
                "github-issue",
                "owner/repo",
                10,
                "网络错误时保持待办",
                "https://github.com/owner/repo/issues/10",
            )
            .unwrap();
        let (updated, changed) = service
            .record_github_sync_error(&item.id, "GitHub API unavailable", "2026-08-17T10:00:00Z")
            .unwrap();
        assert!(changed);
        assert!(!updated.completed);
        let sync = updated.github_sync.unwrap();
        assert_eq!(sync.state, GithubSyncState::Unknown);
        assert_eq!(sync.sync_error.as_deref(), Some("GitHub API unavailable"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn manually_undoing_auto_completion_is_respected_until_source_reopens() {
        let (dir, service) = temp_service("gh-sync-undo");
        let (_, item, _) = service
            .create_item_from_github(
                "github-issue",
                "owner/repo",
                11,
                "允许手动恢复",
                "https://github.com/owner/repo/issues/11",
            )
            .unwrap();
        service
            .sync_github_item(
                &item.id,
                GithubSyncState::Closed,
                "2026-08-17T10:00:00Z",
                true,
            )
            .unwrap();
        service
            .update_item(&item.id, None, Some(false), None, None)
            .unwrap();
        let (undone, _, auto_completed_again) = service
            .sync_github_item(
                &item.id,
                GithubSyncState::Closed,
                "2026-08-17T11:00:00Z",
                true,
            )
            .unwrap();
        assert!(!auto_completed_again);
        assert!(!undone.completed);
        assert!(undone
            .github_sync
            .as_ref()
            .unwrap()
            .auto_completion_undone_at
            .is_some());
        service
            .sync_github_item(
                &item.id,
                GithubSyncState::Open,
                "2026-08-18T10:00:00Z",
                true,
            )
            .unwrap();
        let (closed_again, _, auto_completed_again) = service
            .sync_github_item(
                &item.id,
                GithubSyncState::Closed,
                "2026-08-19T10:00:00Z",
                true,
            )
            .unwrap();
        assert!(auto_completed_again);
        assert!(closed_again.completed);
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
            TodoSchedule::default(),
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
                TodoSchedule::default(),
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

    #[test]
    fn create_item_persists_due_reminder_and_repeat_fields() {
        let (dir, service) = temp_service("due-fields");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "到期任务".into(),
                None,
                schedule(
                    Some("2026-08-20"),
                    Some("2026-08-20T09:00:00+08:00"),
                    Some(RepeatRule::Weekly),
                    Some("2026-12-31"),
                ),
            )
            .unwrap();
        assert_eq!(item.due_date.as_deref(), Some("2026-08-20"));
        assert_eq!(item.repeat, Some(RepeatRule::Weekly));
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items[0].due_date, item.due_date);
        assert_eq!(stored.items[0].remind_at, item.remind_at);
        assert_eq!(stored.items[0].repeat_until, item.repeat_until);
        let raw =
            std::fs::read_to_string(dir.join("todos").join(format!("{}.json", inbox.id))).unwrap();
        assert!(raw.contains("\"dueDate\": \"2026-08-20\""));
        assert!(raw.contains("\"repeat\": \"weekly\""));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_item_rejects_invalid_due_fields_without_partial_record() {
        let (dir, service) = temp_service("due-reject");
        let inbox = service.ensure_inbox().unwrap();
        assert!(service
            .create_item(
                &inbox.id,
                "坏日期".into(),
                None,
                schedule(Some("2026-02-30"), None, None, None),
            )
            .is_err());
        assert!(service
            .create_item(
                &inbox.id,
                "无到期日的提醒".into(),
                None,
                schedule(None, Some("2026-08-20T09:00:00Z"), None, None),
            )
            .is_err());
        assert!(service
            .create_item(
                &inbox.id,
                "无周期的截止".into(),
                None,
                schedule(None, None, None, Some("2026-12-31")),
            )
            .is_err());
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items.len(), 0, "no half-written item");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn completing_repeat_item_spawns_exactly_one_next_instance() {
        let (dir, service) = temp_service("repeat-spawn");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "每周清理 stale PR".into(),
                None,
                schedule(Some("2026-01-05"), None, Some(RepeatRule::Weekly), None),
            )
            .unwrap();
        let done = service
            .update_item(&item.id, None, Some(true), None, None)
            .unwrap();
        assert!(done.completed);
        let stored = service.get(&inbox.id).unwrap();
        let pending: Vec<_> = stored.items.iter().filter(|v| !v.completed).collect();
        assert_eq!(pending.len(), 1, "exactly one next instance");
        let next = &pending[0];
        assert_eq!(next.title, "每周清理 stale PR");
        assert_eq!(next.repeat, Some(RepeatRule::Weekly));
        let today = chrono::Local::now().date_naive();
        let next_due = crate::models::parse_due_date(next.due_date.as_deref().unwrap()).unwrap();
        assert!(next_due > today, "next due must be after today");
        assert!(
            (next_due - today).num_days() <= 7,
            "weekly next occurrence within 7 days"
        );
        // Completing the already-completed item must not spawn a duplicate.
        service
            .update_item(&item.id, None, Some(true), None, None)
            .unwrap();
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items.iter().filter(|v| !v.completed).count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn repeat_until_stops_generation_on_completion() {
        let (dir, service) = temp_service("repeat-until");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "临时每日任务".into(),
                None,
                schedule(
                    Some("2026-01-01"),
                    None,
                    Some(RepeatRule::Daily),
                    Some("2020-12-31"),
                ),
            )
            .unwrap();
        service
            .update_item(&item.id, None, Some(true), None, None)
            .unwrap();
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(
            stored.items.len(),
            1,
            "repeatUntil expired -> no next instance"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn completing_plain_item_creates_no_next_instance() {
        let (dir, service) = temp_service("plain-complete");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "一次性任务".into(),
                None,
                TodoSchedule::default(),
            )
            .unwrap();
        service
            .update_item(&item.id, None, Some(true), None, None)
            .unwrap();
        let stored = service.get(&inbox.id).unwrap();
        assert_eq!(stored.items.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn repeat_next_instance_keeps_source_link() {
        let (dir, service) = temp_service("repeat-source");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "跟进 issue".into(),
                Some(TodoSource {
                    kind: "github-issue".into(),
                    repo: "owner/repo".into(),
                    number: 7,
                    url: "https://github.com/owner/repo/issues/7".into(),
                }),
                schedule(Some("2026-01-01"), None, Some(RepeatRule::Monthly), None),
            )
            .unwrap();
        service
            .update_item(&item.id, None, Some(true), None, None)
            .unwrap();
        let stored = service.get(&inbox.id).unwrap();
        let next = stored.items.iter().find(|v| !v.completed).unwrap();
        let source = next.source.as_ref().expect("source must be kept");
        assert_eq!(source.number, 7);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_item_can_clear_and_validate_due_fields() {
        let (dir, service) = temp_service("update-due");
        let inbox = service.ensure_inbox().unwrap();
        let item = service
            .create_item(
                &inbox.id,
                "可编辑任务".into(),
                None,
                schedule(Some("2026-08-20"), Some("2026-08-20T09:00:00Z"), None, None),
            )
            .unwrap();
        // Invalid datetime must be rejected and leave the item untouched.
        assert!(service
            .update_item(
                &item.id,
                None,
                None,
                None,
                Some(schedule(Some("oops"), None, None, None)),
            )
            .is_err());
        // Clearing dueDate together with remindAt succeeds.
        let updated = service
            .update_item(&item.id, None, None, None, Some(TodoSchedule::default()))
            .unwrap();
        assert_eq!(updated.due_date, None);
        assert_eq!(updated.remind_at, None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
