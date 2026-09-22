# QZip 项目审查报告（2026-09 全量复审）

审查基线：`main` 分支，提交 `d0a94c6`（`v2.0.2`），工作树干净（`git status --porcelain` 输出 0 项）。
版本：`2.0.2`（`package.json`、`Cargo.toml [workspace.package]`、`tauri.conf.json` 三处一致）。

审查方式：逐模块通读全部源码与配置后，按改动风险实机执行静态检查与运行级验证；所有行号以本基线为准。

覆盖范围：

- Rust：`crates/archive-core`、`crates/archive-security`、`crates/archive-sevenzip`、`crates/task-runtime`、`crates/platform-integration`、`apps/cli`、`apps/desktop/src-tauri`（9 个模块约 2 500 行）。
- 前端：`apps/desktop/src`（约 2 400 行 TS/TSX）、`packages/ui/src`。
- 构建与发布：`scripts/` 19 个 PowerShell/Python 脚本、`.github/workflows/`（`ci.yml`、`release.yml`、`macos.yml`）。
- 原生代码：`native/windows/qzip-shell/qzip_shell.cpp`（129 行，含压缩后的 COM 工厂实现）、NSIS 模板 `apps/desktop/src-tauri/windows/`。
- 文档族：`README.md`、`CHANGELOG.md`、`KNOWN_ISSUES.md`、`PRIVACY.md`、`DEPENDENCIES.md`、`QZip_PRD_V2.1.md`、`QZip_TODO_V2.1.md`、`AGENTS.md`。
- 交叉核验：`PROJECT_AUDIT.md`（基线 `037d0b6`）10 项问题与 `PROJECT_AUDIT_2026-07.md`（基线 `3b1b057`/`v1.1.2`）的修复状态。

## 一、本次实际执行的检查与结果

| 检查项 | 实际命令 | 结果 |
| :-- | :-- | :-- |
| ESLint | `node node_modules/eslint/bin/eslint.js .` | 通过，退出码 0，无告警 |
| TypeScript 类型检查（UI） | `tsc -p packages/ui/tsconfig.json --noEmit` | 通过，退出码 0 |
| TypeScript 类型检查（桌面端） | `tsc -p apps/desktop/tsconfig.json --noEmit` | 通过，退出码 0 |
| 前端单元测试 | `vitest run`（apps/desktop） | 8 个文件 / 33 项全部通过，退出码 0 |
| Rust 格式 | `cargo fmt --all -- --check` | 通过，退出码 0 |
| Rust Lint | `cargo clippy --workspace --all-targets -- -D warnings` | 通过，0 warning，退出码 0（55 s） |
| Rust 测试 | `cargo test --workspace` | 全部通过（共 63 项，1 项 ignored，无 failed），退出码 0 |
| Sidecar 完整性 | `scripts/fetch-sevenzip.ps1 -VerifyOnly` | 通过（`7-Zip sidecar is present and runnable.`） |
| Sidecar 真实回归 | `scripts/test-sevenzip.ps1` | 通过（`7-Zip integration test passed.`） |
| Web 构建 | `vite build`（apps/desktop） | 通过，退出码 0 |

补充说明：

- 本机无 `pnpm`（`corepack` 亦缺失），故用仓库内 `node_modules` 中的 `eslint`/`tsc`/`vitest`/`vite` 二进制等价执行 `pnpm check` 的各子步骤，未安装或改动任何依赖。
- 本机 Node 为 `v25.2.1`，与 `engines: ">=24 <25"`、`.nvmrc`（24）及 CI（`node-version: 24`）不一致；本次全部检查仍通过，但本地与 CI 工具链存在偏差。
- `vite build` 会在 `apps/desktop/dist/` 写入产物（`.gitignore` 已覆盖 `dist/`），执行后 `git status --porcelain` 仍为 0 项。

## 二、总体结论

工程基线在同类体量项目中**相当扎实**，未发现可远程利用的注入或 RCE 级缺陷。已确认的正面能力包括：Sidecar 永远以 argv 数组调用（不存在 shell 拼接面）、每次调用前对 `7z.exe` 与 `7z.dll` 做双文件 SHA-256 校验、创建与解压均采用"暂存目录 + 提交/回滚"事务模型、解压前多重路径与链接安全门、Tauri 能力最小化（无 fs/shell/dialog 插件权限）、更新下载具备主机白名单与校验和比对、前端不存在 XSS 汇聚点、密码全程 `SecretString` 且不落入持久化。

