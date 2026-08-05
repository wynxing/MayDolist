# 构建与发布

## 本地环境

- Windows 10/11 与 WebView2 Runtime
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

标签格式错误或任一版本不一致时，工作流会失败且不会创建 Release。

## 代码签名说明

当前自动构建不对 Windows 安装器和便携版执行代码签名，因此用户下载或首次运行时可能看到 Microsoft Defender SmartScreen 提示。后续接入可信代码签名证书时，应将证书和密码存储为 GitHub Actions secrets，不要提交到仓库。

应用不会打包或读取 GitHub token；GitHub 登录统一使用 `gh auth login`。
