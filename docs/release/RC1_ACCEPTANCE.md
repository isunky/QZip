# RC1 验收记录

发布 RC1 前必须记录以下证据：

1. `pnpm check`、`cargo test --workspace --no-fail-fast`、`pnpm sidecar:verify`、`pnpm sidecar:test` 和 Windows 打包均通过。
2. 在干净 Windows 11 x64 账户完成 NSIS 安装、启动、升级和卸载；文件关联、双击、右键菜单、DPI 100%/150%/200% 均有截图或录屏。
3. 创建、解压、取消、错误密码、磁盘空间不足、文件占用和恶意路径样本均有结果记录。
4. 性能基线记录 RC1 的实际机器、release 可执行文件 SHA-256、逐次启动/内存数据、十万项大包列表、进度突发和异常恢复结果。第三方压缩软件交叉兼容性不属于 RC1 发布门禁，不得对外宣称兼容认证。
5. Release 资产包含受信任 Authenticode 签名、SHA-256、第三方许可、GitHub build provenance 和本文件的已签字版本。SBOM 仍是 RC2/后续发布项，不得在 RC1 中声称已提供。

## 发布工作流所需配置

标签 `v1.0.0-rc.1` 只会触发候选构建。公开 prerelease 还要求 GitHub
Actions 机密 `WINDOWS_PFX_BASE64`、`WINDOWS_PFX_PASSWORD` 与仓库变量
`QZIP_WINDOWS_PUBLISHER`。任一缺失、签名不可信或发布物校验失败都会
终止工作流，且不会创建 GitHub Release。

Windows MSI 的 `ProductVersion` 不接受 `rc.1` 这类非数字预发布段，因而
本 RC 使用 `1.0.0` 作为安装包内部产品版本；Git 标签、资产名和 GitHub
prerelease 标题仍使用 `v1.0.0-rc.1`。该映射由发布脚本强制校验。

未完成项必须进入 `KNOWN_ISSUES.md`，且不得声称通过 V1.0 总验收。

## 已记录性能基线（2026-07-30）

- 记录文件：`artifacts/performance/rc1-baseline.json`，使用 Windows 11 Pro、16GB 内存和当前 release 可执行文件。
- 连续 5 次首页可交互中位数为 981ms，P95 为 1125ms；空闲工作集中位数为 28.3MB。
- 十万项合成 7Z 的首批 500 项可见时间为 1353ms；损坏 ZIP 错误呈现后主窗口仍保持运行。
- 2,000 次高频进度模拟触发广播缓冲区丢弃，失败任务后下一任务可在 11ms 内完成；该事件节流缺口已列入已知问题。
