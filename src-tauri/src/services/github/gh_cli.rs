//! `gh` CLI subprocess runner plus the raw GitHub API response shapes.
//!
//! Parse / process-execution only: no business logic, so refresh / sync can
//! be exercised against fixture JSON via the `GhRunner` injection point.

use std::process::Command;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::models::{GhIssue, GhPullRequest};

pub(super) fn run_gh(args: &[&str]) -> AppResult<String> {
    let mut command = Command::new("gh");
    command
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: keep the console hidden in release builds.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| AppError::Github(e.to_string()))?;
    let mut stdout = child.stdout.take().expect("stdout must be piped");
    let mut stderr = child.stderr.take().expect("stderr must be piped");
    let out_thread = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        buf
    });
    let err_thread = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        buf
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::Github("gh timed out after 30s".into()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(AppError::Github(err.to_string())),
        }
    };
    let stdout = out_thread
        .join()
        .map_err(|_| AppError::Github("gh output thread panicked".into()))?;
    let stderr = err_thread
        .join()
        .map_err(|_| AppError::Github("gh stderr thread panicked".into()))?;
    if !status.success() {
        return Err(AppError::Github(stderr.trim().to_string()));
    }
    Ok(stdout)
}

#[derive(Deserialize, Clone)]
pub(super) struct SearchResult {
    pub(super) items: Vec<ApiItem>,
}

#[derive(Deserialize, Clone)]
pub(super) struct GhUser {
    pub(super) login: String,
}

#[derive(Deserialize, Clone)]
pub(super) struct ApiItem {
    pub(super) number: u64,
    pub(super) title: String,
    pub(super) state: String,
    pub(super) html_url: String,
    pub(super) updated_at: String,
    #[serde(default)]
    pub(super) draft: bool,
    #[serde(default)]
    pub(super) assignees: Vec<GhUser>,
    pub(super) pull_request: Option<serde_json::Value>,
}

impl ApiItem {
    pub(super) fn into_pr(self, matches: Vec<String>) -> GhPullRequest {
        GhPullRequest {
            number: self.number,
            title: self.title,
            state: self.state,
            draft: self.draft,
            url: self.html_url,
            updated_at: self.updated_at,
            matches,
            assignees: self.assignees.iter().map(|u| u.login.clone()).collect(),
            reviewers: vec![],
            head_sha: None,
            checks_state: None,
            signals: vec![],
        }
    }
    pub(super) fn into_issue(self, matches: Vec<String>) -> GhIssue {
        GhIssue {
            number: self.number,
            title: self.title,
            state: self.state,
            url: self.html_url,
            updated_at: self.updated_at,
            kind: "issue".into(),
            matches,
            assignees: self.assignees.iter().map(|u| u.login.clone()).collect(),
            signals: vec![],
        }
    }
}

/// Full PR response (`repos/{owner}/{repo}/pulls/{number}`), used for the
/// fields the search / list endpoints do not return: requested reviewers and
/// the head commit SHA for check lookups.
#[derive(Deserialize, Clone)]
pub(super) struct PullDetail {
    #[serde(default)]
    pub(super) draft: bool,
    pub(super) updated_at: String,
    #[serde(default)]
    pub(super) assignees: Vec<GhUser>,
    #[serde(default)]
    pub(super) requested_reviewers: Vec<GhUser>,
    pub(super) head: PullHead,
}

#[derive(Deserialize, Clone)]
pub(super) struct PullHead {
    pub(super) sha: String,
}

/// `GET /repos/{owner}/{repo}/commits/{sha}/status` (classic commit statuses).
#[derive(Deserialize)]
pub(super) struct CombinedStatus {
    pub(super) state: String,
    #[serde(default)]
    pub(super) statuses: Vec<serde_json::Value>,
}

/// `GET /repos/{owner}/{repo}/commits/{sha}/check-runs`.
#[derive(Deserialize)]
pub(super) struct CheckRuns {
    #[serde(default)]
    pub(super) check_runs: Vec<CheckRun>,
}

#[derive(Deserialize)]
pub(super) struct CheckRun {
    #[serde(default)]
    pub(super) conclusion: Option<String>,
}
