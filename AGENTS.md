# MayDolist

Windows 本地收件箱。Tauri 2 + Rust + Vue 3。前端不碰磁盘；GitHub 只走本机 `gh`。
地图：[docs/architecture.md](docs/architecture.md)。

## 分层

UI → `src/api` `call()` → Command（校验）→ Service（规则）→ 原子写盘 → `entity-changed`

Command 不写业务。改功能只动 architecture 代码地图里的文件。

## 准则

- 替换旧实现，不留兼容：无 serde alias、双 API、`schemaVersion` 分叉、`legacy*`。升 schema，改调用点，删旧路径。现有 sanitize / 缺省补齐不要再扩大。
- 不防御式编程：不吞错误、不用默认值掩盖坏数据、不多层重复校验。非法输入走 `AppError` / `ApiError`。
- 不过度设计：单进程单写者。无复现的边角不修、不加防护。不上 Factory / Strategy / Observer 去包直调。
- 不平行发明：用现有 `call()`、`entity-changed`、store、component、`gh`。不新开封装或目录。
- 不过度包装：能直调就直调。不为 1–2 处调用抽 helper / facade / 基类；别把参数收成只传一次的 config 对象。
- 不超范围：只改任务所需的文件和行。不顺手重构、不整文件重写、不扩需求。
- 不留废料：不注释保留旧代码，不留 TODO / stub / 未用导出 / 调试日志。
- 不加依赖：标准库或现有代码能做的，不引入新包。
- 不臆造：先读 architecture 代码地图和现行实现，再用已有 API。禁止编造函数、类型、文件去顶替已有的。
- 不写噪音：不写复述代码的注释；未经要求不改 README / CHANGELOG / 无关文档。

## 不做

云同步、多端、应用内 GitHub 登录/存 token、换数据库、GitHub 写操作。
Focus / Palette / triage 不自建持久化。

## 命令

改完跑相关检查，不要只改不跑。

```
pnpm check && pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
pnpm gen:types   # 改 Rust 模型后，提交 src/types/generated/
```
