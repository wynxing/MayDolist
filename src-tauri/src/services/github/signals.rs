//! Snapshot post-processing: action signals, ignore / pin preferences and
//! the stable display ordering. Pure functions over `RepoSnapshot`, shared
//! by refresh and watch-list mutations.

use crate::models::{ActionSignal, GhIgnoredItem, RepoSnapshot};

use super::refresh::FetchedItem;

/// Recompute the stable action signals for every open item from the parsed
/// response fields. Closed / merged items keep an empty signal list so the
/// existing display rules (dimmed, no action badges) stay intact.
pub(super) fn apply_signals(snapshot: &mut RepoSnapshot, user: &str, stale_days: u32, now: &str) {
    for pr in &mut snapshot.pull_requests {
        pr.signals = if pr.state == "open" {
            compute_signals(SignalInputs {
                is_pr: true,
                draft: pr.draft,
                assignees: pr.assignees.clone(),
                reviewers: pr.reviewers.clone(),
                checks_state: pr.checks_state.clone(),
                matches: pr.matches.clone(),
                user: user.into(),
                stale_days,
                updated_at: pr.updated_at.clone(),
                now: now.into(),
            })
        } else {
            Vec::new()
        };
    }
    for issue in &mut snapshot.issues {
        issue.signals = if issue.state == "open" {
            compute_signals(SignalInputs {
                is_pr: false,
                draft: false,
                assignees: issue.assignees.clone(),
                reviewers: vec![],
                checks_state: None,
                matches: issue.matches.clone(),
                user: user.into(),
                stale_days,
                updated_at: issue.updated_at.clone(),
                now: now.into(),
            })
        } else {
            Vec::new()
        };
    }
}

/// Map parsed GitHub response fields onto the stable signal set. The UI only
/// ever consumes `ActionSignal` values, never raw GitHub strings.
pub(super) struct SignalInputs {
    pub(super) is_pr: bool,
    pub(super) draft: bool,
    pub(super) assignees: Vec<String>,
    pub(super) reviewers: Vec<String>,
    pub(super) checks_state: Option<String>,
    pub(super) matches: Vec<String>,
    pub(super) user: String,
    pub(super) stale_days: u32,
    pub(super) updated_at: String,
    pub(super) now: String,
}

pub(super) fn compute_signals(input: SignalInputs) -> Vec<ActionSignal> {
    let mut signals = Vec::new();
    let needs_action = input
        .matches
        .iter()
        .any(|m| matches!(m.as_str(), "assigned" | "mentioned" | "involved" | "pinned"))
        || input.assignees.contains(&input.user);
    if needs_action {
        signals.push(ActionSignal::NeedsAction);
    }
    if input.is_pr && input.reviewers.contains(&input.user) {
        signals.push(ActionSignal::NeedsReview);
    }
    if input.is_pr
        && input
            .checks_state
            .as_deref()
            .map(|state| state == "failure" || state == "error")
            .unwrap_or(false)
    {
        signals.push(ActionSignal::CiFailed);
    }
    if crate::models::github::is_stale(&input.updated_at, input.stale_days, &input.now) {
        signals.push(ActionSignal::Stale);
    }
    if input.is_pr && input.draft {
        signals.push(ActionSignal::Draft);
    }
    signals
}

pub(super) fn apply_watch_prefs(
    snapshot: &mut RepoSnapshot,
    ignored: &[GhIgnoredItem],
    pinned: &[u64],
) {
    snapshot.pull_requests.retain(|pr| {
        !ignored
            .iter()
            .any(|v| v.number == pr.number && v.kind == "pr")
    });
    snapshot.issues.retain(|issue| {
        !ignored
            .iter()
            .any(|v| v.number == issue.number && v.kind == "issue")
    });
    for pr in &mut snapshot.pull_requests {
        if pinned.contains(&pr.number) && !pr.matches.iter().any(|m| m == "pinned") {
            pr.matches.push("pinned".into());
        }
    }
    for issue in &mut snapshot.issues {
        if pinned.contains(&issue.number) && !issue.matches.iter().any(|m| m == "pinned") {
            issue.matches.push("pinned".into());
        }
    }
}

pub(super) fn sort_snapshot(snapshot: &mut RepoSnapshot) {
    let pinned = |matches: &[String]| matches.iter().any(|m| m == "pinned");
    let actionable = |signals: &[ActionSignal]| {
        signals
            .iter()
            .filter(|signal| signal.is_actionable())
            .count()
    };
    // Pinned first, then items with more actionable signals, then by update
    // recency, then by number descending (stable and explainable).
    snapshot.pull_requests.sort_by(|a, b| {
        pinned(&b.matches)
            .cmp(&pinned(&a.matches))
            .then_with(|| actionable(&b.signals).cmp(&actionable(&a.signals)))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| b.number.cmp(&a.number))
    });
    snapshot.issues.sort_by(|a, b| {
        pinned(&b.matches)
            .cmp(&pinned(&a.matches))
            .then_with(|| actionable(&b.signals).cmp(&actionable(&a.signals)))
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| b.number.cmp(&a.number))
    });
}

pub(super) fn merge_pinned_item(snapshot: &mut RepoSnapshot, item: FetchedItem) {
    match item {
        FetchedItem::Pr(pr) => {
            snapshot.issues.retain(|v| v.number != pr.number);
            if let Some(existing) = snapshot
                .pull_requests
                .iter_mut()
                .find(|v| v.number == pr.number)
            {
                if !existing.matches.iter().any(|m| m == "pinned") {
                    existing.matches.push("pinned".into());
                }
            } else {
                snapshot.pull_requests.push(pr);
            }
        }
        FetchedItem::Issue(issue) => {
            snapshot.pull_requests.retain(|v| v.number != issue.number);
            if let Some(existing) = snapshot
                .issues
                .iter_mut()
                .find(|v| v.number == issue.number)
            {
                if !existing.matches.iter().any(|m| m == "pinned") {
                    existing.matches.push("pinned".into());
                }
            } else {
                snapshot.issues.push(issue);
            }
        }
    }
}

pub(super) fn strip_pin_marker(snapshot: &mut RepoSnapshot, number: u64) {
    if let Some(pr) = snapshot
        .pull_requests
        .iter_mut()
        .find(|v| v.number == number)
    {
        pr.matches.retain(|m| m != "pinned");
        if pr.matches.is_empty() {
            snapshot.pull_requests.retain(|v| v.number != number);
        }
    }
    if let Some(issue) = snapshot.issues.iter_mut().find(|v| v.number == number) {
        issue.matches.retain(|m| m != "pinned");
        if issue.matches.is_empty() {
            snapshot.issues.retain(|v| v.number != number);
        }
    }
}
