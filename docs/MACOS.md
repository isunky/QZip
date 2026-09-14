# macOS 测试版

当前为移植实现，尚未通过 Mac 实机验收，不能视为已正式支持。目标为 macOS 12+，Apple Silicon 和 Intel 各自原生构建。最低系统版本仍需实机验证。

## 开发与构建

在 Mac 安装 Xcode Command Line Tools、Node.js 24、Corepack 和 Rust 1.96.0，然后在仓库根目录运行：

```sh
corepack enable
corepack pnpm install --frozen-lockfile
corepack pnpm macos:fetch
corepack pnpm macos:check
corepack pnpm macos:dev
# 或生成 .app 和 DMG
corepack pnpm macos:build
```

引擎来自 7-Zip 官方 26.02 macOS 下载，下载包固定 SHA256；验证后对内置 universal 7zz 做 ad-hoc 签名，将签名后的摘要嵌入 Rust 主程序。应用每次调用仍检查引擎完整性。不要在生成摘要后重签引擎；构建脚本会校验最终 .app 内的引擎摘要。无需用户安装 Homebrew 或 Rosetta。

`macos:fetch` 将引擎、许可证、图标和环境摘要放在被 Git 忽略的 `third_party/7zip/bin/macos`，临时下载路径会打印，便于排查。更改引擎版本必须重新验证官方下载包并更新清单。

## 发布与安装

`macOS test packages` 工作流仍可在 PR/main/手动触发时分别使用 ARM 和 Intel runner 验证；`Build and release QZip` 正式发布 action 现在默认并行构建这两个 macOS 架构，并在 Windows 构建通过后将 DMG 一起发布到同一个 GitHub Release。

两架构产物为 `QZip-v<版本>-macos-arm64.dmg`、`QZip-v<版本>-macos-x64.dmg` 及独立 SHA256 清单。下载 Actions artifacts 后先完成下列验收，再将两架构产物一起上传 GitHub 测试发布；不得把单架构通过称为双架构支持。上传时保留产物和校验清单原始文件名。正式版本 tag 与包名保持一致，更新检查才能匹配架构。

测试包只有 ad-hoc 签名，没有 Developer ID 签名或公证。浏览器下载后可能被 Gatekeeper 拦截。仅在确认来源和校验值后使用系统“隐私与安全性”中的单应用允许入口；不要求关闭 Gatekeeper，也不承诺所有系统都允许打开。正式分发前需另行配置 Developer ID、嵌套引擎签名及公证并重新验收。

将 QZip 拖入 Applications 后启动。Finder 的“显示简介 → 打开方式 → QZip → 全部更改”可设置默认打开应用；应用仅声明关联，不抢占默认应用。当前不包含 Finder 右键扩展。

关闭窗口不结束任务，Dock/文件打开可恢复窗口；⌘Q 在活动任务存在时请求确认并等待取消清理。菜单支持 ⌘O、⌘, 以及系统常规编辑操作。多压缩包打开进入批量解压页。

更新检查仍仅查询 GitHub 稳定版，保留更新说明弹窗；macOS 匹配对应架构 DMG 后通过浏览器下载，未提供匹配资产时进入发布页。不支持应用内替换安装，也不自动订阅测试版。

## 发布前验收清单（目前均待 Mac 验证）

- [ ] ARM 与 Intel CI：lint、类型检查、前端测试、Rust fmt/clippy/test、DMG 构建及包内引擎校验。
- [ ] 两架构实机：从浏览器下载 DMG，安装 Applications，带隔离属性启动及 Finder 双击冷/热启动。
- [ ] 最低 macOS 12 与当前系统：原生按钮、⌘O/⌘,/⌘W/⌘Q、关闭后 Dock 重开、多文件打开、拖放。
- [ ] ZIP/7Z/TAR/TAR.GZ/TGZ/TAR.XZ/TXZ 的创建、浏览、测试和解压；RAR 只读、密码错误与正确密码。
- [ ] 中文、空格、emoji、Unicode 组合字符、大小写敏感卷、长路径、大目录、只读目录、符号链接风险。
- [ ] 覆盖失败保留原文件；取消/退出等待清理；任务事件、历史和页面切换会话释放无回归。
- [ ] 更新弹窗 Release Notes、匹配架构下载链接、缺失资产回退，无 Windows 安装按钮。
- [ ] Windows Explorer 冷/热启动、NSIS 构建、压缩解压、任务中心和更新安装无回归。

Runner 架构选择依据：[GitHub 官方 runner 文档](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)。签名限制参见 [Tauri macOS 签名文档](https://v2.tauri.app/distribute/sign/macos/)。
