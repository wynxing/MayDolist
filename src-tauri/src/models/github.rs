use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Stable, UI-independent action signals for GitHub items. The UI only ever
/// consumes this enum (serialized camelCase); it never depends on raw GitHub
/// strings such as filter names or API status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionSignal {
    /// 需要我处理：被分配 / 被提及 / 参与 / 手动关注的条目。
    NeedsAction,
    /// 需要 Review：当前用户被请求 review 的 PR。
    NeedsReview,
    /// CI 失败：PR 的检查状态为失败 / 错误。
    CiFailed,
    /// 长期未更新：open 条目超过配置天数未更新。
    Stale,
    /// Draft PR：草稿 PR，不可合并。
    Draft,
}

impl ActionSignal {
    /// Stable serialized name (mirrors the camelCase JSON value).
    pub fn as_str(&self) -> &'static str {
        match self {
            ActionSignal::NeedsAction => "needsAction",
            ActionSignal::NeedsReview => "needsReview",
            ActionSignal::CiFailed => "ciFailed",
            ActionSignal::Stale => "stale",
            ActionSignal::Draft => "draft",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "needsAction" => Some(ActionSignal::NeedsAction),
            "needsReview" => Some(ActionSignal::NeedsReview),
            "ciFailed" => Some(ActionSignal::CiFailed),
            "stale" => Some(ActionSignal::Stale),
            "draft" => Some(ActionSignal::Draft),
            _ => None,
        }
    }

    /// Signals that mean the item itself needs attention. `Draft` is purely
    /// informational and never pulls an item into the Focus view by itself.
    pub fn is_actionable(&self) -> bool {
        !matches!(self, ActionSignal::Draft)
    }
}

/// True when `updated_at` is older than `stale_days` relative to `now`
/// (boundary inclusive). Unparseable timestamps degrade to "not stale" so a
/// missing field never misreports an item.
pub fn is_stale(updated_at: &str, stale_days: u32, now: &str) -> bool {
    if stale_days == 0 {
        return false;
    }
    let (Ok(updated), Ok(now)) = (
        DateTime::parse_from_rfc3339(updated_at),
        DateTime::parse_from_rfc3339(now),
    ) else {
        return false;
    };
    let updated = updated.with_timezone(&Utc);
    let now = now.with_timezone(&Utc);
    now.signed_duration_since(updated) >= chrono::Duration::days(i64::from(stale_days))
}