真实缺陷集中在四处：

1. **受信签名发布链路确定性断裂**（H1，`PROJECT_AUDIT_2026-07.md` 唯一未闭环项，本基线复现成功）；
2. **文档族与代码版本严重脱节**（H2，CHANGELOG/KNOWN_ISSUES/TODO/README 集体落后一至两个大版本）；
3. **崩溃与异常退出后的恢复缺口**（M1、M2：暂存目录与覆盖备份无启动回收，Windows 关闭窗口即打断任务）；
4. **发布校验与验收覆盖仍有断点**（M3、M4、M5：manifest 提交未校验、安装包内 Sidecar 未做哈希、RAR 无样本零覆盖、桌面 crate 缺少 unsafe 管控）。

既往审查项核验（本次复核）：

- `PROJECT_AUDIT_2026-07.md` H1（`QZIP_WINDOWS_PFX_PASSWORD` 变量缺失）：**仍未修复**，见本报告 H1。
- 同报告"manifest commit 未校验""安装包内嵌 sidecar 未做哈希校验"：**仍未修复**，见本报告 M3。
- 同报告 #3（TAR.GZ/TAR.XZ 浏览先完整解包到系统临时目录）：**浏览路径已修复为流式**（`archive-sevenzip/src/lib.rs:386-557`，`7z x -so | 7z l -si -ttar` 管道），**解压路径仍先展开到临时目录**（`archive-sevenzip/src/lib.rs:305-373`），属部分修复。
- 同报告部分问题（前端会话释放、任务历史串行化与保留上限、事件缓冲区回退重同步等）在 v2.0.2 代码中可见对应实现：`App.tsx:141-146`、`task-runtime/src/history.rs:39-67,70-78`、`lib.rs:764-792`。
- `PROJECT_AUDIT.md`（`037d0b6`）中 #4（事件转发永久停止）、#8（会话未释放）、#9（历史写入未串行化）已确认修复；#5（签名变量）即本报告 H1。

## 三、高严重度

### H1. 受信签名发布链路仍然断裂（阻断级，`PROJECT_AUDIT_2026-07.md` H1 未闭环）

**证据：**

- `.github/workflows/release.yml:130-131`：`build-nsis` job 只把机密映射为 `WINDOWS_PFX_BASE64`、`WINDOWS_PFX_PASSWORD`、`QZIP_WINDOWS_PUBLISHER`。
- `.github/workflows/release.yml:203`：签名准备步骤校验 PFX 通过后，仅导出 `QZIP_WINDOWS_PFX_PATH`，**没有** 导出 `QZIP_WINDOWS_PFX_PASSWORD`。
- `scripts/bundle-windows.ps1:47-49`：`-Release` 缺少 `QZIP_WINDOWS_PFX_PATH`、`QZIP_WINDOWS_PFX_PASSWORD`、`QZIP_WINDOWS_PUBLISHER` 任一即 `throw`。
- `scripts/build-windows-shell-integration.ps1:20-22`：同样的三变量强制校验，且明确禁止自签名回退。
- `scripts/configure-github-signing.ps1:32,34`：写入的 secret 名为 `WINDOWS_PFX_BASE64` / `WINDOWS_PFX_PASSWORD`，与脚本要求的环境变量名不一致。
- 全仓检索确认：除文档描述外，**没有任何工作流或脚本会设置 `QZIP_WINDOWS_PFX_PASSWORD`**。
- `.github/workflows/release.yml:211-216`：`signed_build` 步骤带 `continue-on-error: true`；`:228-245` 在可信构建失败后回退 `scripts/build-nsis.ps1 -UnsignedRelease` 并写出 `signing_mode=unsigned-degraded`。

**影响：** 即使维护者正确配置了证书机密，每次"可信签名"发布都会在 `build-windows-shell-integration.ps1` 或 `bundle-windows.ps1` 处抛错，随后被 `continue-on-error` 吞掉并静默降级为 `unsigned-degraded`：产物无法获得有效 Authenticode 签名、SmartScreen 提示"未知发布者"、Windows 11 一级现代右键菜单不可用；而 step summary 仍会显示"已尝试可信签名"，误导维护者认为签名生效。

**修复建议（一行改动 + 防回归）：**

