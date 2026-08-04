use std::sync::Arc;

pub mod github;
pub mod note;
pub mod snippet;
pub mod todo;

pub use github::{GithubService, MockGithubService};
pub use note::{MockNoteService, NoteService};
pub use snippet::{MockSnippetService, SnippetService};
pub use todo::{MockTodoService, TodoService};

/// Container for every domain service. Skeleton phase uses in-memory mocks so
/// the whole UI data flow is exercisable end-to-end without touching disk.
pub struct Services {
    pub todo: Arc<dyn TodoService>,
    pub note: Arc<dyn NoteService>,
    pub snippet: Arc<dyn SnippetService>,
    pub github: Arc<dyn GithubService>,
}

impl Services {
    pub fn mock() -> Self {
        Self {
            todo: Arc::new(MockTodoService::seeded()),
            note: Arc::new(MockNoteService::seeded()),
            snippet: Arc::new(MockSnippetService::seeded()),
            github: Arc::new(MockGithubService::seeded()),
        }
    }
}