/// Keep the `Stale` signal in sync with `updated_at` without touching the
/// API-derived signals. Called whenever a snapshot is served so stale never
/// depends on the last refresh moment.
pub fn refresh_stale(
    signals: &mut Vec<ActionSignal>,
    updated_at: &str,
    stale_days: u32,
    now: &str,
) {
    let stale_now = is_stale(updated_at, stale_days, now);
    let has_stale = signals.contains(&ActionSignal::Stale);
    match (stale_now, has_stale) {
        (true, false) => signals.push(ActionSignal::Stale),
        (false, true) => signals.retain(|s| *s != ActionSignal::Stale),
        _ => {}
    }
}

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
    /// Action-signal filters; empty means no signal filtering, which keeps
    /// the legacy behavior for existing users.
    #[serde(default)]
    pub signal_filters: Vec<String>,
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
    /// Assignee logins; empty when the API response / old cache lacks them.
    #[serde(default)]
    pub assignees: Vec<String>,
    /// Stable action signals computed from the response; empty for old caches.
    #[serde(default)]
    pub signals: Vec<ActionSignal>,
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
    /// Assignee logins; empty when the API response / old cache lacks them.
    #[serde(default)]
    pub assignees: Vec<String>,
    /// Requested reviewer logins (only meaningful for open PRs).
    #[serde(default)]
    pub reviewers: Vec<String>,
    /// Head commit SHA used for check lookups; absent for old caches.
    #[serde(default)]
    pub head_sha: Option<String>,
    /// Summary of the check state (`success` / `pending` / `failure` /
    /// `error`); `None` when the API did not answer.
    #[serde(default)]
    pub checks_state: Option<String>,
    /// Stable action signals computed from the response; empty for old caches.
    #[serde(default)]
    pub signals: Vec<ActionSignal>,
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
    /// When the persisted signals were computed; `None` for pre-signal caches.
    #[serde(default)]
    pub signals_computed_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_signal_names_are_stable_and_roundtrip() {
        for (signal, name) in [
            (ActionSignal::NeedsAction, "needsAction"),
            (ActionSignal::NeedsReview, "needsReview"),
            (ActionSignal::CiFailed, "ciFailed"),
            (ActionSignal::Stale, "stale"),
            (ActionSignal::Draft, "draft"),
        ] {
            assert_eq!(signal.as_str(), name);
            assert_eq!(ActionSignal::from_str(name), Some(signal));
        }
        assert_eq!(ActionSignal::from_str("review_requested"), None);
        assert_eq!(ActionSignal::from_str(""), None);
    }

    #[test]
    fn action_signals_serialize_as_camel_case() {
        let json = serde_json::to_string(&vec![
            ActionSignal::NeedsAction,
            ActionSignal::NeedsReview,
            ActionSignal::CiFailed,
            ActionSignal::Stale,
            ActionSignal::Draft,
        ])
        .unwrap();
        assert_eq!(
            json,
            r#"["needsAction","needsReview","ciFailed","stale","draft"]"#
        );
        let parsed: Vec<ActionSignal> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed,
            vec![
                ActionSignal::NeedsAction,
                ActionSignal::NeedsReview,
                ActionSignal::CiFailed,
                ActionSignal::Stale,
                ActionSignal::Draft,
            ]
        );
    }

    #[test]
    fn draft_is_not_actionable_alone() {
        assert!(!ActionSignal::Draft.is_actionable());
        for signal in [
            ActionSignal::NeedsAction,
            ActionSignal::NeedsReview,
            ActionSignal::CiFailed,
            ActionSignal::Stale,
        ] {
            assert!(signal.is_actionable());
        }
    }

    #[test]
    fn legacy_snapshot_without_signal_fields_reads_with_defaults() {
        let json = r#"{
            "schemaVersion": 1,
            "repo": "owner/repo",
            "fetchedAt": "2026-08-01T00:00:00Z",
            "lastSuccessAt": "2026-08-01T00:00:00Z",
            "lastError": null,
            "issues": [{
                "number": 1,
                "title": "old issue",
                "state": "open",
                "url": "https://github.com/owner/repo/issues/1",
                "updatedAt": "2026-07-01T00:00:00Z",
                "kind": "issue",
                "matches": ["mine"]
            }],
            "pullRequests": [{
                "number": 2,
                "title": "old pr",
                "state": "open",
                "draft": false,
                "url": "https://github.com/owner/repo/pull/2",
                "updatedAt": "2026-07-01T00:00:00Z",
                "matches": ["mine"]
            }]
        }"#;
        let snapshot: RepoSnapshot = serde_json::from_str(json).unwrap();
        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.signals_computed_at, None);
        assert!(snapshot.issues[0].signals.is_empty());
        assert!(snapshot.issues[0].assignees.is_empty());
        assert!(snapshot.pull_requests[0].signals.is_empty());
        assert!(snapshot.pull_requests[0].assignees.is_empty());
        assert!(snapshot.pull_requests[0].reviewers.is_empty());
        assert_eq!(snapshot.pull_requests[0].head_sha, None);
        assert_eq!(snapshot.pull_requests[0].checks_state, None);
    }

    #[test]
    fn signal_fields_roundtrip_camel_case() {
        let mut pr = GhPullRequest {
            number: 2,
            title: "pr".into(),
            state: "open".into(),
            draft: true,
            url: "https://github.com/owner/repo/pull/2".into(),
            updated_at: "2026-07-01T00:00:00Z".into(),
            matches: vec!["pinned".into()],
            assignees: vec!["alice".into()],
            reviewers: vec!["wynxing".into()],
            head_sha: Some("abc123".into()),
            checks_state: Some("failure".into()),
            signals: vec![
                ActionSignal::NeedsAction,
                ActionSignal::NeedsReview,
                ActionSignal::CiFailed,
                ActionSignal::Draft,
            ],
        };
        let snapshot = RepoSnapshot {
            schema_version: 2,
            repo: "owner/repo".into(),
            fetched_at: "2026-08-01T00:00:00Z".into(),
            last_success_at: Some("2026-08-01T00:00:00Z".into()),
            last_error: None,
            issues: vec![],
            pull_requests: vec![pr.clone()],
            signals_computed_at: Some("2026-08-01T00:00:00Z".into()),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"signalsComputedAt\":\"2026-08-01T00:00:00Z\""));
        assert!(
            json.contains("\"signals\":[\"needsAction\",\"needsReview\",\"ciFailed\",\"draft\"]")
        );
        assert!(json.contains("\"headSha\":\"abc123\""));
        assert!(json.contains("\"checksState\":\"failure\""));
        assert!(json.contains("\"reviewers\":[\"wynxing\"]"));
        let restored: RepoSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, snapshot);
        assert_eq!(restored.pull_requests[0], pr);
        pr.signals.clear();
        assert_ne!(restored.pull_requests[0], pr);
    }

    #[test]
    fn stale_boundary_is_inclusive_and_tolerant() {
        let now = "2026-08-15T00:00:00Z";
        // Exactly `stale_days` old is stale (boundary inclusive).
        assert!(is_stale("2026-08-01T00:00:00Z", 14, now));
        // One second younger is not stale yet.
        assert!(!is_stale("2026-08-01T00:00:01Z", 14, now));
        // zero days disables the signal.
        assert!(!is_stale("2026-01-01T00:00:00Z", 0, now));
        // Unparseable inputs degrade to not-stale.
        assert!(!is_stale("not-a-date", 14, now));
        assert!(!is_stale("2026-08-01T00:00:00Z", 14, "invalid"));
        // Future timestamps are never stale.
        assert!(!is_stale("2026-09-01T00:00:00Z", 14, now));
    }

    #[test]
    fn refresh_stale_adds_and_removes_signal() {
        let mut signals = vec![ActionSignal::NeedsAction];
        refresh_stale(
            &mut signals,
            "2026-07-01T00:00:00Z",
            14,
            "2026-08-15T00:00:00Z",
        );
        assert!(signals.contains(&ActionSignal::Stale));
        refresh_stale(
            &mut signals,
            "2026-08-10T00:00:00Z",
            14,
            "2026-08-15T00:00:00Z",
        );
        assert!(!signals.contains(&ActionSignal::Stale));
        assert!(signals.contains(&ActionSignal::NeedsAction));
        // Disabled stale never adds the signal.
        let mut signals = vec![];
        refresh_stale(
            &mut signals,
            "2026-01-01T00:00:00Z",
            0,
            "2026-08-15T00:00:00Z",
        );
        assert!(signals.is_empty());
    }
}