- 在 `release.yml:203` 的导出块补充 `"QZIP_WINDOWS_PFX_PASSWORD=$env:WINDOWS_PFX_PASSWORD" | Add-Content -LiteralPath $env:GITHUB_ENV -Encoding UTF8`（值源自 secret，GitHub 自动掩码）。
- 在 CI（`ci.yml`）新增用临时测试证书走通 `trusted` 路径的 job，确保该分支不再长期无人执行。

### H2. 文档族与代码版本严重脱节

**证据：**

- `CHANGELOG.md:3` 最新条目仍为 `## 1.0.0 - 2026-08-24`，而仓库标签已到 `v1.1.0`、`v1.1.1`、`v1.1.2`、`v1.2.0`、`v1.2.1`、`v1.2.2`、`v2.0.0`、`v2.0.1`、`v2.0.2`（`git tag` + `package.json` 版本提交历史可证），即 1.1.x / 1.2.x / 2.0.x 全部缺失发布记录。
- `KNOWN_ISSUES.md:1` 标题为"v1.0.0 已知问题"；`:10` 称"macOS DMG、Finder Quick Action、Linux AppImage、MIME/Desktop Entry 不属于 Windows V1.0 范围，计划在 V1.1 提供"，但 `0a555ef`（v2.0.0）已实现 macOS 支持：`scripts/macos.mjs`、`.github/workflows/macos.yml`（`macos-15`/`macos-15-intel` 双架构 DMG + SHA256SUMS 上传）、`archive-sevenzip/src/lib.rs:57-60`（`7zz` + `QZIP_MAC_ENGINE_SHA256`）、`crates/archive-sevenzip/build.rs`、`desktop_platform.rs` 的 Finder/菜单/退出确认分支均已存在。
- `QZip_TODO_V2.1.md:13-15` 仍把 M2–M5、M6、M7 标为 `[~]` 未完成，与已发布的 2.0.x 事实不符。
- `README.md:11,52` 仅声明 `Windows 10 / 11 x64`，未提供 macOS 构建或下载说明，与上一条矛盾。
- 正面项：`PRIVACY.md:6` 对"密码经 7-Zip CLI 参数传递、同用户进程可见"的披露与代码行为一致（`archive-sevenzip/src/lib.rs:905,933,948,955,962`）；`DEPENDENCIES.md:16-17` 对 `clap`/`rpassword` 的描述与 `apps/cli/Cargo.toml:15-16` 一致。

**影响：** 用户与贡献者无法从文档判断真实功能边界、版本差异与已知缺陷；发布说明与实际资产（NSIS + macOS DMG）不匹配；`KNOWN_ISSUES.md` 会把"已支持 macOS"错误呈现为"未支持"。

**修复建议：** 补齐 `CHANGELOG.md` 的 1.1.x/1.2.x/2.0.x 条目（可依据既有 tag 与 `git log` 回溯）；`KNOWN_ISSUES.md` 更新标题与 macOS 现状；`README.md` 增补 macOS 章节；`QZip_TODO_V2.1.md` 按实际状态校正勾选。

## 四、中严重度

### M1. 崩溃或强杀后无残留清理，覆盖备份可能造成"原文件消失"的观感

**证据：**

- 创建暂存目录 `.qzip-create-<uuid>`（`crates/task-runtime/src/staging.rs:76`）、解压暂存目录 `.qzip-extract-<uuid>`（`:131`）、覆盖备份 `.qzip-overwrite-<uuid>`（`:332`）只在**进程内**被回收或回滚。
- 回滚日志 `ExtractionJournal` 完全在内存中（`staging.rs:187-247`），没有落盘 journal，也没有启动时的重建逻辑。
- 应用启动时只清理预览缓存（`apps/desktop/src-tauri/src/lib.rs:756` → `preview.rs:19-40`），**不存在**对 `.qzip-extract-*`、`.qzip-create-*`、`.qzip-overwrite-*` 的扫描、恢复或回收。
- 已有测试只覆盖进程内路径（`task-runtime/src/tests.rs:412-460,598-600,672-673`），没有覆盖崩溃恢复。

