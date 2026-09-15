# QZip 项目审查报告（2026-07 全量复审）

审查基线：`main` 分支，提交 `3b1b057`（`v1.1.2-15`），工作树干净。版本 1.1.2。
审查方式：全部源码逐文件通读（6 个 Rust crate、Tauri 后端、前端、CLI、C++ shell 扩展、
NSIS 钩子、全部 PowerShell 脚本与 GitHub Actions 工作流），并交叉核验
`PROJECT_AUDIT.md`（基线 `037d0b6`）中 10 项既往问题的修复状态。
所有行号以本基线为准。

## 总体结论

工程基线在同类体量项目中**相当扎实**：路径安全多层纵深（`safe_relative_path` 双重校验 +
`output_path` 包含性检查 + 风险评级门）、fail-closed 发布链（provenance 校验、哈希先于执行、
签名模式双向严格断言）、Tauri 能力最小化（无 shell/fs/dialog 插件、updater feature 默认关闭）、
无 XSS 汇聚点、密码处理规范（CLI 走 stdin/prompt、桌面端 `SecretString`）、遥测永久关闭、
隐私声明与代码行为一致。未发现注入或 RCE 级缺陷。

真实缺陷集中在四处：

1. **受信签名链路断裂**（H1，既往审计唯一未闭环项，确定性复现）——所有"受信"发布实际
   都以 unsigned-degraded 上架；
2. **发布验证链有两处断点**（manifest commit 未校验、安装包内嵌 sidecar 未做哈希校验）；
3. **回归保障缺口**（验收 harness 陈旧到无法运行、CI 不构建发布路径、风险评级引擎几乎
   无测试、CI 无 RAR 样本）；
4. **文档族整体落后一个版本**（CHANGELOG 缺失全部 1.1.x 历史、许可证清单实质缺失、
   三处文档与实际行为矛盾）。

既往审计 10 项问题：8 项确认修复（#1、#2、#4、#6、#7、#8、#9、#10），1 项部分修复
（#3 浏览已流式、解压/测试未），1 项**未修复**（#5 签名变量）。

---

## 高严重度

### H1. 受信签名静默退化为 unsigned-degraded（既往审计 #5，仍未修复）

**证据：**

- `release.yml:129-136`：build-nsis job env 只映射 `WINDOWS_PFX_BASE64`、
  `WINDOWS_PFX_PASSWORD`、`QZIP_WINDOWS_PUBLISHER`，无 `QZIP_WINDOWS_PFX_PASSWORD`。
- `release.yml:203`：签名准备步骤验证 PFX 成功后，仅向 `GITHUB_ENV` 导出
  `QZIP_WINDOWS_PFX_PATH`。
- `scripts/bundle-windows.ps1:40-42` 与 `scripts/build-windows-shell-integration.ps1:20-21`：
  `-Release` 构建强制要求 `QZIP_WINDOWS_PFX_PASSWORD`，缺失即 `throw`。
- `release.yml:211-216`：`signed_build` 带 `continue-on-error: true`；`:224-245` 在可信构建
  失败后回退 `-UnsignedRelease` 并输出 `signing_mode=unsigned-degraded`，publish 照常执行。
- 全仓 grep 证实没有任何 workflow 设置该变量。
- 佐证：`RC2_ACCEPTANCE.md:28` 将"受信任发布签名"列为 V1.0 前保留项，至今未闭环。

**影响：** 即使正确配置了证书 secrets，每次"受信"发布都必然落入 unsigned-degraded——
SmartScreen/未知发布者警告、Win11 一级现代右键菜单不可用；step summary 声称"受信凭据已验证"
会误导维护者以为签名生效。

**修复建议（一行改动）：** 在 `release.yml:203` 的导出块补充
`"QZIP_WINDOWS_PFX_PASSWORD=$env:WINDOWS_PFX_PASSWORD" | Add-Content -LiteralPath $env:GITHUB_ENV`
（密码源自 secret，GitHub 自动掩码）。同时让 CI 用测试证书走通 trusted 路径防回归（见 R3）。

---

## 中严重度

### 发布与 CI 链路

#### R1. release-manifest 的 `commit` 字段从未被校验（provenance 链断一环）

