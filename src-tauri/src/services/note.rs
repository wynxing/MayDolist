use std::sync::Mutex;

use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::Note;

pub trait NoteService: Send + Sync {
    fn list(&self) -> Vec<Note>;
    fn create(&self, title: String, content: String) -> AppResult<Note>;
    fn update(&self, id: &str, title: Option<String>, content: Option<String>)
        -> AppResult<Note>;
}

pub struct MockNoteService {
    notes: Mutex<Vec<Note>>,
}

impl MockNoteService {
    pub fn seeded() -> Self {
        Self {
            notes: Mutex::new(vec![
                Note {
                    id: "note-1".into(),
                    title: "窗口材质备忘".into(),
                    content: "Tauri 2 原生支持 windowEffects: acrylic，无需手写 DWM。".into(),
                    created_at: "2026-08-04T10:00:00Z".into(),
                    updated_at: "2026-08-04T10:00:00Z".into(),
                },
                Note {
                    id: "note-2".into(),
                    title: "快捷键记录".into(),
                    content: "默认全局快捷键 Ctrl+Alt+M 呼出主面板。".into(),
                    created_at: "2026-08-04T11:00:00Z".into(),
                    updated_at: "2026-08-04T11:00:00Z".into(),
                },
            ]),
        }
    }
}

impl NoteService for MockNoteService {
    fn list(&self) -> Vec<Note> {
        self.notes.lock().unwrap().clone()
    }

    fn create(&self, title: String, content: String) -> AppResult<Note> {
        if title.trim().is_empty() {
            return Err(AppError::InvalidInput("title must not be empty".into()));
        }
        let now = now_rfc3339();
        let note = Note {
            id: Uuid::new_v4().to_string(),
            title,
            content,
            created_at: now.clone(),
            updated_at: now,
        };
        self.notes.lock().unwrap().push(note.clone());
        Ok(note)
    }

    fn update(
        &self,
        id: &str,
        title: Option<String>,
        content: Option<String>,
    ) -> AppResult<Note> {
        let mut notes = self.notes.lock().unwrap();
        let note = notes
            .iter_mut()
            .find(|note| note.id == id)
            .ok_or_else(|| AppError::NotFound(format!("note {id}")))?;
        if let Some(title) = title {
            if title.trim().is_empty() {
                return Err(AppError::InvalidInput("title must not be empty".into()));
            }
            note.title = title;
        }
        if let Some(content) = content {
            note.content = content;
        }
        note.updated_at = now_rfc3339();
        Ok(note.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_create_update() {
        let service = MockNoteService::seeded();
        let note = service.create("标题".into(), "内容".into()).unwrap();
        assert_eq!(service.list().len(), 3);

        let updated = service.update(&note.id, Some("新标题".into()), None).unwrap();
        assert_eq!(updated.title, "新标题");
        assert_eq!(updated.content, "内容");
        assert!(updated.updated_at >= note.updated_at);

        assert!(matches!(
            service.update("nope", None, None),
            Err(AppError::NotFound(_))
        ));
    }
}
