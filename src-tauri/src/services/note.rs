use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{Note, WindowBounds};
use crate::storage::Storage;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePatch {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub color: Option<String>,
    pub pinned: Option<bool>,
    pub floating: Option<bool>,
    pub collapsed: Option<bool>,
    pub always_on_top: Option<bool>,
    pub window_bounds: Option<WindowBounds>,
    pub deleted: Option<bool>,
}

pub struct NoteService {
    storage: Arc<Storage>,
    /// Serializes read-modify-write so concurrent content and bounds updates
    /// cannot overwrite each other with stale whole-note snapshots.
    write_lock: Mutex<()>,
    /// Full on-disk notes (including deleted). Invalidated on every write so
    /// readers (focus overview, palette, floating windows) do not rescan JSON.
    cache: Mutex<NoteCache>,
}

/// Shared read projections of the note data, handed out as `Arc` clones so
/// frequent readers never deep-clone the whole list.
#[derive(Default)]
struct NoteCache {
    full: Option<Arc<Vec<Note>>>,
    visible: Option<Arc<Vec<Note>>>,
}
impl NoteService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            write_lock: Mutex::new(()),
            cache: Mutex::new(NoteCache::default()),
        }
    }
    pub fn invalidate_cache(&self) {
        if let Ok(mut guard) = self.cache.lock() {
            *guard = NoteCache::default();
        }
    }
    pub fn list(&self, include_deleted: bool) -> AppResult<Arc<Vec<Note>>> {
        let mut guard = self
            .cache
            .lock()
            .map_err(|_| AppError::Internal("note cache lock poisoned".into()))?;
        if include_deleted {
            if guard.full.is_none() {
                let loaded: Vec<Note> = self.storage.list_json("notes")?;
                guard.full = Some(Arc::new(loaded));
            }
            return Ok(guard.full.clone().expect("full cache populated"));
        }
        if guard.visible.is_none() {
            if guard.full.is_none() {
                let loaded: Vec<Note> = self.storage.list_json("notes")?;
                guard.full = Some(Arc::new(loaded));
            }
            let full = guard.full.clone().expect("full cache populated");
            let mut visible: Vec<Note> = full.iter().filter(|v| !v.deleted).cloned().collect();
            visible.sort_by(|a, b| {
                b.pinned
                    .cmp(&a.pinned)
                    .then_with(|| b.updated_at.cmp(&a.updated_at))
            });
            guard.visible = Some(Arc::new(visible));
        }
        Ok(guard.visible.clone().expect("visible cache populated"))
    }
    pub fn get(&self, id: &str) -> AppResult<Note> {
        self.list(true)?
            .iter()
            .find(|v| v.id == id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("note {id}")))
    }
    pub fn create(&self, title: String, content: String) -> AppResult<Note> {
        if title.trim().is_empty() {
            return Err(AppError::InvalidInput("title must not be empty".into()));
        }
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| AppError::Internal("note write lock poisoned".into()))?;
        let now = now_rfc3339();
        let note = Note {
            schema_version: 1,
            id: Uuid::new_v4().to_string(),
            title: title.trim().into(),
            content,
            tags: vec![],
            color: "blue".into(),
            pinned: false,
            floating: false,
            collapsed: false,
            always_on_top: true,
            window_bounds: None,
            deleted: false,
            created_at: now.clone(),
            updated_at: now,
        };
        self.save(&note)?;
        Ok(note)
    }
    pub fn update(&self, id: &str, patch: NotePatch) -> AppResult<Note> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| AppError::Internal("note write lock poisoned".into()))?;
        let mut note = self.get(id)?;
        if let Some(v) = patch.title {
            if v.trim().is_empty() {
                return Err(AppError::InvalidInput("title must not be empty".into()));
            }
            note.title = v.trim().into();
        }
        if let Some(v) = patch.content {
            note.content = v;
        }
        if let Some(v) = patch.tags {
            let mut tags: Vec<String> = v
                .into_iter()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect();
            tags.sort();
            tags.dedup();
            note.tags = tags;
        }
        if let Some(v) = patch.color {
            note.color = v;
        }
        if let Some(v) = patch.pinned {
            note.pinned = v;
        }
        if let Some(v) = patch.floating {
            note.floating = v;
        }
        if let Some(v) = patch.collapsed {
            note.collapsed = v;
        }
        if let Some(v) = patch.always_on_top {
            note.always_on_top = v;
        }
        if let Some(v) = patch.window_bounds {
            note.window_bounds = Some(v);
        }
        if let Some(v) = patch.deleted {
            note.deleted = v;
        }
        note.updated_at = now_rfc3339();
        self.save(&note)?;
        Ok(note)
    }
    pub fn permanent_delete(&self, id: &str) -> AppResult<()> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| AppError::Internal("note write lock poisoned".into()))?;
        self.storage.delete_entity("notes", id)?;
        self.invalidate_cache();
        Ok(())
    }
    fn save(&self, note: &Note) -> AppResult<()> {
        self.storage.save_entity("notes", &note.id, note)?;
        self.invalidate_cache();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::WindowBounds;
    use std::sync::Arc;

    fn temp_service(tag: &str) -> (tempfile::TempDir, NoteService) {
        let tmp = tempfile::Builder::new()
            .prefix(&format!("maydolist-note-{tag}-"))
            .tempdir()
            .unwrap();
        let storage = Arc::new(Storage::with_dir(tmp.path()).unwrap());
        (tmp, NoteService::new(storage))
    }

    #[test]
    fn concurrent_content_and_bounds_updates_preserve_both() {
        let (_tmp, service) = temp_service("rmw");
        let service = Arc::new(service);
        let note = service.create("title".into(), "initial".into()).unwrap();
        let id = note.id.clone();

        let mut handles = Vec::new();
        for i in 0..40 {
            let service = service.clone();
            let id = id.clone();
            handles.push(std::thread::spawn(move || {
                if i % 2 == 0 {
                    service
                        .update(
                            &id,
                            NotePatch {
                                content: Some(format!("content-{i}")),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                } else {
                    service
                        .update(
                            &id,
                            NotePatch {
                                window_bounds: Some(WindowBounds {
                                    x: i as f64,
                                    y: i as f64,
                                    width: 360.0,
                                    height: 280.0,
                                }),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        let final_note = service.get(&id).unwrap();
        assert!(
            final_note.content.starts_with("content-"),
            "content lost to bounds RMW: {}",
            final_note.content
        );
        assert!(
            final_note.window_bounds.is_some(),
            "window_bounds lost to content RMW"
        );
    }
}
