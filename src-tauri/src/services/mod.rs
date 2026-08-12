use std::sync::Arc;
pub mod backup;
pub mod focus;
pub mod github;
pub mod note;
pub mod reminder;
pub mod todo;
use crate::storage::Storage;
pub struct Services {
    pub backup: Arc<backup::BackupService>,
    pub todo: Arc<todo::TodoService>,
    pub note: Arc<note::NoteService>,
    pub github: Arc<github::GithubService>,
    pub focus: Arc<focus::FocusService>,
}
impl Services {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self::new_with_mode(storage, false)
    }

    pub fn new_with_mode(storage: Arc<Storage>, demo_mode: bool) -> Self {
        let backup = Arc::new(backup::BackupService::new(storage.clone()));
        let todo = Arc::new(todo::TodoService::new(storage.clone()));
        let note = Arc::new(note::NoteService::new(storage.clone()));
        let github = Arc::new(github::GithubService::new_with_mode(storage, demo_mode));
        Self {
            backup,
            focus: Arc::new(focus::FocusService::new(
                todo.clone(),
                note.clone(),
                github.clone(),
            )),
            todo,
            note,
            github,
        }
    }
}