**证据：** `prepare-rc-release.ps1:66` 写入 `commit = git rev-parse HEAD`；
`verify-release-assets.ps1:32-51` 只校验 version/assets/签名模式，从不读取 `.commit`；
publish 调用（`release.yml:289-296`）不传期望 commit，`gh release create --target`
（`:319-327`）使用的是 dispatch 载荷 SHA。

**影响：** 伪造/过期 manifest 只要与 checksum、签名、载荷一致即可通过验证；发布页"来源
commit"与实际构建来源可能不一致。

**建议：** `verify-release-assets.ps1` 增加 `-ExpectedCommit` 参数并与 `manifest.commit` 强制
相等；publish 步骤传 `$RELEASE_SHA`。

#### R2. 安装包内嵌 sidecar（7z.exe/7z.dll）未做哈希校验

**证据：** `verify-release-assets.ps1:83-105` 只检查载荷文件"存在/必须缺失"；manifest 已含
`sidecar.fileHashes`（`prepare-rc-release.ps1:82`），验证端从未使用。

**影响：** 被篡改或换版的 7-Zip sidecar 可混入发布物并通过发布验证。

**建议：** 验证时用 7z 从安装包解出 `7zip\7z.exe|dll`，与 manifest `sidecar.fileHashes` 逐一对
SHA-256。

#### R3. CI 从不构建发布路径，且缺 PRD §34 要求的许可证门禁

**证据：** `ci.yml:36-43` 仅 sidecar:fetch、`pnpm check`、核心回归、debug Tauri 构建；
`bundle-windows.ps1`/`build-nsis.ps1`/`build-windows-shell-integration.ps1`/
`verify-release-assets.ps1` 全链路只在发布时执行；PRD §34 要求的"许可证检查"无对应步骤。

**影响：** 安装器/签名/验证回归（包括 H1 这类确定性断链）要到发布才暴露，发现即已造成版本
污染；许可证合规无门禁。

**建议：** CI 增加一条任务用开发签名跑 `build-nsis` + `verify-release-assets`
（unsigned-degraded 全链路）；对 `scripts/` 变更触发；补 license-check（见 D2）。

#### R4. test-sevenzip.ps1 正向路径不检查退出码（静默假通过）

**证据：** `scripts/test-sevenzip.ps1:26-30、52-61` 形如
`cargo run ... | Select-String 'pattern' | Out-Null`，未检查 `$LASTEXITCODE`
（负向路径 `:43-46、70-91` 有检查）。

**影响：** 若 CLI 输出错误 JSON（无匹配行），`Select-String` 无输出且不抛错，集成测试静默
通过。

**建议：** 封装辅助函数：同时要求 `$LASTEXITCODE -eq 0` 且匹配成功，否则 throw。

#### R5. 验收测试 harness 与当前版本失配（发布前回归实际失效）

**证据：** `tests/acceptance/windows-sandbox/run-rc1-sandbox-acceptance.ps1:4-5` 默认安装器为
`QZip_0.1.0`/`QZip_1.0.0`，`:215` 硬编码 `Test-InstalledIntegration '1.0.0'`；
`QZip-RC1-Upgrade.wsb:20` 基线又写 `0.9.0`，与脚本默认值互相矛盾；`.wsb` 还硬编码宿主路径
`D:\AICode\QZip`/`C:\QZip`。当前版本为 1.1.2。

**影响：** 验收测试对当前构建无法跑通，升级场景基线不一致，发布前回归保障实际失效。

**建议：** 期望版本参数化（或按 `QZip_*_x64-setup.exe` 通配 + manifest 推导）；统一 .wsb 与
脚本默认值；移除宿主机绝对路径。

#### R6. CI 无 RAR 样本，RAR 路径是回归盲区

**证据：** `scripts/test-core-regression.ps1:2` 的 `-RarSample` 为可选参数；
`ci.yml:41` 的 `pnpm test:core:windows` 未提供样本。

**影响：** RAR 解压/密码/错误映射路径在 CI 从不执行。

**建议：** 向 `tests/fixtures` 提交一个小型开放许可 RAR 样本，CI 传 `-RarSample`。

#### R7. 7-Zip sidecar 供应链：in-repo manifest 是唯一信任根