**影响：** 进程崩溃或被强杀后，用户目标目录内会残留隐藏暂存目录并占用磁盘；若崩溃发生在 `replace_existing_file`（`staging.rs:324-349`）的两次 `rename` 之间，原文件停留在 `.qzip-overwrite-<uuid>`，而目标位置为空，用户观感为"文件丢失"（数据未真正丢失，但无提示、无自动恢复）。`PROJECT_AUDIT_2026-07.md` 第 11 条已作为低severity记录该现象，本报告认为配合 M2 应升级为中severity。

**修复建议：** 在暂存目录内写入轻量 journal（目标路径、已创建/已替换条目），启动时对 `app_data_dir` 中登记过的目标目录执行恢复（回滚新建文件、还原备份）与孤儿目录回收；至少提供"检测到残留，是否恢复"的用户可见提示。

### M2. Windows 上关闭窗口即退出，进行中任务被直接打断

**证据：**

- 唯一的运行事件处理器 `apps/desktop/src-tauri/src/desktop_platform.rs:61` 的整个 `match` 语句受 `#[cfg(target_os = "macos")]` 约束（`:62`），因此 `RunEvent::ExitRequested`（`:86-92`，取消任务并等待事务回滚后退出）与 `WindowEvent::CloseRequested`（`:93-101`，阻止关闭并隐藏窗口）在 Windows 上**完全不生效**。
- `tauri::Builder` 链条中也没有 `.on_window_event(...)`（`lib.rs:742-757` 已逐行核对），全仓 `prevent_close`/`prevent_exit`/`QUITTING` 命中 5 处且全部位于 macOS 门控代码内。
- macOS 路径实现了完整的"仍有任务运行 → 弹窗确认 → 取消任务 → 等待清理完成 → 退出"（`desktop_platform.rs:113-141`）。

**影响：** Windows 用户点击标题栏关闭按钮或使用 `Alt+F4` 时，应用立即退出且不提示，正在进行的压缩/解压被中断；结合 M1，会留下未提交的暂存目录（用户数据不会写入目标目录，但残留与磁盘占用无人清理）。

**修复建议：** 将退出/关闭守卫跨平台化：Windows 上保持"最小化到托盘/隐藏窗口"或至少提供"仍有任务运行，取消并退出？"的确认流程，退出前调用 `tasks.cancel` 并等待终态。

### M3. 发布资产校验仍有断点（manifest 提交未比对、安装包内 Sidecar 未做哈希）

**证据：**

- `scripts/prepare-rc-release.ps1:66,72-84` 会把 `git rev-parse HEAD` 写入 manifest 的 `commit` 字段，并在 `sidecar` 段写入 `version` 与 `runtime.fileHashes`；但 `scripts/verify-release-assets.ps1` 全文未引用 `$manifest.commit`，也未比对 `$manifest.sidecar.fileHashes`。
- `release.yml:106,135` 已具备 `RELEASE_SHA`（构建 job 的 checkout 固定在 `client_payload.release_sha`，`release.yml:141`），因此"期望提交"是可用信息，只缺校验动作。
- `scripts/verify-release-assets.ps1:78-106` 对安装包载荷只做正则存在性检查：`unsigned-degraded` 分支要求 `Path = 7zip\\7z\\.exe$` 等，`trusted` 分支要求 shell 载荷存在，均**不校验内容哈希**。
- 缓解因素：`scripts/bundle-windows.ps1:50` 在打包前执行 `fetch-sevenzip.ps1 -VerifyOnly`；运行时每次调用 Sidecar 前仍做双文件 SHA-256（`archive-sevenzip/src/lib.rs:56-67,622-643`），因此实际攻击面有限，缺的是发布门禁自身的闭环。

**影响：** manifest 可被替换为来自其它提交（或不同 Sidecar 版本）的产物而照样通过 `prepare-rc-release.ps1` + `verify-release-assets.ps1` 的校验；安装包内嵌的 `7z.exe`/`7z.dll` 是否与记录的哈希一致，发布链无证据。

**修复建议：** 在 `verify-release-assets.ps1` 增加 `if ($manifest.commit -ne $env:RELEASE_SHA) { throw ... }`（或在参数中显式传入期望提交）；用捆绑的 7-Zip 将安装包内的 `7zip/7z.exe`、`7zip/7z.dll` 抽取到临时目录后与 `manifest.sidecar.fileHashes` 比对。

### M4. 对外声明支持 RAR，但仓库无 RAR 样本、CI 零覆盖

**证据：**

