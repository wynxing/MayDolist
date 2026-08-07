# 构建与发布

## 本地环境

- Windows 11（玻璃效果与视觉基线仅针对 Windows 11 + WebView2 校准，不维护旧系统降级路径）
- WebView2 Runtime
- Node.js 22+、pnpm 11+
- Rust stable MSVC toolchain，包含 `rustfmt` 与 `clippy`
- 可选：GitHub CLI `gh`（运行 GitHub 追踪功能需要）

安装依赖并运行开发环境：

```powershell
pnpm install
pnpm tauri dev
```

## 本地质量检查

提交代码前应执行与 CI 相同的检查：

```powershell
pnpm install --frozen-lockfile
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

面向 `main` 的 Pull Request 和推送到 `main` 的提交会在 Windows runner 上自动执行这些检查。建议在 GitHub 分支保护中将 `Frontend and Rust checks` 设为合并前必需检查。

## main 自动构建

`main` 分支的 CI 成功后，`Build main` 工作流会构建：

- NSIS 安装器
- 以短提交 SHA 命名的便携版 EXE

两个文件会合并上传到名为 `MayDolist-windows-<short-sha>` 的 Actions artifact，并保留 14 天。可在仓库的 **Actions → Build main → 对应运行 → Artifacts** 中下载。

本地构建相同产物可运行：

```powershell
pnpm tauri build --bundles nsis
```

NSIS 安装器位于 `src-tauri/target/release/bundle/nsis/`，release 可执行文件位于 `src-tauri/target/release/maydolist.exe`。

## 正式发布

正式版本使用 `v<major>.<minor>.<patch>` 标签，例如 `v1.2.3`。发布前必须把以下三个文件中的版本同步为去掉 `v` 的版本号：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

提交版本变更后创建并推送标签：

```powershell
git tag v1.2.3
git push origin v1.2.3
```

`Release` 工作流会验证标签格式及三处版本，重新运行完整质量检查，构建 Windows 包，并创建带自动发行说明的正式 GitHub Release。每个 Release 固定包含：

- `MayDolist-setup-<version>-x64.exe`
- `MayDolist-portable-<version>.exe`
- `MayDolist-setup-<version>-x64.exe.sig`（应用内更新签名）
- `latest.json`（Tauri updater 元数据）

发布前需要在仓库 Actions Secrets 中配置：

- `TAURI_SIGNING_PRIVATE_KEY`：与应用内 updater 公钥配对的私钥全文。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：私钥密码；无密码密钥可留空。

私钥不得提交到仓库。若私钥丢失，已发布客户端将无法验证使用新密钥签名的更新。

标签格式错误或任一版本不一致时，工作流会失败且不会创建 Release。

## 代码签名说明

当前自动构建不对 Windows 安装器和便携版执行代码签名，因此用户下载或首次运行时可能看到 Microsoft Defender SmartScreen 提示。后续接入可信代码签名证书时，应将证书和密码存储为 GitHub Actions secrets，不要提交到仓库。

应用不会打包或读取 GitHub token；GitHub 登录统一使用 `gh auth login`。

## 玻璃透明度与配置升级

设置页提供「玻璃透明度」主面板与悬浮便签两个滑块，范围 40%–100%，对应配置键：

- `mainWindowGlassOpacity`（主面板）
- `floatingNoteGlassOpacity`（悬浮便签）

该配置采用兼容 schema 升级：旧版 `config.json` 会保留已有主题、快捷键、热角和数据目录，自动补齐玻璃透明度并升级 `schemaVersion`。只有 JSON 损坏或结构不可恢复时才会隔离备份并按默认值重建。视觉基线仅针对当前 Windows 11 + WebView2 环境，不维护旧系统或缺失玻璃能力环境的降级样式。