**证据：** `third_party/7zip/manifest.json` 中 URL 与 SHA-256 在同一文件；
`fetch-sevenzip.ps1:46-50` 下载后先验哈希再使用（fail-closed 良好），但持有提交权的攻击者
可在一个 PR 中同时替换 URL 与哈希。无 CODEOWNERS、无带外锚点、无 sidecar 独立 attestation。

**影响：** 供应链信任根单点（缓解因素：运行时 Rust 常量 `EXPECTED_*_SHA256`
（`archive-sevenzip/src/lib.rs:34-38`）与 manifest 一致且每次调用复核，manifest 被篡改后
运行时也会拒绝）。

**建议：** manifest 加 CODEOWNERS + 强制 review；已知良好哈希另存于独立 secret 做交叉断言；
或对 sidecar 生成独立 attestation。

#### R8. 开发证书零校验导入机器根存储

**证据：** `scripts/install-qzip-development-certificate.ps1:8-12` 仅做存在性与管理员检查，
即 `Import-Certificate -CertStoreLocation Cert:\LocalMachine\Root`，无 Subject/EKU/链校验
（对比 `configure-github-signing.ps1:18-23` 有完整校验）。

**影响：** 管理员对来源不可信的 .cer 运行本脚本，会给攻击者证书机器级代码签名信任。

**建议：** 导入前强制校验 Subject（CN=QZip Development）与 CodeSigning EKU，且限定输入文件
来自已知工件路径。

### 应用安全加固

#### S1. 全局无 CSP（缺少 XSS 纵深防线）

**证据：** `tauri.conf.json:27-29` `"security": { "csp": null }`；`index.html` 无 CSP meta。

**影响：** Tauri v2 应用自定义 `#[tauri::command]` 默认对 webview JS 开放。当前前端无 XSS
汇聚点（已 grep 验证无 `dangerouslySetInnerHTML`/`eval` 等），属纵深防御缺口而非可利用
漏洞；一旦上游依赖引入 XSS，攻击者可调用全部 IPC。

**建议：** 配置严格策略，例如：
`default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' https://api.github.com; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`。

#### S2. `open_path` / `reveal_in_file_manager` 接受前端任意路径

**证据：** `preview.rs:216-237`。两者均为同步命令，路径完全由前端给定。

**影响：** 与 S1 组合后是"用默认应用打开任意文件 / 资源管理器定位任意路径"的原语。正常 UI
路径传的是后端返回的 `task.output`（`TaskCenter.tsx:170-171`），当前风险低。

**建议：** 对传入路径做来源校验（仅允许已知任务产物 / 已打开压缩包的派生路径），或后端维护
"本会话合法打开目标"白名单。

#### S3. 预览可打开无扩展名文件（会被系统直接执行）+ 阻止列表缺口

**证据：** `preview.rs:46-86` 的阻止列表为黑名单（31 个扩展名）；`path.extension()` 为空时
`preview_extension_is_blocked` 返回 false，`explorer.exe <文件>` 对无扩展名可执行文件会直接
运行。列表还缺 `.pif`、`.appref-ms`、`.sct`、`.msc`、`.chm` 等可执行/载荷载体。

**影响：** 攻击者需让受害者解压恶意包并点击"预览"，增量风险有限（攻击者本可让受害者直接
打开文件），但"我只是看看"场景下这是代码执行向量。

**建议：** 拒绝无扩展名条目；阻止列表补充上述扩展名；中期考虑对可预览类型改白名单。

#### S4. 会话准备阶段无条目数量上限（内存耗尽 DoS）+ 列表无超时

**证据：** `lib.rs:386-396` `prepare_archive_session` 对 `backend.list` 不设数量上限，全部
条目载入内存并存入 `sessions`；`ExtractionLimits::max_entries=100_000`
（`archive-security/src/lib.rs:18-28`）只在解压前的 `assess_entries` 生效，浏览/准备阶段不受
约束；list 调用的 `CancellationToken` 新建后从不触发，无超时。

**影响：** 恶意构造的超大条目压缩包（如数千万条目）在受害者仅"打开浏览"时即可造成内存
耗尽；超大包列表期间 UI 长时间停留在加载状态。

**建议：** `prepare_archive_session` 复用/提前应用 `ExtractionLimits`（条目数超过上限即拒绝
或降级为"仅目录"模式）；给 list 加超时；配合 S5 的会话淘汰。

#### S5. `sessions` Map 无上限、无淘汰