- `crates/archive-sevenzip/src/lib.rs:662-675` 的 `readable_formats` 包含 `ArchiveFormat::Rar`；`README.md:35` 也对外声明可查看/解压 RAR。
- `tests/fixtures/compat/manifest.json:23-31` 的 WinRAR 用例为 `"status": "blocked"`、`"sha256": ""`、`"blocker": "Producer unavailable on this machine."`；`git ls-files tests/` 确认仓库内不存在任何 `.rar` 文件。
- `scripts/verify-compat-fixtures.ps1:11` 对 `blocked` 用例直接跳过；`scripts/test-core-regression.ps1:2,11` 需要显式 `-RarSample` 才测试 RAR，而 `.github/workflows/ci.yml:41` 调用 `pnpm test:core:windows` 时未传该参数。
- 结论：RAR 的列表/解压路径在 CI 中**从不执行**，仅在带样本的本机手工运行时可覆盖。

**影响：** 7-Zip 上游对 RAR5（尤其新版本引入的压缩方法）的兼容行为可能回归而无人察觉；若某格式实际不可用，文档与界面（`readableFormats` 驱动）仍会向用户承诺支持。

**修复建议：** 纳入一个可再分发的最小 RAR 样本（或由 `generate-local-compat-fixtures.ps1` 生成并附哈希），在 CI 中传入 `-RarSample`；在样本缺失时让回归脚本显式输出 skip 记录，而非静默不执行。

### M5. 桌面主 crate 缺少 unsafe 管控

**证据：**

- 6 个 crate（`archive-core`、`archive-security`、`archive-sevenzip`、`task-runtime`、`platform-integration`、`apps/cli`）与示例 `task-runtime/examples/performance_probe.rs` 均在首行声明 `#![forbid(unsafe_code)]`；**唯独 `apps/desktop/src-tauri/src/lib.rs` 及其模块没有该约束**（`lib.rs` 第 1 行直接是 `use std::...`）。
- `apps/desktop/src-tauri/src/system_icons.rs:27,40,41,59,72,83,87,89,90,103,105,107` 与 `update_download.rs:287-289` 存在 FFI `unsafe` 块（`SHGetFileInfoW`/GDI 位图/`ShellExecuteW`）。
- 人工复核结论：`system_icons.rs` 的句柄生命周期基本正确——`CreateDIBSection` 失败时释放 DC（`:82-85`）、`SelectObject` 复原后才 `DeleteObject`（`:104-110`）、`SHGetFileInfoW` 使用 `SHGFI_USEFILEATTRIBUTES` 不触盘（`:33`）；仅在 `previous.is_null()` 的异常分支上仍会删除仍被选中的位图（`SelectObject` 失败属罕见路径）。`update_download.rs:287` 的 `ShellExecuteW` 输入来自已校验的本地状态。

**影响：** 仓储内绝大多数代码受编译器级 unsafe 禁令保护，而风险最高的 FFI 模块反而没有任何机制阻止后续无意扩大 unsafe 范围。

**修复建议：** 在 `apps/desktop/src-tauri/src/lib.rs` 增加 `#![deny(unsafe_code)]`，对 `system_icons`、`update_download` 两个模块加模块级 `#[allow(unsafe_code)]` 并补 `// SAFETY:` 说明；`system_icons.rs` 的失败分支调整为先 `SelectObject` 复原再释放。

## 五、低严重度（汇总）

