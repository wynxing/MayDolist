use std::sync::Arc;
pub mod focus;
pub mod github;
pub mod note;
pub mod todo;
use crate::storage::Storage;
pub struct Services {
    pub todo: Arc<todo::TodoService>,
    pub note: Arc<note::NoteService>,
    pub github: Arc<github::GithubService>,
    pub focus: Arc<focus::FocusService>,
}
impl Services {
    pub fn new(storage: Arc<Storage>) -> Self {
        let todo = Arc::new(todo::TodoService::new(storage.clone()));
        let note = Arc::new(note::NoteService::new(storage.clone()));
        let github = Arc::new(github::GithubService::new(storage));
        Self {
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