**证据：** `lib.rs:826`（`sessions: Mutex::new(HashMap::new())`）；`close_archive_session`
仅前端显式调用时删除。前端当前配对良好（已验证，含 `App.test.tsx:63` 回归测试），但后端无
兜底。

**影响：** XSS 或未来前端回归可导致会话/内存无界增长。

**建议：** 加上限（如 8 个）+ LRU 淘汰；或按 `(archive, fingerprint)` 复用已有会话。

#### S6. 密码以 `-p` 明文出现在 7z 进程命令行

**证据：** `archive-sevenzip/src/lib.rs` 的 `build_list/extract/create` 参数拼装
（`-p{password}`）；`PRIVACY.md:6` 已如实披露。

**影响：** 同用户进程/工具在任务运行期间可看到密码参数。这是 7-Zip CLI 的固有限制（无
stdin 密码通道），无法在不换后端的前提下消除。

**建议：** 接受该限制并在文档中保持披露（现状可接受）；中期评估 7z 24+ 的 `-p` 替代或
`codecs` 方案；至少避免密码进入日志、任务快照与错误信息（现状满足）。

#### S7. 每次调用对 7z.exe + 7z.dll 全量 SHA-256

**证据：** `archive-sevenzip/src/lib.rs` `verify_runtime_files` 在每次进程启动前完整哈希两个
文件。

**影响：** 每次任务增加数十毫秒~百毫秒级开销；且"哈希时"与"执行时"之间存在理论 TOCTOU
窗口（同用户可替换文件）。收益/成本比偏低：文件已随安装包分发且路径受控。

**建议：** 进程启动时校验一次并缓存结果（以 mtime+size 为失效条件），或每会话校验一次。

#### S8. tar.gz/txz 的解压与测试仍完整展开内层 TAR 到 %TEMP%（既往 #3 部分未闭环）

**证据：** `archive-sevenzip/src/lib.rs` `expand_tar_wrapper` 仍被 extract/test 路径使用；
流式修复（`list_tar_wrapper_streaming`，`:376`）只覆盖 list。

**影响：** 用户"解压"一个 .tar.gz 时，内层 TAR 先被完整解到 `%TEMP%`，无展开前大小/空间
限制，磁盘占用与解压结果成倍。

**建议：** 与 #3 同样的流式思路扩展到 extract（外层 `x -so` → 内层 tar 解包直接落 staging
目录）；至少补展开前的条目预检与空间估算。

#### S9. `create_tar_compressed` 中间 .tar 未做磁盘空间预检

**证据：** 创建 .tar.gz/.tar.xz 时先在 `%TEMP%\qzip-{uuid}` 生成完整中间 `.tar` 再压缩。

**影响：** 大目录打包时 `%TEMP%` 所在卷需要约 2 倍临时空间，未检查可致中途 ENOSPC。

**建议：** 创建前检查临时卷可用空间 ≥ 输入总大小，不足时给出明确错误。

#### S10. `archive-security` 风险评级/解压前防护几乎无测试

**证据：** `crates/archive-security/src/lib.rs` 268 行仅 3 个测试（`:241-267`）：
`safe_relative_path` 基本接受/拒绝向量 + UnsafePath 不可覆盖。8 个风险码中
EntryCount/TotalSize/SingleFileSize/Depth/CompressionRatio/InsufficientDisk/Symlink/Hardlink
全部无测试；压缩炸弹比公式（`:161-167`）、磁盘预留 512MB+total/20（`:168-177`）、限额
（10 万条目/256GB/64GB/64 层）无边界验证；`output_path()`（`:127-137`）完全无测试；
`tests/security/` 为空目录。

**影响：** 解压前安全门是 zip 炸弹/路径穿越/符号链接注入的最后防线，公式回归（如磁盘预留
系数）无回归保护。

**建议：** 为 `assess_entries` 每个风险码补参数化边界测试（限额±1）；`safe_relative_path`/
`output_path` 补攻击向量边界（中段穿越 `a/../../x`、单独 `..`、反斜杠 UNC）；补真实压缩包
符号链接/炸弹集成用例。

### 文档与一致性

#### D1. CHANGELOG.md 缺失全部 1.1.x 历史