1. **CSP 未配置**：`apps/desktop/src-tauri/tauri.conf.json:27-29` 的 `security.csp` 为 `null`，Tauri 不注入任何内容安全策略；结合"自定义 `#[tauri::command]` 默认对 webview 开放"，属纵深防御缺口。当前前端全库无 `dangerouslySetInnerHTML`/`innerHTML`/`eval`/`new Function`（已 grep 全仓确认，命中仅出现在历史审查文档中），因此不是可利用漏洞，但一旦上游依赖引入 XSS，攻击者可调用全部 IPC。建议改为 `"default-src 'self'"` 起步并逐步收敛 `connect-src`/`img-src`。
2. **性能标记命令在正式构建中仍注册**：`record_performance_marker`（`lib.rs:696-740`）通过 `QZIP_PERF_MARKER_PATH` 环境变量 + 临时目录 + `qzip-performance-markers-*.jsonl` 前缀校验限制写入位置，但生产构建中依然可用。建议用 `#[cfg(debug_assertions)]` 或 feature 门控，仅在 RC 性能采集时启用。
3. **密码经命令行参数传递**：`archive-sevenzip/src/lib.rs:905`（创建）、`:933`（解压）、`:948`（列表）、`:955`（测试）、`:962`（更新）把密码拼入 `-p{password}`；同用户进程/工具在任务运行期间可读取该参数。`PRIVACY.md:6` 已如实披露，属 7-Zip CLI 固有限制。可选改进：改用 `-p`（空值）配合 stdin 读取，或走 7z.dll/`-si` 方案。
4. **重复列表开销**：单次解压会触发三次列表（`task-runtime/src/lib.rs:456-465` 风险预检、`archive-sevenzip/src/lib.rs:751-765` 解压前路径校验、7-Zip 自身读取），十万项大包上有可测的额外耗时。建议把两次 Rust 侧列表合并为一次并复用结果。
5. **错误分类偏差**：`map_exit` 将 `cannot open ...` 归入 `CorruptArchive`（`archive-sevenzip/src/lib.rs` 的错误映射），会使密码错误/路径不可访问被报告为"压缩包损坏"，影响用户自助排查。
6. **取消与完成的竞态**：`TaskManager::cancel`（`task-runtime/src/lib.rs:210-251`）可能把刚进入终态的任务短暂标记为 `Cancelling`；终态有校正，影响限于任务卡短暂闪动。
7. **未忽略的空目录**：`.tmp-pnpm/`（仓库根，当前为空）未被 `.gitignore` 覆盖；`target/`、`target-rc1-verify/`、`artifacts/*`、`third_party/7zip/bin|source` 均已正确忽略。
8. **`KNOWN_ISSUES.md` 未覆盖 2.0 新增行为**：压缩包内文件"预览打开"的扩展名黑名单策略（`preview.rs:46-86`，含 `.exe/.ps1/.js/.lnk` 等 31 类）、应用内更新下载与校验流程（`update_download.rs`）、macOS 权限/公证缺失等均未在已知问题中体现。

## 六、正面确认（含代码证据）

- **无 shell 注入面**：Sidecar 一律以 argv 数组调用，注释与实现一致（`archive-sevenzip/src/lib.rs:1-4,69-91`）；Windows 使用 `CREATE_NO_WINDOW`（`:409,429`）；诊断输出以 64 KB 上限截断（`:41`）。
- **Sidecar 完整性**：每次调用前校验 `7z.exe` + `7z.dll` 双哈希（`archive-sevenzip/src/lib.rs:56-67,622-643`）；macOS 由 `build.rs` 在编译期注入并断言 64 位十六进制哈希。
- **事务化创建与解压**：创建走 `.qzip-create-<uuid>` 暂存 + `commit_created_archive`（`staging.rs:62-119`）；解压走 `.qzip-extract-<uuid>` 暂存 + journal 提交，覆盖采用"备份 → 替换 → 成功删备份/失败恢复"（`staging.rs:155-349`）；文件被占用映射为 `FileInUse`（Win 32/33，`staging.rs:351-359`）并有专项测试（`task-runtime/src/tests.rs:514-556`）。
- **风险门禁与路径安全**：拒绝绝对路径、UNC、盘符前缀、`..`、Windows 保留名、尾随点/空格（`archive-security/src/lib.rs:67-126`）；`output_path` 再做包含性与 `ParentDir` 复核（`:127-137`）；符号链接、硬链接、不安全路径为**不可覆盖**风险（`:199-211`），可覆盖风险需用户在界面显式确认后才发送 `acceptRisk`（`ArchivePages.tsx:341,371`）；批量解压固定 `acceptRisk:false` 并跳过风险包（`App.tsx:277`），与 `KNOWN_ISSUES.md:9` 一致。
- **重解析点防护**：提交阶段对目标与暂存条目做符号链接/junction（`FILE_ATTRIBUTE_REPARSE_POINT`）检查（`staging.rs:172,264,278,385-398`），覆盖同名文件与目录的冲突矩阵有测试（`tests.rs:431-495`）。
- **更新下载硬化**：仅 HTTPS + 主机白名单（`github.com`/`api.github.com`/`release-assets.githubusercontent.com`/`objects.githubusercontent.com`）+ 重定向上限 5（`updates.rs:135-160`）；资产名与 URL 由版本严格模板拼接（`:94-129`）；大小双重校验、SHA-256 比对、安装前再次哈希（`update_download.rs:109-158,255-259`）。
- **Tauri 能力最小化**：`capabilities/default.json` 仅授予 window 控制、webview zoom 与 notification，未启用 fs/shell/dialog/process 插件权限；单一窗口 `label: "main"` 绑定。
- **密码与隐私**：桌面端全程 `SecretString`（`lib.rs:199-201,517,548,563,579`）；任务快照与历史不含密码（`task-runtime/src/lib.rs:3-4` 模块注释，`platform-integration/src/lib.rs:296-299` 有测试锁定）；`telemetry_enabled` 在 `apply` 中被强制为 `false`（`platform-integration/src/lib.rs:227`）；CLI 只接受 `--password-stdin`/`--password-prompt`（`apps/cli/src/main.rs:75-83,275-297`）。
- **任务历史可靠性**：tmp 文件 + `sync_all` + 平台差异化替换（POSIX 原子 rename / Windows 备份回滚）+ 启动恢复 + 100 条上限 + 写锁串行化（`task-runtime/src/history.rs:11,39-67,118-153`）。
- **事件流回退处理**：`broadcast` 缓冲区滞后时重新订阅并回放快照（`lib.rs:764-792`），避免旧进度事件覆盖终态（对应 `PROJECT_AUDIT.md` #4）。
- **发布门禁方向正确**：`unsigned-degraded` 严禁携带 `QZip.Shell.msix`/`qzip-shell.dll`/注册脚本（`verify-release-assets.ps1:80-93`），签名模式双向严格断言（`:37-51,64-74`）；`PROJECT_AUDIT_2026-07.md` 记录的"未签名资产会被严格拒绝"在脚本层面仍成立。

