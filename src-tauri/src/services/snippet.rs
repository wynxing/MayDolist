use std::sync::Mutex;

use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::Snippet;

pub trait SnippetService: Send + Sync {
    fn list(&self) -> Vec<Snippet>;
    fn create(&self, title: String, content: String, tags: Vec<String>) -> AppResult<Snippet>;
    fn update(
        &self,
        id: &str,
        title: Option<String>,
        content: Option<String>,
        tags: Option<Vec<String>>,
    ) -> AppResult<Snippet>;
    fn delete(&self, id: &str) -> AppResult<()>;
}

pub struct MockSnippetService {
    snippets: Mutex<Vec<Snippet>>,
}

impl MockSnippetService {
    pub fn seeded() -> Self {
        Self {
            snippets: Mutex::new(vec![
                Snippet {
                    id: "snippet-1".into(),
                    title: "原子写模式".into(),
                    content: "临时文件 + rename，先写盘后广播。".into(),
                    tags: vec!["rust".into(), "storage".into()],
                    created_at: "2026-08-03T09:00:00Z".into(),
                    updated_at: "2026-08-03T09:00:00Z".into(),
                },
                Snippet {
                    id: "snippet-2".into(),
                    title: "gh api 分页".into(),
                    content: "gh api --paginate /repos/{owner}/{repo}/issues".into(),
                    tags: vec!["github".into()],
                    created_at: "2026-08-03T10:00:00Z".into(),
                    updated_at: "2026-08-03T10:00:00Z".into(),
                },
            ]),
        }
    }
}

impl SnippetService for MockSnippetService {
    fn list(&self) -> Vec<Snippet> {
        self.snippets.lock().unwrap().clone()
    }

    fn create(&self, title: String, content: String, tags: Vec<String>) -> AppResult<Snippet> {
        if title.trim().is_empty() {
            return Err(AppError::InvalidInput("title must not be empty".into()));
        }
        let now = now_rfc3339();
        let snippet = Snippet {
            id: Uuid::new_v4().to_string(),
            title,
            content,
            tags,
            created_at: now.clone(),
            updated_at: now,
        };
        self.snippets.lock().unwrap().push(snippet.clone());
        Ok(snippet)
    }

    fn update(
        &self,
        id: &str,
        title: Option<String>,
        content: Option<String>,
        tags: Option<Vec<String>>,
    ) -> AppResult<Snippet> {
        let mut snippets = self.snippets.lock().unwrap();
        let snippet = snippets
            .iter_mut()
            .find(|snippet| snippet.id == id)
            .ok_or_else(|| AppError::NotFound(format!("snippet {id}")))?;
        if let Some(title) = title {
            if title.trim().is_empty() {
                return Err(AppError::InvalidInput("title must not be empty".into()));
            }
            snippet.title = title;
        }
        if let Some(content) = content {
            snippet.content = content;
        }
        if let Some(tags) = tags {
            snippet.tags = tags;
        }
        snippet.updated_at = now_rfc3339();
        Ok(snippet.clone())
    }

    fn delete(&self, id: &str) -> AppResult<()> {
        let mut snippets = self.snippets.lock().unwrap();
        let index = snippets
            .iter()
            .position(|snippet| snippet.id == id)
            .ok_or_else(|| AppError::NotFound(format!("snippet {id}")))?;
        snippets.remove(index);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_crud_flow() {
        let service = MockSnippetService::seeded();
        let snippet = service
            .create("标题".into(), "内容".into(), vec!["tag".into()])
            .unwrap();
        assert_eq!(service.list().len(), 3);

        let updated = service
            .update(&snippet.id, None, None, Some(vec!["a".into(), "b".into()]))
            .unwrap();
        assert_eq!(updated.tags, vec!["a", "b"]);

        service.delete(&snippet.id).unwrap();
        assert_eq!(service.list().len(), 2);
        assert!(matches!(service.delete("nope"), Err(AppError::NotFound(_))));
    }
}
