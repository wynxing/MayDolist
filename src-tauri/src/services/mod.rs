use std::sync::Arc;
pub mod github;
pub mod note;
pub mod todo;
use crate::storage::Storage;
pub struct Services {
    pub todo: todo::TodoService,
    pub note: note::NoteService,
    pub github: github::GithubService,
}
impl Services {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            todo: todo::TodoService::new(storage.clone()),
            note: note::NoteService::new(storage.clone()),
            github: github::GithubService::new(storage),
        }
    }
}
