//! Isolated, deterministic data used by the README screenshot workflow.
//!
//! Demo mode is intentionally implemented below the Tauri command boundary so
//! the screenshots exercise the same stores and UI as a normal application.
//! It never uses the user's configured data directory or GitHub credentials.

use std::fs;
use std::path::Path;

use crate::error::AppResult;
use crate::models::{
    ActionSignal, AppConfig, GhIssue, GhPullRequest, Note, RepoSnapshot, RepoWatch, TodoItem,
    TodoList, TodoSource,
};
use crate::storage::Storage;

const DEMO_REPO: &str = "demo-labs/maydolist-sample";
const DEMO_NOW: &str = "2026-08-12T09:30:00Z";
const DEMO_YESTERDAY: &str = "2026-08-11T16:10:00Z";
const DEMO_LAST_WEEK: &str = "2026-08-05T10:20:00Z";

/// Create a fresh per-process demo directory under the OS temp directory.
pub fn create_storage() -> AppResult<Storage> {
    let dir = std::env::temp_dir().join(format!("maydolist-demo-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    let storage = Storage::with_dir(&dir)?;
    seed(&storage, &dir)?;
    Ok(storage)
}

fn seed(storage: &Storage, dir: &Path) -> AppResult<()> {
    let mut config = AppConfig {
        // Keep the actual storage root isolated on disk, but do not expose
        // the current Windows user name in screenshots or the status bar.
        data_dir: "Demo 数据（临时隔离）".into(),
        hot_corner: "off".into(),
        github_refresh_interval_minutes: 0,
        first_run: false,
        theme: "light".into(),
        ..Default::default()
    };
    config.sanitize();
    storage.save_config(&config)?;

    let inbox = TodoList {
        schema_version: 1,
        id: "00000000-0000-4000-8000-000000000001".into(),
        title: "收件箱".into(),
        kind: Some("inbox".into()),
        sort_order: 0,
        deleted: false,
        created_at: DEMO_LAST_WEEK.into(),
        updated_at: DEMO_NOW.into(),
        items: vec![
            todo(
                "00000000-0000-4000-8000-000000000011",
                "整理本周发布说明",
                0,
                DEMO_NOW,
                None,
                Some("2026-08-12"),
            ),
            todo(
                "00000000-0000-4000-8000-000000000012",
                "跟进 API 错误处理的边界情况",
                1,
                DEMO_YESTERDAY,
                Some(TodoSource {
                    kind: "github-issue".into(),
                    repo: DEMO_REPO.into(),
                    number: 214,
                    url: format!("https://github.com/{DEMO_REPO}/issues/214"),
                }),
                None,
            ),
        ],
    };
    let work = TodoList {
        schema_version: 1,
        id: "00000000-0000-4000-8000-000000000002".into(),
        title: "产品迭代".into(),
        kind: None,
        sort_order: 1,
        deleted: false,
        created_at: DEMO_LAST_WEEK.into(),
        updated_at: DEMO_YESTERDAY.into(),
        items: vec![
            todo(
                "00000000-0000-4000-8000-000000000021",
                "补充快速收集的使用说明",
                0,
                DEMO_YESTERDAY,
                None,
                Some("2026-08-10"),
            ),
            todo(
                "00000000-0000-4000-8000-000000000022",
                "验证 Windows 玻璃效果",
                1,
                DEMO_LAST_WEEK,
                None,
                Some("2026-08-15"),
            ),
        ],
    };
    let personal = TodoList {
        schema_version: 1,
        id: "00000000-0000-4000-8000-000000000003".into(),
        title: "个人计划".into(),
        kind: None,
        sort_order: 2,
        deleted: false,
        created_at: DEMO_LAST_WEEK.into(),
        updated_at: DEMO_YESTERDAY.into(),
        items: vec![todo(
            "00000000-0000-4000-8000-000000000031",
            "为下周安排一个无会议上午",
            0,
            DEMO_YESTERDAY,
            None,
            None,
        )],
    };
    for list in [&inbox, &work, &personal] {
        storage.save_entity("todos", &list.id, list)?;
    }

    let notes = [
        Note {
            schema_version: 1,
            id: "00000000-0000-4000-8000-000000000101".into(),
            title: "本周开发节奏".into(),
            content: "先收集，再判断下一步。把真正需要行动的内容留在今日焦点。".into(),
            tags: vec!["工作流".into(), "重点".into()],
            color: "blue".into(),
            pinned: true,
            floating: false,
            collapsed: false,
            always_on_top: true,
            window_bounds: None,
            deleted: false,
            created_at: DEMO_LAST_WEEK.into(),
            updated_at: DEMO_NOW.into(),
        },
        Note {
            schema_version: 1,
            id: "00000000-0000-4000-8000-000000000102".into(),
            title: "发布前检查".into(),
            content: "截图、构建、备份，最后再发布。".into(),
            tags: vec!["发布".into()],
            color: "purple".into(),
            pinned: false,
            floating: false,
            collapsed: false,
            always_on_top: true,
            window_bounds: None,
            deleted: false,
            created_at: DEMO_YESTERDAY.into(),
            updated_at: DEMO_YESTERDAY.into(),
        },
        Note {
            schema_version: 1,
            id: "00000000-0000-4000-8000-000000000103".into(),
            title: "灵感收集".into(),
            content: "把工具做成一个安静的行动入口。".into(),
            tags: vec!["想法".into()],
            color: "yellow".into(),
            pinned: false,
            floating: false,
            collapsed: false,
            always_on_top: true,
            window_bounds: None,
            deleted: false,
            created_at: DEMO_LAST_WEEK.into(),
            updated_at: DEMO_LAST_WEEK.into(),
        },
    ];
    for note in notes {
        storage.save_entity("notes", &note.id, &note)?;
    }

    let watch = RepoWatch {
        full_name: DEMO_REPO.into(),
        filters: vec!["mine".into(), "mentioned".into(), "all-prs".into()],
        collapsed: false,
        ignored: vec![],
        pinned: vec![128],
        signal_filters: vec![],
    };
    storage.write_json(&dir.join("github/watchlist.json"), &[watch])?;
    let snapshot = RepoSnapshot {
        schema_version: 2,
        repo: DEMO_REPO.into(),
        fetched_at: DEMO_NOW.into(),
        last_success_at: Some(DEMO_NOW.into()),
        last_error: None,
        issues: vec![GhIssue {
            number: 214,
            title: "补充离线缓存的空状态说明".into(),
            state: "open".into(),
            url: format!("https://github.com/{DEMO_REPO}/issues/214"),
            updated_at: DEMO_YESTERDAY.into(),
            kind: "issue".into(),
            matches: vec!["mentioned".into()],
            assignees: vec!["demo-user".into()],
            signals: vec![ActionSignal::NeedsAction],
        }],
        pull_requests: vec![
            GhPullRequest {
                number: 128,
                title: "展示今日焦点的行动信号".into(),
                state: "open".into(),
                draft: false,
                url: format!("https://github.com/{DEMO_REPO}/pull/128"),
                updated_at: DEMO_NOW.into(),
                matches: vec!["pinned".into(), "mine".into()],
                assignees: vec!["demo-user".into()],
                reviewers: vec!["demo-reviewer".into()],
                head_sha: Some("demo128".into()),
                checks_state: Some("failure".into()),
                signals: vec![ActionSignal::NeedsReview, ActionSignal::CiFailed],
            },
            GhPullRequest {
                number: 121,
                title: "优化快速收集窗口的提示文案".into(),
                state: "open".into(),
                draft: true,
                url: format!("https://github.com/{DEMO_REPO}/pull/121"),
                updated_at: DEMO_LAST_WEEK.into(),
                matches: vec!["all-prs".into()],
                assignees: vec![],
                reviewers: vec![],
                head_sha: Some("demo121".into()),
                checks_state: Some("success".into()),
                signals: vec![ActionSignal::Draft],
            },
        ],
        signals_computed_at: Some(DEMO_NOW.into()),
    };
    storage.write_json(
        &dir.join("github/cache/demo-labs_maydolist-sample.json"),
        &snapshot,
    )?;
    Ok(())
}

fn todo(
    id: &str,
    title: &str,
    sort_order: i32,
    updated_at: &str,
    source: Option<TodoSource>,
    due_date: Option<&str>,
) -> TodoItem {
    TodoItem {
        id: id.into(),
        title: title.into(),
        completed: false,
        deleted: false,
        sort_order,
        created_at: DEMO_LAST_WEEK.into(),
        updated_at: updated_at.into(),
        source,
        due_date: due_date.map(str::to_string),
        remind_at: None,
        repeat: None,
        repeat_until: None,
        last_reminded_at: None,
    }
}
