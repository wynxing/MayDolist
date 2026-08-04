use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GhAuthStatus {
    pub logged_in: bool,
    pub user: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoWatch {
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GhIssue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GhPullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub draft: bool,
    pub url: String,
    pub updated_at: String,
}

/// Snapshot cache for one watched repository, mirroring `github/cache/<repo>.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoSnapshot {
    pub repo: String,
    pub fetched_at: String,
    pub issues: Vec<GhIssue>,
    pub pull_requests: Vec<GhPullRequest>,
}
