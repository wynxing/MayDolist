//! Linked-Todo synchronization: batched per-repo GraphQL state checks and
//! the summary / failure shapes reported back to the UI.

use std::collections::HashMap;

use serde::Serialize;

use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{GithubSyncMetadata, GithubSyncState, TodoSource};
use crate::services::todo::TodoService;

use super::{normalize_repo, GithubService};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubSyncFailure {
    pub repo: String,
    pub kind: String,
    pub number: u64,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GithubSyncSummary {
    pub checked: usize,
    pub changed: usize,
    pub auto_completed: usize,
    pub reopened: usize,
    pub failed: usize,
    pub failures: Vec<GithubSyncFailure>,
    #[serde(skip_serializing)]
    #[ts(skip)]
    pub changed_item_ids: Vec<String>,
    #[serde(skip_serializing)]
    #[ts(skip)]
    pub auto_completed_item_ids: Vec<String>,
}

type LinkedTodo = (String, Option<GithubSyncMetadata>);
type LinkedSourceGroup = (TodoSource, Vec<LinkedTodo>);

impl GithubService {
    /// Synchronize only GitHub sources already attached to local Todos. The
    /// caller decides whether the user enabled this feature and whether
    /// closed/merged sources may auto-complete Todos.
    pub fn sync_linked_todos(&self, todo: &TodoService, auto_complete: bool) -> GithubSyncSummary {
        let mut summary = GithubSyncSummary::default();
        if self.demo_mode {
            return summary;
        }
        let lists = match todo.list(false) {
            Ok(lists) => lists,
            Err(err) => {
                summary.failed = 1;
                summary.failures.push(GithubSyncFailure {
                    repo: "*".into(),
                    kind: "todo".into(),
                    number: 0,
                    message: err.to_string(),
                });
                return summary;
            }
        };
        let mut sources: HashMap<String, LinkedSourceGroup> = HashMap::new();
        for list in lists.iter() {
            for item in &list.items {
                let Some(source) = item.source.clone() else {
                    continue;
                };
                let key = format!("{}:{}:{}", source.kind, source.repo, source.number);
                sources
                    .entry(key)
                    .or_insert_with(|| (source, Vec::new()))
                    .1
                    .push((item.id.clone(), item.github_sync.clone()));
            }
        }
        if sources.is_empty() {
            return summary;
        }
        summary.checked = sources.len();

        let auth_error = if self.status().state == "authenticated" {
            None
        } else {
            Some("GitHub CLI 未登录或当前不可用".to_string())
        };
        // Group sources by repository so each repo costs one gh subprocess
        // (a single GraphQL query) instead of one subprocess per source.
        let mut by_repo: HashMap<String, Vec<LinkedSourceGroup>> = HashMap::new();
        for (_key, group) in sources {
            by_repo.entry(group.0.repo.clone()).or_default().push(group);
        }
        for (repo, entries) in by_repo {
            let now = now_rfc3339();
            let states = match &auth_error {
                Some(error) => Err(AppError::Github(error.clone())),
                None => self.fetch_linked_repo_states(&repo, &entries),
            };
            match states {
                Ok(states) => {
                    for (source, items) in entries {
                        let alias = source_alias(&source);
                        match states.get(&alias) {
                            Some(state) => {
                                let state = *state;
                                for (id, previous) in items {
                                    let reopened = previous.as_ref().is_some_and(|metadata| {
                                        matches!(
                                            metadata.state,
                                            GithubSyncState::Closed | GithubSyncState::Merged
                                        ) && metadata.auto_completed_at.is_some()
                                            && state == GithubSyncState::Open
                                    });
                                    match todo.sync_github_item(&id, state, &now, auto_complete) {
                                        Ok((_, changed, auto_completed)) => {
                                            if changed {
                                                summary.changed += 1;
                                                summary.changed_item_ids.push(id.clone());
                                            }
                                            if auto_completed {
                                                summary.auto_completed += 1;
                                                summary.auto_completed_item_ids.push(id.clone());
                                            }
                                            if reopened {
                                                summary.reopened += 1;
                                            }
                                        }
                                        Err(err) => {
                                            summary.failed += 1;
                                            summary.failures.push(GithubSyncFailure {
                                                repo: source.repo.clone(),
                                                kind: source.kind.clone(),
                                                number: source.number,
                                                message: err.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                            None => {
                                // Alias missing from the response (deleted or
                                // inaccessible item): record per source.
                                summary.failed += 1;
                                let message = "GitHub 响应中缺少该条目".to_string();
                                summary.failures.push(GithubSyncFailure {
                                    repo: source.repo.clone(),
                                    kind: source.kind.clone(),
                                    number: source.number,
                                    message: message.clone(),
                                });
                                for (id, _) in items {
                                    if let Ok((_, changed)) =
                                        todo.record_github_sync_error(&id, &message, &now)
                                    {
                                        if changed {
                                            summary.changed += 1;
                                            summary.changed_item_ids.push(id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    for (source, items) in entries {
                        summary.failed += 1;
                        summary.failures.push(GithubSyncFailure {
                            repo: source.repo.clone(),
                            kind: source.kind.clone(),
                            number: source.number,
                            message: err.to_string(),
                        });
                        for (id, _) in items {
                            if let Ok((_, changed)) =
                                todo.record_github_sync_error(&id, &err.to_string(), &now)
                            {
                                if changed {
                                    summary.changed += 1;
                                    summary.changed_item_ids.push(id);
                                }
                            }
                        }
                    }
                }
            }
        }
        summary
    }

    /// Fetch the open/closed/merged state of every linked source in `entries`
    /// (all belonging to one repository) with a single GraphQL request, so a
    /// sync pass spawns one `gh` subprocess per repo instead of per source.
    fn fetch_linked_repo_states(
        &self,
        repo: &str,
        entries: &[LinkedSourceGroup],
    ) -> AppResult<HashMap<String, GithubSyncState>> {
        let repo = normalize_repo(repo)?;
        let (owner, name) = repo
            .split_once('/')
            .ok_or_else(|| AppError::InvalidInput("repository must be owner/repo".into()))?;
        let mut fields = Vec::new();
        for (source, _) in entries {
            if source.kind == "github-pr" {
                fields.push(format!(
                    "{}: pullRequest(number: {}) {{ state }}",
                    source_alias(source),
                    source.number
                ));
            } else if source.kind == "github-issue" {
                fields.push(format!(
                    "{}: issue(number: {}) {{ state }}",
                    source_alias(source),
                    source.number
                ));
            } else {
                return Err(AppError::InvalidInput(format!(
                    "unsupported GitHub source type: {}",
                    source.kind
                )));
            }
        }
        let query = format!(
            "query {{ repository(owner: \"{owner}\", name: \"{name}\") {{ {} }} }}",
            fields.join(" ")
        );
        let response: serde_json::Value =
            self.gh(&["api", "graphql", "-f", &format!("query={query}")])?;
        if let Some(errors) = response.get("errors") {
            if errors.is_array() && !errors.as_array().is_some_and(Vec::is_empty) {
                let message = errors[0]
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("GraphQL error");
                return Err(AppError::Github(message.to_string()));
            }
        }
        parse_linked_repo_states(&response)
    }
}

/// Stable GraphQL alias for a linked source, e.g. `pr123` / `issue42`.
pub(super) fn source_alias(source: &TodoSource) -> String {
    let prefix = if source.kind == "github-pr" {
        "pr"
    } else {
        "issue"
    };
    format!("{prefix}{}", source.number)
}

/// Map a GraphQL `repository` response to sync states keyed by alias. Aliases
/// with a null node (deleted / inaccessible item) are omitted so the caller
/// records a per-source failure.
pub(super) fn parse_linked_repo_states(
    response: &serde_json::Value,
) -> AppResult<HashMap<String, GithubSyncState>> {
    let repo = response
        .pointer("/data/repository")
        .ok_or_else(|| AppError::Github("invalid graphql response: missing repository".into()))?;
    let object = repo
        .as_object()
        .ok_or_else(|| AppError::Github("invalid graphql response".into()))?;
    let mut states = HashMap::new();
    for (alias, node) in object {
        let Some(state) = node.get("state").and_then(|v| v.as_str()) else {
            continue;
        };
        let value = match state {
            "OPEN" => GithubSyncState::Open,
            "MERGED" => GithubSyncState::Merged,
            _ => GithubSyncState::Closed,
        };
        states.insert(alias.clone(), value);
    }
    Ok(states)
}