**证据：** `CHANGELOG.md` 仅 3 条条目（1.0.0、1.0.0-rc.1、0.1.0）；git 中 `v1.1.0`/
`v1.1.1`/`v1.1.2` 标签均存在，`v1.1.2..HEAD` 15 个提交（流式 TAR 浏览、事件 resync、
TGZ/TXZ、更新检查、会话生命周期、历史并发、模块拆分、文件关联启动等）无一写入
CHANGELOG；发布流程的版本提交白名单不含 CHANGELOG，无强制机制。

**影响：** 仓库内无法追溯 1.1.x 变更；变更说明只存在于 GitHub Release 页面，仓库文档与发布
文档脱节。

**建议：** 补 1.1.0/1.1.1/1.1.2 条目及 15 个发布后提交；让 `bump-version.ps1` 或发布工作流
强制同步 CHANGELOG。

#### D2. THIRD_PARTY_LICENSES.md 实质是空的许可证清单

**证据：** 全文 14 行，唯一实质条目是 7-Zip（细节指向 manifest）；直接依赖实际为
Rust 24 个（tauri、tokio、reqwest、clap 等）+ npm 24 个（react、@fluentui/react-icons、
zustand 等），无任何许可证记录；PRD §34 要求的 CI 许可证检查未落地（见 R3）。
兼容性核查结论（正面）：直接依赖中无 GPL/AGPL；7-Zip 以独立未修改 sidecar 调用
（LGPL-2.1-or-later，unRAR 限制），与当时的 Apache-2.0 许可无冲突。

**影响：** 当时的 Apache-2.0 分发项目缺少可交付的许可证审计清单；法律义务主体（7-Zip LGPL）已
覆盖，其余风险有限，但审计/交付合规不完整。

**建议：** 用 cargo-deny/cargo-about + `pnpm licenses` 生成锁定包许可证清单并提交入库；
CI 增加 license-check 门禁。

#### D3. 文档族落后当前版本，三处与实际行为矛盾

**证据：**

- `KNOWN_ISSUES.md:1,3` 标题与正文停留在 v1.0.0（当前 1.1.2）；`:10` "计划在 V1.1 提供"
  跨平台，而实际 1.1.x 均为 Windows 补丁版，版本语义冲突；`:5` 披露分卷未提供，但 PRD
  §14.1 将分卷列为 V1.0 必须项——V1.0 带着 PRD 必须项缺失发布，1.1.2 仍未补齐。
- `PRIVACY.md:7` "仅在配置签名更新服务后才提供手动检查更新"与实际矛盾：
  `updates.rs:7-9` 无条件提供手动 GitHub 检查（命令恒注册，unsigned-degraded 构建亦可用）。
  隐私核心承诺成立：遥测硬编码关闭（`platform-integration/src/lib.rs:165,227`）、全仓无
  遥测代码、唯一出站网络为 GitHub 公共 Releases API 且不携带用户数据；`:6` 对 7z 命令行
  密码可见性的披露与实现一致。
- `RC1_ACCEPTANCE.md:13-16` 称签名不可用会"终止工作流且不创建 Release"，与现行
  unsigned-degraded 降级继续发布的行为矛盾。
- `QZip_TODO_V2.1.md` "V1.0 剩余阻塞项"全部未勾选，但 1.0.0/1.1.2 早已发布。
- `DEPENDENCIES.md:21-23` 称 updater "disabled in the RC1 release build"，已过时；
  `@tauri-apps/plugin-updater` 仍是 desktop 依赖（`apps/desktop/package.json:20`）但前端
  从未 import（更新走自定义命令）。
- `docs/licensing/7zip-sidecar.md:1-4` 仍是 M0 模板文字（"M1 must populate…"）；PRD §38
  要求的 `third_party/7zip/` 合规文件布局（LICENSE.txt/VERSION/SOURCE_INFO.md）与实际不符
  （实际信息集中在 manifest.json，内容齐全但路径不符）。

**建议：** 以 1.1.2 为基线批量更新上述文档；明确 1.1.x 版本语义；分卷能力补齐或在 PRD 中
显式降级；评估移除未使用的 npm plugin-updater。

---

## 低严重度（汇总）

前端/桌面端：

1. `list_archive_entries` `limit=0` 翻页死循环 → `limit.unwrap_or(500).clamp(1, 500)`
   （`lib.rs:467-477`；前端恒用默认值，仅异常调用方受影响）。
