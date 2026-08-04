# 构建与发布

## 环境

- Windows 10/11 与 WebView2 Runtime
- Node.js 22+、pnpm 11+
- Rust stable MSVC toolchain，包含 `rustfmt` 与 `clippy`
- 可选：GitHub CLI `gh`（运行 GitHub 追踪功能需要）

## 开发与检查

```powershell
pnpm install
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri dev
```

## 发布

```powershell
pnpm release
Copy-Item src-tauri\target\release\maydolist.exe `
  src-tauri\target\release\MayDolist-portable-1.0.0.exe
```

NSIS 安装器位于 `src-tauri/target/release/bundle/nsis/`。便携版使用相同 release
可执行文件；移动便携版后，如已启用开机自启，需要在设置中重新关闭并启用。

应用不会打包或读取 GitHub token；GitHub 登录统一使用 `gh auth login`。
