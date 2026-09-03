# Changelog

## 1.3.3

- Collapse the notes empty state into a single header CTA, and auto-select a note when any exist.

## 1.3.2

- Show delete, import, and empty-trash confirm bars next to the action instead of at the top of the page.

## 1.3.1

- Make inbox triage "later" days configurable (1–30, default 3).
- Quick capture: `/note 标题` creates a titled note; date prefixes include next weekdays (`下周五`).
- Extract Todo list/row/schedule/triage into reusable components.
- Generate TypeScript types from Rust models (ts-rs) and fail CI when they drift.
- Add Prettier checks; split the Rust app and GitHub service into modules.

## 1.3.0

- Sync GitHub PR / Issue source states into linked Todo items.
- Automatically complete linked Todos when a source closes or a PR merges.
- Preserve local decisions when a source reopens or an automatic completion is manually undone.
- Add GitHub source status badges, refresh summaries, settings controls, and linked-source sync commands.

## 1.2.3

- Unify page headers, empty states, and pin marks; reduce nested glass blur.
- Align the notes view with Todo (header, keyboard list, delete confirm, color dots).
- Add short tab / triage / complete motion and in-app confirm bars.
- Wire tray “新建便签” / “刷新 GitHub”; keep the GitHub store in sync via `entity-changed`.
- Collapse GitHub filters by default and add an empty watchlist CTA.
- Persist reminder delivery (`lastRemindedAt`), cache Todo JSON reads, and dedupe GitHub→Todo by repo + number.
- Show a one-time first-run hint using the existing `firstRun` flag.

## 1.2.2

- Inbox triage 逐条处理模式：`1`-`5` 快捷键（今天做 / 稍后做 / 转列表 / 完成 / 删除）将收件箱归零。
- 全局命令面板（Ctrl+K）：切换视图、新建 Todo / 便签、立即备份、打开数据目录，并即搜即得 Todo / 便签 / GitHub 三个域。
- Todo 支持到期日、提醒时间与周期规则，配本地 Windows 通知与托盘徽标。
- GitHub 追踪行动信号：需要处理、需要 Review、CI 失败、长期未更新、Draft 等。