2. `ExtractPage`/`BatchExtractPage` 无防重入（`ArchivePages.tsx:365-386、458-488`），
   同帧双击提交两个解压任务；CreatePage 有 `submittingRef` 防护（`:78,169,182-202`），
   两处未对齐。
3. `get_integration_status`（`lib.rs:600-642`）同步命令内拉起 powershell.exe + reg.exe，
   每次打开设置页阻塞 IPC 线程 0.5–2s → 改 async + TTL 30–60s 缓存。
4. `check_updates_on_startup` 是持久化死设置（默认 false，无消费方，
   `SettingsPage.tsx:196` 开关已 disabled）→ 实现或移除。
5. 更新检查把 pre-release 当稳定版（`updates.rs:46` 剥离 `-` 后缀）；`/releases/latest`
   本身排除 prerelease，影响有限 → 解析时识别并跳过 pre-release。
6. shell-request 通道可被同用户进程伪造（`shell_integration.rs:35-101`，设计内自动化面；
   已有 UUID/4MB/存在性/15s 窗口四重校验，无提权）→ 可选收紧目录 ACL。
7. `TaskCenter.tsx:170-171` open/reveal 无错误处理与 isTauri 守卫；
   `settingsClient.ts:21` `openDefaultApps` 同样缺 isTauri 守卫（浏览器预览模式裸 TypeError）。
8. `reveal_in_file_manager`（`preview.rs:225-237`）无 `#[cfg(windows)]` 平台门（可移植性
   瑕疵，产品为 Windows-only）。
9. 解压双列表：runtime 风险预检与 backend 解压前检查各 list 一次大档案（性能，可合并）。
10. `map_exit` 把 "cannot open" 归为 CorruptArchive（密码/路径错误误分类，信息性问题）。
11. finalize 清理失败时可能残留 `.qzip-overwrite-*` 备份与空父目录（原文件安全优先的设计
    取舍，journal 回滚覆盖主要路径）。
12. cancel 与 complete 竞态可能把已完成操作报为 Cancelled（低概率，任务快照有最终校正）。
13. `list_tar_wrapper_streaming` 理论死锁边缘：若 TAR lister 提前退出 0，extractor 阻塞在
    完整管道（`archive-sevenzip/src/lib.rs:376` 区域；实践中 7z `l` 不会提前退出）。
14. `App.test.tsx` demo 数据与 `BrowserPage` demo 条目仅浏览器模式可见（无安全问题）。

CLI：

15. CLI extract 绕过 `assess_entries` 风险门与 staging 原子提交（`cli/main.rs:201-228`）；
    后端 `safe_relative_path` 底线仍在。开发者工具 → 补 `--accept-risk` 或对齐预检。

脚本/发布：

16. RFC3161 时间戳走 HTTP（`bundle-windows.ps1:58,75`、`build-windows-shell-integration.ps1:99`）
    → 改 HTTPS TSA。
17. signtool `/p $env:...` 命令行传签名密码（进程列表短暂可见，行业常见）。
18. `configure-github-signing.ps1:29` SecureString→明文管道给 `gh secret set`（标准做法）；
    `:24-27` `-Repository` 无 ValidatePattern（极端输入 flag 注入面）。
19. 硬编码工具路径：Windows Kits 仅 x86 搜索（`bundle-windows.ps1:55`）、VS2022 目录与
    `-G 'Visual Studio 17 2022'`（`build-windows-shell-integration.ps1:45,48,93-95`）、
    `C:\Windows\System32\tar.exe` 与 Bandizip 路径（`generate-local-compat-fixtures.ps1:10`）
    → vswhere/where.exe 探测。
20. `@PUBLISHER@` 替换无 XML 转义（`build-windows-shell-integration.ps1:121-122`，publisher
    含 `&`/`<` 时 makeappx 失败）。
21. 发布方校验为子串匹配（`verify-release-assets.ps1:69`、`bundle-windows.ps1:60-63/77-80`）
    → 精确相等/词边界。
22. 验证器自依赖仓库 7z.exe 解析安装包载荷（`verify-release-assets.ps1:76-78`，与 R7 信任根
    形成循环依赖）。