## 七、建议实施顺序

| 优先级 | 动作 | 对应项 | 验收方式 |
| :-- | :-- | :-- | :-- |
| P0 | 在 `release.yml:203` 补 `QZIP_WINDOWS_PFX_PASSWORD` 导出，并在 CI 用测试证书走通 trusted 路径 | H1 | `trusted` 分支 CI 实际产出 `signing_mode=trusted` 且 `verify-release-assets.ps1 -RequireTrustedSignature` 通过 |
| P0 | 同步 `CHANGELOG.md`（1.1.x/1.2.x/2.0.x）、`KNOWN_ISSUES.md` 标题与 macOS 现状、README 平台说明、TODO 勾选 | H2 | 文档版本号与 tag、`tauri.conf.json` 版本一致，macOS 表述与 `macos.yml` 资产一致 |
| P1 | 暂存 journal 落盘 + 启动恢复/回收（`.qzip-extract-*`/`.qzip-create-*`/`.qzip-overwrite-*`） | M1 | 新增崩溃恢复单元测试：杀掉进程后重启能还原备份并清理暂存 |
| P1 | 退出/关闭守卫跨平台化，退出前取消任务并等待终态 | M2 | Windows 上手测"任务运行中关闭窗口"提示与回滚 |
| P1 | `verify-release-assets.ps1` 比对 `manifest.commit` 与 `RELEASE_SHA`、抽取安装包内 Sidecar 校验哈希 | M3 | 构造错误 commit / 篡改 7z.exe 的负例必须被拒绝 |
| P2 | RAR 样本入 CI（或在能力声明与文档中降级表述） | M4 | `ci.yml` 中 RAR 用例实际执行并输出通过记录 |
| P2 | 补齐风险引擎与恶意矩阵测试（压缩炸弹、深度、条目数、磁盘不足、链接） | M4/M5 关联 | `cargo test` 新增用例覆盖 `assess_entries` 全部分支 |
| P2 | `lib.rs` 增加 `#![deny(unsafe_code)]` + 两个 FFI 模块局部 `#[allow]` 与 `// SAFETY:` | M5 | `cargo clippy -- -D warnings` 仍通过，unsafe 范围不再扩大 |
| P3 | 配置 CSP、性能命令门控、合并重复 list、修正 `map_exit` 分类 | 低严重度 1/2/4/5 | 前端构建与 `pnpm check` 通过，错误码分类有测试 |

## 八、未验证项与边界

以下内容本次**未执行**，报告中相关结论仅来自源码、脚本与配置通读，不构成实测通过；后续验证需在具备相应环境与授权的前提下进行：

