use std::sync::Arc;
pub mod github;
pub mod note;
pub mod todo;
use crate::storage::Storage;
pub struct Services {
    pub todo: Arc<todo::TodoService>,
    pub note: Arc<note::NoteService>,
    pub github: Arc<github::GithubService>,
}
impl Services {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            todo: Arc::new(todo::TodoService::new(storage.clone())),
            note: Arc::new(note::NoteService::new(storage.clone())),
            github: Arc::new(github::GithubService::new(storage)),
        }
    }
}
