use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GhAuthStatus {
    pub state: String,
    pub logged_in: bool,
    pub user: Option<String>,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GhIgnoredItem {
    pub number: u64,
    /// `"pr"` or `"issue"`.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoWatch {
    pub full_name: String,
    pub filters: Vec<String>,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub ignored: Vec<GhIgnoredItem>,
    #[serde(default)]
    pub pinned: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GhIssue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub url: String,
    pub updated_at: String,
    pub kind: String,
    pub matches: Vec<String>,
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
    pub matches: Vec<String>,
}

/// Snapshot cache for one watched repository, mirroring `github/cache/<repo>.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoSnapshot {
    pub schema_version: u32,
    pub repo: String,
    pub fetched_at: String,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub issues: Vec<GhIssue>,
    pub pull_requests: Vec<GhPullRequest>,
}
