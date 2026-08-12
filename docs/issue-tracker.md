# Issue 执行进度跟踪

> 每个 issue 一轮独立对话。新对话开场先读本文件 + 目标 issue 全文 + 最新 `main`。
> 完成标准：自动化校验全绿（`pnpm check` / `pnpm build` / `cargo fmt` / `cargo clippy` / `cargo test`）、PR 合并、issue 自动关闭。

> 编排模式：由「巡检控制器」每小时兜底巡检 + 每轮线程即时串联（用 create_thread 创建下一轮）。tracker 的 activeThreadId 由创建者写入、完成者清除；控制器据此去重。

## 总体安排

| Issue | 标题 | 依赖 | 状态 | 分支 | PR | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| #16 | RFC：演进为开发者行动收件箱 | 全部实现 | ✅ 已完成 | `codex/issue-16-rfc-action-inbox` | [#27](https://github.com/wynxing/MayDolist/pull/27) | CI 绿 + 已合并 + issue 自动关闭 |
| #17 | 快速收集入口与 Inbox | 无 | ✅ 已完成 | `codex/issue-17-quick-capture` | [#22](https://github.com/wynxing/MayDolist/pull/22) | CI 绿 + 已合并 |
| #18 | Focus 统一视图 | #17 | ✅ 已完成 | `codex/issue-18-focus-view` | [#23](https://github.com/wynxing/MayDolist/pull/23) | CI 绿 + 已合并 + issue 自动关闭 |
| #19 | GitHub 条目转 Todo | #17/#18 | ✅ 已完成 | `codex/issue-19-github-todo` | [#24](https://github.com/wynxing/MayDolist/pull/24) | CI 绿 + 已合并 + issue 自动关闭 |
| #20 | GitHub 可行动信号 | #18/#19 | ✅ 已完成 | `codex/issue-20-action-signals` | [#25](https://github.com/wynxing/MayDolist/pull/25) | CI 绿 + 已合并 + issue 自动关闭 |
| #21 | 备份 / 导入 / 恢复 | 无 | ✅ 已完成 | `codex/issue-21-backup-restore` | [#26](https://github.com/wynxing/MayDolist/pull/26) | CI 绿 + 已合并 + issue 自动关闭 |
| #29 | Todo 支持到期日、提醒与周期性任务 | 无 | ✅ 已完成 | `codex/issue-29-due-reminder` | [#31](https://github.com/wynxing/MayDolist/pull/31) | CI 全绿 + 已合并（merge commit `c23d994`）+ issue 自动关闭 |
| #30 | 全局命令面板（Ctrl+K） | 无 | ⬜ 待排期 | `codex/issue-30-command-palette` | 待创建 | 顺序 2：独立 |
| #28 | Inbox 逐条处理模式（triage） | #17 Inbox | ⬜ 待排期 | `codex/issue-28-inbox-triage` | 待创建 | 顺序 3：依赖 #29 |

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

## 验收清单（#19 GitHub 条目转 Todo）

