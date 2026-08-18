# Changelog

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

- Inbox triage, command palette, due dates / reminders, and GitHub action signals.