- **安装包与系统集成**：NSIS 打包、真实安装、覆盖安装的关联保留、卸载恢复（涉及注册表与系统文件关联，需用户授权且会改动系统状态）；`tests/acceptance/windows-sandbox/*.wsb` 沙箱验收脚本未运行。
- **真实启动路径**：双击启动、已有实例接收文件（单实例 `qzip://launch-request`）、Windows 资源管理器右键菜单与文件关联的实际行为；`AGENTS.md` 要求此类改动必须做真实场景验证，本次为只读审查，未触发。
- **发布链端到端**：受信签名的真实发布（本机无凭据，且不触发 GitHub Actions）；`release.yml` 的 `publish` job、attestation 与 Release notes 生成未运行。
- **macOS 相关**：`macos.mjs fetch/check/build`、DMG 产物、Gatekeeper/公证；当前为 Windows 环境。
- **性能与压力**：十万项大包列表基线、高频进度突发丢事件、内存占用基线（`task-runtime/examples/performance_probe.rs` 未运行）。
- **恶意样本矩阵**：真实压缩炸弹、磁盘不足、符号链接/硬链接、Windows 保留名与超长路径的端到端解压行为。
- **完整工具链等价性**：`pnpm check` / `pnpm test:core:windows` 未以 pnpm 原样执行（本机无 pnpm），已用等价二进制逐项替代并通过；CI 的 Windows 运行器结论未获取。

## 九、附录

### A. 检查复现命令

```powershell
# 前端
node node_modules/eslint/bin/eslint.js .
node node_modules/typescript/bin/tsc -p packages/ui/tsconfig.json --noEmit
node node_modules/typescript/bin/tsc -p apps/desktop/tsconfig.json --noEmit
Push-Location apps/desktop; & node_modules/.bin/vitest.CMD run; Pop-Location
Push-Location apps/desktop; & node_modules/.bin/vite.CMD build; Pop-Location

# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 7-Zip Sidecar
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-sevenzip.ps1 -VerifyOnly
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/test-sevenzip.ps1
```

### B. 关键文件索引（本报告引用）

| 主题 | 文件 |
| :-- | :-- |
| 发布链与签名 | `.github/workflows/release.yml`、`scripts/bundle-windows.ps1`、`scripts/build-windows-shell-integration.ps1`、`scripts/build-nsis.ps1`、`scripts/prepare-rc-release.ps1`、`scripts/verify-release-assets.ps1`、`scripts/configure-github-signing.ps1` |
| 暂存与提交 | `crates/task-runtime/src/staging.rs`、`crates/task-runtime/src/lib.rs`、`crates/task-runtime/src/history.rs` |
| 路径与风险 | `crates/archive-security/src/lib.rs` |
| 7-Zip 适配 | `crates/archive-sevenzip/src/lib.rs`、`crates/archive-sevenzip/build.rs`、`third_party/7zip/manifest.json` |
| 桌面宿主 | `apps/desktop/src-tauri/src/lib.rs`、`desktop_platform.rs`、`preview.rs`、`update_download.rs`、`updates.rs`、`system_icons.rs`、`shell_integration.rs`、`capabilities/default.json`、`tauri.conf.json` |
| 前端 | `apps/desktop/src/app/App.tsx`、`features/archive/ArchivePages.tsx`、`lib/archiveClient.ts`、`contracts/archive.ts` |
| 平台契约 | `crates/platform-integration/src/lib.rs` |
| 测试与样本 | `crates/task-runtime/src/tests.rs`、`tests/fixtures/compat/manifest.json`、`scripts/test-sevenzip.ps1`、`scripts/test-core-regression.ps1`、`scripts/verify-compat-fixtures.ps1` |

### C. 与既往审查的关系

- `PROJECT_AUDIT.md`（基线 `037d0b6`）：10 项问题，本次复核确认 #4、#8、#9 已修复；#5（签名变量）仍未闭环（本报告 H1）。
- `PROJECT_AUDIT_2026-07.md`（基线 `3b1b057`/`v1.1.2`）：H1 未闭环（本报告 H1）；"manifest commit 未校验""安装包内嵌 Sidecar 未做哈希"未修复（本报告 M3）；#3 属部分修复（浏览流式、解压仍展开）；新增本报告 M1、M2、M5 与 H2。