- [x] Rust Todo 模型与 TypeScript 类型新增向后兼容 `TodoSource`（JSON 字段 `type` / `repo` / `number` / `url`；无来源条目不落盘该字段）
- [x] 旧数据迁移：source 可选 + serde 默认值，旧 Todo 读取为无来源
- [x] `todo_create_item` command / service / API 支持可选 source；新增 `todo_create_from_github` 命令（默认进入 Inbox，复用 `ensure_inbox` 幂等逻辑，默认标题「仓库 #编号 标题」）
- [x] GitHub 视图 PR / Issue 行新增「转为 Todo」：保存中 / 成功 / 失败反馈，防快速重复点击；同一条目允许重复转换
- [x] Todo 视图与 Focus 视图显示来源徽标并提供「打开来源」（仅 http / https，前后端双重校验）
- [x] 不改变 GitHub 缓存、认证与网络状态；前端不直接写文件（来源经 Rust command / service 持久化）
- [x] 单元测试：source 序列化、旧数据迁移、URL 校验、GitHub→Todo service 层、Focus 投影（cargo test 48 passed）
- [x] 全量校验全绿（pnpm check/build、cargo fmt/clippy -D warnings、cargo test 48 passed）
- [x] 文档更新（architecture.md 数据布局与跨模块引用 + README.md）
- [x] PR [#24](https://github.com/wynxing/MayDolist/pull/24) 合并（merge commit `f3d4ff2`），issue #19 已自动关闭

## 验收清单（#20 GitHub 可行动信号）

- [x] 扩展 GitHub 内部 API response model：assignee、requested reviewers（PR 详情）、draft、checks（status + check-runs 兜底）、更新时间
- [x] 稳定枚举 `ActionSignal`（needsAction / needsReview / ciFailed / stale / draft），UI 不依赖 GitHub 原始字符串
- [x] `RepoSnapshot` 向后兼容信号字段（条目级 signals 等 + 快照级 `signalsComputedAt`），旧缓存读取默认空值、刷新自动补全
- [x] 按行动信号过滤入口（默认空 = 保持旧行为）+ 单仓库刷新；`githubStaleDays` 配置（默认 14 天，0 关闭），stale 读取时实时计算
- [x] GitHub 视图与 Focus 视图展示信号徽标、最后更新时间与本地缓存状态（旧缓存提示）
- [x] 刷新失败 / 认证失败 / 网络失败 / 缺字段降级：保留旧缓存、不清空其他仓库；全部刷新按仓库串行 + 同仓库并发去重；未变化 PR 复用缓存详情（rate-limit 友好）
- [x] 文档更新：architecture.md（数据布局 / 缓存策略 / 权限说明）+ README.md
- [x] 单元测试：response fixture→signal 映射、新旧 schema 兼容、过滤器 / 排序 / stale 边界、真实数据聚合（cargo test 62 passed）
- [x] 前端不直接写文件；GitHub API 调用在 Rust service 中执行，阻塞调用走 spawn_blocking
- [x] 全量校验全绿（pnpm check/build、cargo fmt/clippy -D warnings、cargo test 62 passed）
- [x] PR [#25](https://github.com/wynxing/MayDolist/pull/25) 合并（merge commit `9d127fa`），issue #20 已自动关闭

## 验收清单（#21 备份 / 导入 / 恢复）

- [x] 定义导出包格式、版本字段和目录布局，并记录在架构文档中（§5.5 + README「数据备份与恢复」）
- [x] 增加 Rust export / import / backup service，复用现有原子写和损坏隔离能力（`storage::replace_domain` 持锁原子交换 + 失败逐项回滚）
- [x] 增加导入包校验、路径安全校验和版本兼容策略（manifest 版本拒绝、白名单布局、路径穿越 / 重复 / 非法 JSON 拒绝）
- [x] 按需接入 Windows 原生文件选择 / 保存对话框，并补充最小 capability（tauri-plugin-dialog + `dialog:default`）
- [x] 在 SettingsView 增加导出、导入、备份、打开数据目录和最近备份信息（「数据安全」区块 + 成功 / 失败反馈）
- [x] 导入前提供覆盖提示；导入失败时保持原数据和原配置可用（预览确认 → 自动备份 → 原子替换 / 回滚）
- [x] 记录关键操作日志，但日志中不得写入 GitHub token 或完整私密路径以外的敏感内容（只记录路径与计数）
- [x] 更新 README、架构文档和恢复操作说明
- [x] 测试与验证：导出 / 导入完整往返、备份轮转保留 10 份、空包导入、路径穿越 / 重复条目 / 非法 JSON / 版本过高拒绝、损坏 cache 降级跳过、原子替换回滚（cargo test 76 passed）
- [x] 全量校验全绿（pnpm check/build、cargo fmt/clippy -D warnings、cargo test 76 passed）
- [x] PR [#26](https://github.com/wynxing/MayDolist/pull/26) 合并（merge commit `c614c9e`），issue #21 已自动关闭

## 验收清单（#16 RFC：演进为开发者行动收件箱）

### RFC 分段清单（5 项全部实现并合并）

- [x] 快速收集与 Inbox —— #17 → PR [#22](https://github.com/wynxing/MayDolist/pull/22)（merge `ddfd778`）
- [x] 今日 / Focus 统一视图 —— #18 → PR [#23](https://github.com/wynxing/MayDolist/pull/23)（merge `8259291`）
- [x] GitHub 条目转 Todo 与来源关联 —— #19 → PR [#24](https://github.com/wynxing/MayDolist/pull/24)（merge `f3d4ff2`）
- [x] GitHub 可行动信号 —— #20 → PR [#25](https://github.com/wynxing/MayDolist/pull/25)（merge `9d127fa`）
- [x] 本地备份、导入与恢复 —— #21 → PR [#26](https://github.com/wynxing/MayDolist/pull/26)（merge `c614c9e`）

### RFC 总体验收标准（6 条逐项核对）

- [x] 用户可以在不打开主面板的情况下快速创建 Todo 或便签 —— 快速收集窗（`Ctrl+Alt+Space` / 托盘「快速收集」），`todo:` / `note:` 前缀，`quick_capture_submit` 单测覆盖
- [x] 用户打开主面板后能看到按行动优先级聚合的内容 —— Focus / 今日默认页聚合收件箱优先 Todo、置顶 / 最近便签、可行动 GitHub 条目（`FocusService::overview` 并行加载 + 局部失败隔离）
- [x] GitHub PR / Issue 可以转成带来源的 Todo，并能返回 GitHub 页面 —— `todo_create_from_github` + `TodoSource`（type / repo / number / url）+「打开来源」浏览器跳转
- [x] GitHub 刷新结果可以表达「需要我行动」的状态 —— 稳定 `ActionSignal` 枚举（needsAction / needsReview / ciFailed / stale / draft）+ 徽标 + 按信号过滤
- [x] 本地数据可以导出、导入，并在异常情况下恢复 —— export / import / backup service：staging 校验 → 自动备份 → 原子交换 / 回滚
- [x] 现有 Todo、便签、GitHub 追踪、悬浮窗口、回收站和更新能力不回归 —— 领域格式与字段向后兼容（serde 默认值），全量校验全绿

### 回归与校验

- 本轮仅文档收尾：完善 architecture.md（产品定位 / 设计原则 / 模块关系 / 行动闭环 / 非目标）与 README.md，未改动任何源码、模型或持久化字段，已实现模块行为不变。
- 全量校验全绿：`pnpm check` ✅、`pnpm build` ✅、`cargo fmt --check` ✅、`cargo clippy --all-targets -D warnings` ✅、`cargo test`（76 passed）✅；PR #27 CI 3m7s 全绿（rebase 前 2m24s 亦全绿）。
- PR [#27](https://github.com/wynxing/MayDolist/pull/27) squash 合并（merge commit `39ecbb1`），issue #16 已自动关闭。

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
- 下一轮：#19 GitHub 条目转 Todo（依赖 #17/#18），线程已即时创建（threadId `019ff151-a86c-70b0-9ce6-96f546f7ab84`，worktree `C:\Users\wynn\.codex\worktrees\2e60\MayDolist`），开场指令见本轮交接摘要。

### 轮次 3（#19，已完成）

- 完成证据：PR #24（CI 3m43s + rebase 后 2m48s 全绿）→ squash 合并（merge commit `f3d4ff2`）→ issue #19 自动关闭。
- 实现要点：Todo 可选 `source`（`type`/`repo`/`number`/`url`，serde 默认值向后兼容）；`todo_create_from_github` 命令默认进 Inbox（复用 `ensure_inbox`）；GitHub 行「转为 Todo」带保存中/成功/失败反馈；Todo 与 Focus 视图来源徽标 + 「打开来源」（仅 http/https）；不触碰 GitHub 缓存/认证/网络；前端不直接写文件。cargo test 48 passed。
- 下一轮：#20 GitHub 可行动信号（依赖 #18/#19），线程已即时创建（threadId `019ff170-2ab6-7201-a181-e2148d8cd8a2`，worktree `C:\Users\wynn\.codex\worktrees\3bc6\MayDolist`），开场指令见本轮交接摘要。

### 轮次 4（#20，已完成）

- 完成证据：PR #25（CI 2m56s 全绿）→ squash 合并（merge commit `9d127fa`）→ issue #20 自动关闭。
- 实现要点：稳定 `ActionSignal` 枚举与徽标（需要我处理 / 需要 Review / CI 失败 / 长期未更新 / Draft）；PR 详情 + checks 枚举（未变化 PR 复用缓存详情）；RepoSnapshot 信号字段全向后兼容；行动信号过滤（默认关闭保持旧列表）；全部刷新串行 + 同仓库并发去重 + 单仓库失败不清空其他仓库；Focus 只聚合带可行动信号的 open 条目；`githubStaleDays` 可配置（默认 14 天）。cargo test 62 passed。
- 下一轮：#21 备份 / 导入 / 恢复，线程已即时创建（threadId `019ff1ad-8212-7380-b82b-6855de03857f`，worktree `C:\Users\wynn\.codex\worktrees\1fe5\MayDolist`），开场指令见本轮交接摘要。

### 轮次 5（#21，已完成）

- 完成证据：PR #26（CI 3m56s 全绿）→ squash 合并（merge commit `c614c9e`）→ issue #21 自动关闭。
- 实现要点：ZIP 数据包（`manifest.json` `packageSchemaVersion=1` + config / notes / todos / watchlist + 可选 cache，不含 logs / backups / 凭据）；导入「预览确认 → 自动备份当前数据 → 同卷 staging 校验 → 持锁原子交换（失败逐项回滚）→ 广播 settings-changed + entity-changed」；路径安全 / 版本 / 重复 / 非法 JSON 拒绝，损坏 cache 跳过降级；设置页「数据安全」（导出 / 导入 / 备份 / 打开数据目录 / 最近备份，轮转保留 10 份）；tauri-plugin-dialog + `dialog:default`；日志不写文件内容与凭据。cargo test 76 passed。
- 下一轮：#16 RFC：演进为开发者行动收件箱（全部实现已就绪，最后收尾项），线程待创建者即时创建并回写 activeThreadId。

### 轮次 6（#16，创建中，2026-08-12 并发去重）

- 同一时间窗口内有多个控制器 run 并发唤醒：15:22 run（019ff1a2）先创建 #16 线程 `019ff329-5061-7e60-bb6b-4fe744ebc3c9`（worktree `7e01`，分支 `codex/issue-16-rfc-action-inbox`）并回写 activeThreadId（提交 `9900b20`）；本 run 随后创建的重名线程 `019ff329-fb8f-76c1-8635-bd73172c7487`（worktree `1641`，分支 `codex/issue-16-rfc-inbox`）已按 tracker 去重规则通知停止，未推送任何内容。
- 进行中：#16 由 `019ff329-5061` 线程执行，完成后全部 6 个 issue 收官（#17 已完成 + #18-#21 及 #16 均 ✅），自动化输出完成总结并暂停。

### 轮次 6（#16，已完成）

- 完成证据：PR [#27](https://github.com/wynxing/MayDolist/pull/27)（CI 3m7s 全绿，rebase 后重跑亦全绿）→ squash 合并（merge commit `39ecbb1`）→ issue #16 自动关闭（2026-08-12 07:50 +08）。
- 收尾内容：RFC 分段清单 5 项逐项核对 ✅（#17-#21 → PR #22-#26）；总体验收标准 6 条逐条核对 ✅；回归确认（Todo / 便签 / GitHub 追踪 / 悬浮窗口 / 回收站 / 更新能力）无回归；完善 architecture.md 与 README.md（产品定位、模块关系、闭环流程「捕获 → 判断下一步 → 执行 → 回到来源」、非目标）；无新增持久化字段、无破坏性变更。
- 全量校验全绿：pnpm check / pnpm build / cargo fmt --check / cargo clippy --all-targets -D warnings / cargo test（76 passed）。
- **全量编排完成**：6 个 issue（#16-#21）全部 ✅，无下一轮 issue；巡检控制器不再创建新轮次，仅保留兜底巡检。

## 编排启动（#29 → #30 → #28，2026-08-12）

> 由产品对话建议产生（RFC 提案风格），用户确认开启全自动编排；执行顺序 #29 → #30 → #28。
> 模式：复用巡检控制器 `maydolist-issue`（每小时兜底巡检）+ 每轮线程即时串联；本地五道门禁 + CI 绿后自动 squash 合并并关闭 issue。

| Issue | 标题 | 依赖 | 状态 |
| --- | --- | --- | --- |
| [#29](https://github.com/wynxing/MayDolist/issues/29) | feat: Todo 支持到期日、提醒与周期性任务 | 无 | ✅ 已完成 |
| [#30](https://github.com/wynxing/MayDolist/issues/30) | feat: 全局命令面板（Ctrl+K） | 无 | ⬜ 待排期 |
| [#28](https://github.com/wynxing/MayDolist/issues/28) | feat: Inbox 逐条处理模式（triage） | #17 Inbox | ⬜ 待排期 |

- 决策：顺序 #29→#30→#28（先到期日地基 → 命令面板独立 → triage 依赖到期日）；自动 squash 合并、不开人工评审；不 bump 版本 / 不发版。
- 创建方式：`gh issue create --body-file`，正文遵循“目标问题 → 实现设计 → 任务清单 → 测试与验证 → 风险与回滚 → 非目标”。
- #28 与 #29 设计上松耦合：#28 的“今天做 / 稍后做”在 #29 未实现时降级为移动列表，不阻塞；#28 将消费 #29 的 dueDate。
- 失败策略：某轮同因失败连续 3 次 → 暂停自动化并通知用户，绝不跳过 issue；并发去重：创建线程前校验 activeThreadId。

### 轮次 1（#29，已完成，2026-08-12）

- #29 执行线程已创建并完成：threadId `019ff487-db47-7050-99ee-7a1da3a3b50c`，worktree `38d3`，分支 `codex/issue-29-due-reminder`；activeThreadId 已清除。
- 完成证据：PR [#31](https://github.com/wynxing/MayDolist/pull/31)（CI 全绿，首次 5m27s，rebase 后 3m9s）→ squash 合并（merge commit `c23d994`）→ issue #29 自动关闭（2026-08-12 14:32 +08）。
- 实现要点：TodoItem 新增可选 `dueDate` / `remindAt` / `repeat` / `repeatUntil`（serde 默认值向后兼容，落盘跳过 None）；Focus 按「已逾期 > 今天到期 > 近期 7 天 > 无日期」分组并输出分组元信息，逾期高亮 + 数量标识；后台调度到期提醒（Windows Toast，点击聚焦条目），托盘红色逾期数字徽标（0 隐藏）；完成周期任务时 service 层原子生成下一次实例（月末钳制，repeatUntil 停止，保留来源）；快速捕捉日期前缀解析（明天 / 周X / N天后 / 月底）；Todo 编辑 UI 设置/清除到期日与周期规则；配置新增可选安静时段 `quietHours`（跨午夜）；前端不直接写文件；cargo test 115 passed。
- 下一轮：#30 全局命令面板（Ctrl+K），线程已即时创建（threadId `019ff4b2-45f8-7783-af30-3afbb3037cd1`，worktree `09da`，分支 `codex/issue-30-command-palette`），开场指令见本轮交接摘要。
