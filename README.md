# 轻压 · QZip

QZip 是一款开源、跨平台、轻量化的桌面压缩工具。当前仓库处于 RC1 实施阶段：Windows x64 的安全写入、安装包和发布验证正在收敛；macOS/Linux 与部分高级功能计划在 RC2 完成。

开发环境：Node.js 24、pnpm 11.9、Rust 1.96（MSVC 工具链）。

开始开发：pnpm install，然后执行 pnpm dev。

仅启动前端：pnpm dev:web。

质量检查：pnpm check。

M1 的 Windows x64 7-Zip Sidecar 可执行 `pnpm sidecar:fetch` 拉取和校验；使用 `pnpm sidecar:test` 验证 7Z 创建、列表、测试、解压及 Unicode 路径。开发者验证入口为 `cargo run -p qzip-cli -- capabilities`。

Windows 打包需要先拉取 Sidecar，再执行 `pnpm tauri:bundle:windows`；资源映射仅在该打包配置中启用，因此普通开发/CI 构建不需要下载二进制。`pnpm sidecar:verify` 会校验 7-Zip 可执行文件、动态库和源码归档的 SHA-256。

发布相关说明见 [隐私说明](PRIVACY.md)、[卸载说明](UNINSTALL.md)、[已知问题](KNOWN_ISSUES.md) 与 [变更记录](CHANGELOG.md)。RC 标签由 GitHub Actions 构建；没有可信签名凭据时，工作流会失败且不会创建公开 Release。

产品需求见 [QZip_PRD_Requirements_V2.1.md](QZip_PRD_Requirements_V2.1.md)，实施清单见 [QZip_TODO_V2.1.md](QZip_TODO_V2.1.md)。原始开发基线保留在 `QZip_PRD_V2.1.md`。