23. NSIS `nsExec` 调用注册/注销脚本后不检查退出码（`qzip-shell-hooks.nsh:416-418、426-428`）。
24. `Register-QZipShell.ps1:14-15` 无条件先 Remove 后 Add，Add 失败则注册丢失直到应用重试。
25. `release.yml:25` 并发组含未净化的 `client_payload.branch`（含换行即中止运行）；tag 存在
    检查在 build 与 publish 之间存在 TOCTOU（竞态窗口小）。
26. `generate-local-compat-fixtures.ps1:39` 每次重跑向兼容 manifest 追加重复 bsdtar 用例
    （数据污染）。
27. 发布方占位与 AppxManifest 静态版本 `1.0.0.5`（`AppxManifest.xml.in`）与构建版本脱钩
    （cosmetic）。
28. 卸载不清理 `%LOCALAPPDATA%\QZip\ShellRequests` 与 Logs（`Unregister-QZipShell.ps1`、
    `qzip-shell-hooks.nsh:423-450`）。

C++ shell 扩展：

29. `CMakeLists.txt:1-9` 无 /W4//WX；依赖 SDK 头 pragma 隐式链接 appmodel/shobjidl。
30. `qzip_shell.cpp:51-55` `JsonEscape` 丢弃 <0x20 控制字符而非 \u 转义（文件名含换行被
    静默截断，消费端 `.exists()` 过滤兜底）；`:104` 开发回退路径受 MAX_PATH 截断；
    `:110-112` CreateProcessW 手工引号不转义内嵌引号（路径可信，良性）；`:132`
    `DllCanUnloadNow`=S_FALSE，DLL 永不卸载，更新需重启 explorer。

测试/文档杂项：

31. `package.json:32-33` release 脚本硬编码 `-Version v1.1.2`；`SettingsPage.tsx:195/205`
    硬编码 "1.1.2" 作版本 fallback → 读 manifest。
32. `packages/ui` 测试孤儿：`components.test.tsx` 4 个用例存在，但根 `test` 脚本
    （`package.json:18`）只跑 desktop vitest，CI 从不执行 ui 测试 → ui 包加 `vitest run`
    并纳入根脚本。
33. 关键前端页面缺直接测试：BrowserPage/SettingsPage/TaskCenter 无测试文件；拖放路径分类
    （V1.0 核心交互）无前端测试。
34. `RC2_ACCEPTANCE.md:32` "15 项前端测试"与当前 25 个用例不符（2026-07-30 历史快照，
    建议标注基线日期）。
35. `tests/e2e/` 仅 2 张 PNG 无 e2e 代码；`tests/security/`、`tests/compatibility/` 为空目录。
36. 兼容矩阵 9 个用例中 4 个 blocked（WinRAR/PeaZip/Keka/GNU tar），RC1 验收已声明"不得
    对外宣称兼容认证"→ README 兼容性口径注明已验证比例。
37. GitHub 匿名 API 限速（60 req/h）可能使频繁更新检查误报失败（`updates.rs`）→ 错误提示
    区分限速或加本地缓存。
38. `design-qa.md:2` 引用本机绝对路径；PRD 内部编号错误（§38 下小标题误写 37.1/37.2）。
39. `README` 声称 ISO/CAB/WIM 查看与解压（PRD 可选项）但无对应测试支撑。

---

## 既往审计（PROJECT_AUDIT.md，基线 037d0b6）10 项核验

