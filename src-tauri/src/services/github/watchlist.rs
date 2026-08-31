//! Watch-list management: repository subscriptions, filters, collapsed state
//! and the ignore / pin item preferences applied onto cached snapshots.

use crate::error::{AppError, AppResult};
use crate::events::now_rfc3339;
use crate::models::{GhIgnoredItem, RepoSnapshot, RepoWatch};

use super::signals::{
    apply_signals, apply_watch_prefs, merge_pinned_item, sort_snapshot, strip_pin_marker,
};
use super::{cache_path, normalize_repo, GithubService, DEFAULT_FILTERS, FILTERS, SIGNAL_FILTERS};

impl GithubService {
    pub fn watchlist(&self) -> AppResult<Vec<RepoWatch>> {
        let path = self.storage.data_dir().join("github/watchlist.json");
        Ok(self.storage.read_json(&path)?.unwrap_or_default())
    }
    fn save_watchlist(&self, list: &[RepoWatch]) -> AppResult<()> {
        self.storage
            .write_json(&self.storage.data_dir().join("github/watchlist.json"), list)
    }
    pub fn add_watch(&self, name: String) -> AppResult<Vec<RepoWatch>> {
        let full_name = normalize_repo(&name)?;
        if !self.demo_mode {
            self.run(&["api", &format!("repos/{full_name}"), "--jq", ".full_name"])?;
        }
        let mut list = self.watchlist()?;
        if !list
            .iter()
            .any(|v| v.full_name.eq_ignore_ascii_case(&full_name))
        {
            list.push(RepoWatch {
                full_name,
                filters: DEFAULT_FILTERS.iter().map(|v| v.to_string()).collect(),
                collapsed: false,
                ignored: vec![],
                pinned: vec![],
                signal_filters: vec![],
            });
            self.save_watchlist(&list)?;
        }
        Ok(list)
    }
    pub fn remove_watch(&self, name: &str) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        list.retain(|v| !v.full_name.eq_ignore_ascii_case(name));
        self.save_watchlist(&list)?;
        Ok(list)
    }
    pub fn set_filters(&self, name: &str, filters: Vec<String>) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.filters = filters
            .into_iter()
            .filter(|v| FILTERS.contains(&v.as_str()))
            .collect();
        self.save_watchlist(&list)?;
        Ok(list)
    }

    /// Action-signal filters. An empty list means "no signal filtering",
    /// which preserves the legacy list behavior for existing users.
    pub fn set_signal_filters(
        &self,
        name: &str,
        filters: Vec<String>,
    ) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.signal_filters = filters
            .into_iter()
            .filter(|v| SIGNAL_FILTERS.contains(&v.as_str()))
            .collect();
        self.save_watchlist(&list)?;
        Ok(list)
    }

    pub fn set_collapsed(&self, name: &str, collapsed: bool) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.collapsed = collapsed;
        self.save_watchlist(&list)?;
        Ok(list)
    }

    pub fn ignore_item(&self, name: &str, number: u64, kind: String) -> AppResult<Vec<RepoWatch>> {
        if kind != "pr" && kind != "issue" {
            return Err(AppError::InvalidInput(
                "kind must be \"pr\" or \"issue\"".into(),
            ));
        }
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.pinned.retain(|&n| n != number);
        if !item
            .ignored
            .iter()
            .any(|v| v.number == number && v.kind == kind)
        {
            item.ignored.push(GhIgnoredItem {
                number,
                kind: kind.clone(),
            });
        }
        let ignored = item.ignored.clone();
        let pinned = item.pinned.clone();
        self.save_watchlist(&list)?;
        if let Some(mut snap) = self.snapshot(name)? {
            apply_watch_prefs(&mut snap, &ignored, &pinned);
            self.storage
                .write_json(&cache_path(&self.storage, name), &snap)?;
        }
        Ok(list)
    }

    pub fn pin_item(&self, name: &str, number: u64) -> AppResult<RepoSnapshot> {
        if self.demo_mode {
            return self
                .snapshot(name)?
                .ok_or_else(|| AppError::Github("Demo 仓库没有模拟快照".into()));
        }
        let full_name = normalize_repo(name)?;
        let fetched = self.fetch_issue_or_pr(&full_name, number)?;
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == full_name)
            .ok_or_else(|| AppError::NotFound(full_name.clone()))?;
        item.ignored.retain(|v| v.number != number);
        if !item.pinned.contains(&number) {
            item.pinned.push(number);
        }
        let ignored = item.ignored.clone();
        let pinned = item.pinned.clone();
        self.save_watchlist(&list)?;

        let user = self
            .status()
            .user
            .ok_or_else(|| AppError::Github("GitHub CLI 未登录或离线".into()))?;
        let now = now_rfc3339();
        let mut snapshot = self.snapshot(&full_name)?.unwrap_or_else(|| RepoSnapshot {
            schema_version: 2,
            repo: full_name.clone(),
            fetched_at: now.clone(),
            last_success_at: None,
            last_error: None,
            issues: vec![],
            pull_requests: vec![],
            signals_computed_at: Some(now.clone()),
        });
        merge_pinned_item(&mut snapshot, fetched);
        apply_watch_prefs(&mut snapshot, &ignored, &pinned);
        apply_signals(&mut snapshot, &user, self.stale_days(), &now);
        sort_snapshot(&mut snapshot);
        self.storage
            .write_json(&cache_path(&self.storage, &full_name), &snapshot)?;
        Ok(snapshot)
    }

    pub fn unpin_item(&self, name: &str, number: u64) -> AppResult<Vec<RepoWatch>> {
        let mut list = self.watchlist()?;
        let item = list
            .iter_mut()
            .find(|v| v.full_name == name)
            .ok_or_else(|| AppError::NotFound(name.into()))?;
        item.pinned.retain(|&n| n != number);
        let ignored = item.ignored.clone();
        let pinned = item.pinned.clone();
        self.save_watchlist(&list)?;
        if let Some(mut snap) = self.snapshot(name)? {
            strip_pin_marker(&mut snap, number);
            apply_watch_prefs(&mut snap, &ignored, &pinned);
            sort_snapshot(&mut snap);
            self.storage
                .write_json(&cache_path(&self.storage, name), &snap)?;
        }
        Ok(list)
    }
}
