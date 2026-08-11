# Issue 执行进度跟踪

> 每个 issue 一轮独立对话。新对话开场先读本文件 + 目标 issue 全文 + 最新 `main`。
> 完成标准：自动化校验全绿（`pnpm check` / `pnpm build` / `cargo fmt` / `cargo clippy` / `cargo test`）、PR 合并、issue 自动关闭。

> 编排模式：由「巡检控制器」每小时兜底巡检 + 每轮线程即时串联（用 create_thread 创建下一轮）。tracker 的 activeThreadId 由创建者写入、完成者清除；控制器据此去重。

## 总体安排

| Issue | 标题 | 依赖 | 状态 | 分支 | PR | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| #16 | RFC：演进为开发者行动收件箱 | 全部实现 | 待做 | - | - | 最后收尾 |
| #17 | 快速收集入口与 Inbox | 无 | ✅ 已完成 | `codex/issue-17-quick-capture` | [#22](https://github.com/wynxing/MayDolist/pull/22) | CI 绿 + 已合并 |
| #18 | Focus 统一视图 | #17 | ✅ 已完成 | `codex/issue-18-focus-view` | [#23](https://github.com/wynxing/MayDolist/pull/23) | CI 绿 + 已合并 + issue 自动关闭 |
| #19 | GitHub 条目转 Todo | #17/#18 | 进行中 | - | - | activeThreadId: 创建中（本轮回写） |
| #20 | GitHub 可行动信号 | #18/#19 | 待做 | - | - | |
| #21 | 备份 / 导入 / 恢复 | 无 | 待做 | - | - | |

## 验收清单（#17 快速收集入口与 Inbox）

- [x] 配置模型新增快速收集快捷键字段、默认值（`Ctrl+Alt+Space`）与旧配置迁移（serde 默认值）
- [x] Rust 层注册 / 更新快速收集快捷键；快速收集窗口启动时创建（默认隐藏）并复用，重新打开自动聚焦
- [x] 前端 QuickCapture 视图：Enter 提交（组合输入不误提交）、Esc 关闭、失败保留输入、清空输入后隐藏
- [x] `TodoService::ensure_inbox` 幂等：`kind=inbox` 稳定标记 → 采用同名旧列表 → 才创建
- [x] `quick_capture_submit` 统一命令（`todo:` / `note:` / 默认 todo），解析与错误路径有单测
- [x] 托盘菜单新增「快速收集」入口
- [x] 文档更新（architecture.md / README.md）
- [x] 单元测试：config 迁移、前缀解析、ensure_inbox、kind 序列化
- [x] 全量校验全绿（pnpm check/build、cargo fmt/clippy/test 34 passed）
- [x] PR [#22](https://github.com/wynxing/MayDolist/pull/22) 合并（merge commit `ddfd778`），issue #17 已自动关闭

## 验收清单（#18 Focus 统一视图）

- [x] 主面板新增「今日」(Focus) Tab，并作为默认打开页（托盘 / 快捷键跳转行为不变）
- [x] 只读投影：`FocusService` + `focus_overview` 命令，并行加载 Todo / Note / GitHub，任一领域失败只产生局部错误，不阻塞其他区块
- [x] 聚合规则：未完成 Todo「收件箱」优先；便签置顶 + 最近更新（id 去重）；GitHub 只取本地快照 open 条目、手动钉住优先；上限 Todo 50 / 便签 8 / GitHub 30
- [x] 前端 FocusView + focus store：完成 Todo、打开来源、进入对应模块（便签携带 id 打开编辑 + 保留悬浮）等最小动作
- [x] 加载 / 空 / 局部失败 / 离线缓存状态齐全；`entity-changed` 防抖刷新，多窗口一致
- [x] 窄窗响应式布局与键盘焦点顺序
- [x] 不改变领域文件格式（无新增持久化字段；新增字段仅属于 Focus 投影结构，向后兼容）
- [x] 文档更新（README / architecture.md，说明排序与纳入规则）
- [x] 单元测试：聚合排序、去重、来源映射、离线缓存标记、局部失败隔离、真实数据聚合（cargo test 41 passed）
- [x] 全量校验全绿（pnpm check/build、cargo fmt/clippy -D warnings、cargo test 41 passed）
- [x] PR [#23](https://github.com/wynxing/MayDolist/pull/23) 合并（merge commit `8259291`），issue #18 已自动关闭

## 交接记录

### 自动化编排启动（2026-08-11）

- 巡检控制器：`MayDolist Issue 编排巡检`（automationId `maydolist-issue`，cron 每小时，failed_runs_only）。
- #18 线程已由控制器编排创建（clientThreadId `1dfe29ce-973d-440e-846d-cf09f851bfa3`，worktree 环境），完成校验合并后将即时创建 #19。
- 失败策略：当轮自动修复重跑直到全绿；连续 3 次同因失败则暂停并通知，不跳过。

### 轮次 1（#17，已完成）

- 工作区注意：恢复源码后 `git add -- src src-tauri public` 刷新了 stat 缓存（内容无变化）；未跟踪文件 `liquid-glass-*.png` 保留不动，不入提交。
- 完成证据：PR #22（CI 2m53s 全绿）→ squash 合并 → issue #17 自动关闭。
- 下一轮：#18 Focus 统一视图（依赖 #17），开场指令见上一轮对话结尾。

### 轮次 2（#18，已完成）

- 完成证据：PR #23（CI 3m22s 全绿）→ squash 合并（merge commit `8259291`）→ issue #18 自动关闭。
- 下一轮：#19 GitHub 条目转 Todo（依赖 #17/#18），线程已即时创建（threadId 见 #19 行备注），开场指令见本轮交接摘要。
