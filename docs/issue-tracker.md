# Issue 执行进度跟踪

> 每个 issue 一轮独立对话。新对话开场先读本文件 + 目标 issue 全文 + 最新 `main`。
> 完成标准：自动化校验全绿（`pnpm check` / `pnpm build` / `cargo fmt` / `cargo clippy` / `cargo test`）、PR 合并、issue 自动关闭。

## 总体安排

| Issue | 标题 | 依赖 | 状态 | 分支 | PR | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| #16 | RFC：演进为开发者行动收件箱 | 全部实现 | 待做 | - | - | 最后收尾 |
| #17 | 快速收集入口与 Inbox | 无 | 进行中 | `codex/issue-17-quick-capture` | - | 本轮 |
| #18 | Focus 统一视图 | #17 | 待做 | - | - | 下一轮 |
| #19 | GitHub 条目转 Todo | #17/#18 | 待做 | - | - | |
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
- [ ] 全量校验 + PR 合并 + issue #17 关闭（待本轮完成）

## 交接记录

### 轮次 1（#17，进行中）

- 工作区注意：恢复源码后 `git add -- src src-tauri public` 刷新了 stat 缓存（内容无变化）；未跟踪文件 `liquid-glass-*.png` 保留不动，不入提交。
- 下一个 issue：#18 Focus 统一视图（依赖 #17）。