| # | 问题 | 状态 | 核验证据 |
|---|------|------|----------|
| 1 | 清单截断/UTF-8 panic | ✅ 已修复 | Listing 流式解析 + `append_bounded` 字符边界截断（`archive-sevenzip/src/lib.rs`）；1,500 条目单测 + 1,250 中文文件真实回归（`206d326`） |
| 2 | 覆盖解压数据丢失 | ✅ 已修复 | staging+commit+journal 回滚；备份/替换/恢复原文件（`task-runtime/src/staging.rs:155`）；`never_overwrites`/回滚/锁定文件三组测试（`c495c53`） |
| 3 | TAR.GZ 浏览临时目录 | ⚠️ 部分修复 | 浏览已流式（双 7z 进程管道，`archive-sevenzip/src/lib.rs:376`，`4089bda`）；**解压/测试仍用 `expand_tar_wrapper` 完整展开内层 TAR 到 %TEMP%**（见 S8） |
| 4 | 事件通道 Lagged | ✅ 已修复 | `Lagged`→重订阅→`task.resync`（`lib.rs:795-812`），前端不重复 Toast（`d502190`）；接收器溢出后可用回归测试 |
| 5 | 签名环境变量不一致 | ❌ 未修复 | 即本报告 H1（独立复核 + 子代理交叉确认，行号见 H1 证据） |
| 6 | tgz/txz 复合别名 | ✅ 已修复 | format 枚举 tgz→TarGz、txz→TarXz（`archive-core/src/lib.rs`），NSIS 关联与 CLI ValueEnum 全覆盖（`04beafa`） |
| 7 | 更新检查缺失 | ✅ 已实现 | GitHub API 固定 URL + 10s 超时 + rustls + 严格 semver 解析（`updates.rs`，4 个单测，`6e59825`）；见 L5/L11 残留小项 |
| 8 | 会话泄漏 | ✅ 已修复 | `prepareArchive` 成功即关旧会话、全部导航出口 `closeActiveSession`、批量解压 try/finally（`App.tsx`、`ArchivePages.tsx:483-485`）；`App.test.tsx:63` 回归（`25b8d10`） |
| 9 | 历史并发 | ✅ 已修复 | 写锁 + UUID 临时名 + sync_all + 启动恢复（`task-runtime/src/history.rs`）；并发写有效快照测试（`9bd5a83`） |
| 10 | 模块拆分 | ✅ 已修复 | 6d2b94a 后文件规模在限内（ArchivePages.tsx 518 行、desktop lib.rs 867 行、task-runtime lib.rs 737 行） |

---

## 正面确认（供结论引用）

- **路径安全底座**：`safe_relative_path`（拒绝绝对/UNC/盘符/`..`/保留名/尾随空格点）+
  `output_path` 词法包含性 + 7z 后端解压逐 entry 二次校验（zip-slip 双重防护）。
- **发布 provenance 链**：SemVer 校验、tag==v+version、branch==默认分支、
  HEAD==RELEASE_SHA==GITHUB_SHA、远端分支 HEAD 复核、tag 不存在、bump-version CheckOnly +
  6 文件提交白名单（`release.yml:154-180`）。
- **能力最小化**：`capabilities/default.json` 仅 core:default + 少量 window/notification 权限；
  未注册 shell/fs/dialog/opener 插件；`tauri-plugin-updater` 为 optional feature 默认关闭。
- **更新检查安全**：固定 GitHub API URL、10s 超时、rustls+ring、无重定向跟随、严格数值化
  x.y.z 比较、403/429 友好处理。
- **无 XSS 汇聚点**：前端全库无 `dangerouslySetInnerHTML`/`innerHTML`/`eval`/`new Function`。
- **密码处理**：CLI 仅 stdin/prompt；桌面端 `SecretString`；任务历史/设置持久化不含密码
  （有测试锁定）。
- **遥测**：硬编码关闭且无采集路径（`platform-integration/src/lib.rs:165,227`）。
- **测试质量**：Rust 58 个单测（task-runtime 事务/并发/回滚语义扎实）、前端 25 个用例、
  核心回归覆盖真实 7-Zip 创建/解压/错误密码/取消/冲突。
- **版本一致性**：1.1.2 在 6 处清单完全一致；`bump-version.ps1` 恰好一处匹配正则编辑 +
  `cargo metadata --locked` 核对 + release.yml 严格校验。
- **Sidecar 合规**：manifest 记录 26.02/LGPL/未修改，构建与运行前双重 SHA-256 校验。

---

## 建议实施顺序

1. **立即（1 行改动）**：H1 签名变量断链修复 + trusted 路径 CI 护栏（与 R3 联动）。
2. **本迭代**：R1/R2（发布验证链两处断点）、R5/R4/R6（验收与 CI 回归面）、
   S1（CSP）、S3/S4（预览无扩展名执行 + 准备阶段条目上限）、S2（路径来源收敛）。
3. **下一迭代**：R7/R8（供应链信任根与开发证书校验）、S7/S8/S9（sidecar 校验缓存、
   TAR 解压流式化、中间文件空间预检）、S10（风险矩阵测试）、D2（许可证清单 + CI 门禁）。
4. **例行**：D1/D3（CHANGELOG 与文档族批量更新）、低危 39 项按模块归属并入各迭代。
