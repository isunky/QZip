# 轻压 · QZip 产品需求文档（Codex 开发版）

> 文档版本：V2.1  
> 文档用途：供 Codex 等 AI 编程工具直接拆解、实现、测试和验收  
> 产品类型：开源、Windows 优先、具备跨平台路线图的轻量化桌面压缩软件
> V1.0 目标平台：Windows 10/11 x64
> 默认技术栈：Tauri 2 + React + TypeScript + Rust + 7-Zip CLI Sidecar  
> 默认许可证建议：Apache-2.0  
> 文档状态：开发基线  
> 正式产品名称：中文名“轻压”，英文品牌“QZip”，统一品牌展示为“轻压 · QZip”

---

# 版本范围与需求摘要

本文件是 QZip 产品范围、功能需求、工程约束与验收标准的唯一权威来源。实施顺序和完成状态以 `QZip_TODO_V2.1.md` 为准；若两者冲突，先修正文档，不得由实现者自行放宽安全或验收要求。

## 发布阶段

| 阶段 | 平台与目标 | 完成口径 |
|---|---|---|
| `v1.0.0-rc.1` | Windows x64 公开预览 | 安全写入、安装和发布链路可验证；允许存在已公开记录的功能缺口 |
| RC2 | Windows x64 功能收口 | 补齐拖拽、创建选项、部分解压、浏览与任务中心等 V1.0 缺口 |
| V1.0 | Windows x64 稳定版 | 本文规定的 Windows 功能、体验、安全和发布验收全部通过 |
| V1.1 | 跨平台扩展 | 增加 macOS、Linux 的发行、签名、系统集成和平台验收 |

RC1 的已知问题不得被描述为已经完成；RC1 可以发布带说明的功能缺口，但 V1.0 稳定版不得以已知问题代替必需功能。

## V1.0 产品目标

QZip 是本地优先、轻量化的桌面压缩软件。用户应能通过拖拽和合理默认值，在 Windows 10/11 x64 上快速、安全地完成压缩、解压、压缩包浏览和任务管理。

V1.0 必须支持：

- 创建 7Z、ZIP、TAR、TAR.GZ、TAR.XZ；
- 读取、浏览和解压 7Z、ZIP、RAR、TAR、GZ、XZ、BZ2；
- 部分解压、密码压缩/解压、7Z 文件名加密、分卷和完整性测试；
- 文件/文件夹拖拽、任务进度、取消和失败重试；
- 浅色、暗夜、跟随系统和 6 套主题色；
- Windows 文件关联、Explorer 高频右键菜单、NSIS/MSI 安装和卸载；
- 受信任的 Authenticode 发布签名、校验和与合规分发的 7-Zip Sidecar；
- 可替换的 `ArchiveBackend` 接口，UI 不依赖具体后端。

V1.0 明确不包含：RAR 创建、修复压缩包、自解压 EXE、云服务、账号、广告、插件市场、完整文件管理器、远程挂载、跨重启任务恢复、移动端及企业授权服务器。

## 核心流程

| 场景 | 输入 | 必须结果 |
|---|---|---|
| 快速压缩 | 单个或多个普通文件/文件夹 | 进入创建页；多个对象生成一个压缩包 |
| 快速解压 | 单个压缩包 | 进入快速解压页，默认输出到同名目录 |
| 批量解压 | 多个压缩包 | 每个压缩包创建独立解压任务 |
| 混合拖拽 | 普通对象与压缩包混合 | 要求用户选择处理方式 |
| 浏览 | 双击或打开压缩包 | 进入压缩包浏览页 |
| 部分解压 | 浏览页选择条目 | 创建仅包含所选条目的解压任务 |
| 完整性测试 | 选择测试操作 | 创建测试任务并显示结构化结果 |
| 密码错误 | 失败任务重试 | 保留非敏感参数并要求重新输入密码 |

高频任务的体验标准：首次打开即可理解；普通压缩最多两次点击开始；普通解压一次确认；失败提示说明原因、下一步和是否可重试，并提供技术详情入口。

## 功能需求

### FR-01 首页与拖拽

- 首页显示拖拽入口、压缩文件、打开压缩包、任务和设置入口。
- 拖拽识别在后台执行，可取消且不得阻塞 UI；拖入后在当前窗口切换页面。
- 混合输入必须要求用户选择压缩普通对象、解压压缩包或分别处理。

### FR-02 创建压缩包

- 显示输入摘要、名称、保存位置、格式、压缩方式、密码、开始操作与更多设置。
- 默认输出位于源目录，使用 7Z 和 Balanced；根据单文件夹、单文件或多文件生成名称，重名自动追加序号，禁止直接覆盖。
- 更多设置包括分卷、文件名加密、Solid、压缩后测试、删除源文件和排除规则；专家参数按需展示。
- 格式切换后按后端能力禁用选项；创建立即返回任务 ID；删除源文件必须二次确认。

### FR-03 快速解压与安全

- 显示格式、压缩大小、预计解压大小、文件数量、输出位置、冲突策略、密码和查看内容入口。
- 默认输出到同名目录，冲突策略为 `Rename`，默认不创建链接；风险检查必须早于任何最终目录写入。
- 普通压缩包支持一键解压；密码错误可原地重试；不得静默覆盖。

### FR-04 压缩包浏览

- 提供返回、添加、解压、测试、更多、面包屑和搜索。
- 支持目录导航、多选、即时过滤、部分解压、完整性测试和属性查看。
- 大列表必须虚拟化并分批载入，加载和搜索不得阻塞界面。

### FR-05 任务中心

- 区分进行中、已完成和失败任务；进度必须来自真实后端事件。
- 完成任务支持打开结果、打开位置和再次执行；失败任务提供原因、修复操作、重试和技术详情。
- 重试保留非敏感业务参数，不得保留密码、失效临时路径或原进程 ID。

### FR-06 设置与本地存储

- 设置覆盖常规、压缩、解压、文件关联、外观、隐私安全、更新和关于。
- 持久化主题、缩放、默认格式、压缩预设、冲突策略和解压偏好；不得保存密码、完整命令或未脱敏诊断数据。
- 任务历史默认不保存压缩包内部清单。

### FR-07 Windows 系统集成与发行

- 文件关联覆盖 7Z、ZIP、RAR、TAR、GZ、XZ、BZ2；双击复用单实例。
- Explorer 右键菜单提供打开、压缩为 7Z/ZIP、解压到当前目录/同名目录和更多选项。
- 支持 Windows 长路径、保留名称及 100%～200% DPI。
- 发布物包含 NSIS/MSI、便携包、校验和、第三方许可、Sidecar 版本和来源信息，并通过受信任 Authenticode 签名。

## V1.0 验收摘要

以下四类验收必须同时成立：

- 功能：Windows 上可完成创建、解压、浏览、部分解压、密码、文件名加密、分卷、取消、测试、拖拽、关联和右键菜单操作；
- 体验：高频流程满足点击次数要求，主题完整、键盘可用，`760×520` 最小窗口不溢出；
- 安全：路径、链接、压缩炸弹、覆盖、取消清理和密码隐私要求全部通过；
- 发布：Windows 安装、升级和卸载可验证，签名、校验和、许可、隐私、卸载、已知问题和 GitHub Release 齐备。

## V1.1 跨平台路线图

V1.1 在不改变 V1.0 领域模型和 `ArchiveBackend` 契约的前提下增加：

- macOS：Finder Quick Action、Command 快捷键、Apple Silicon/Intel、系统窗口控制、签名、公证和 Gatekeeper 验收；
- Linux：MIME、Desktop Entry、AppImage、Wayland/X11 验收，并避免依赖不稳定的透明效果；
- 对应平台的 Sidecar、路径、文件关联、安装、卸载和发布流水线。

V1.1 不属于 Windows V1.0 或 `v1.0.0-rc.1` 的发布阻塞项。

---

# 0. Codex 执行说明

本节是 Codex 实施时的最高优先级约束。除非开发者明确修改本文件，否则不得自行偏离。

## 0.1 规范关键词

本文使用以下关键词：

- **必须（MUST）**：不可省略，不满足即视为功能未完成。
- **应当（SHOULD）**：原则上必须满足，存在明确技术原因时可以偏离，但需要记录原因。
- **可以（MAY）**：可选能力，不影响当前版本验收。
- **不得（MUST NOT）**：明确禁止。
- **首版**：指 Windows x64 V1.0 正式版范围。
- **当前迭代**：指开发任务中明确指定的里程碑范围。

## 0.2 文档优先级

实现冲突时按以下顺序判断：

1. 安全与隐私约束；
2. 数据模型和接口契约；
3. 当前里程碑验收标准；
4. 页面功能需求；
5. UI 设计规范；
6. 性能目标；
7. 可选优化建议。

## 0.3 Codex 实施原则

Codex 必须：

1. 先阅读本 PRD，再修改代码；
2. 按里程碑实施，不得一次性实现所有候选功能；
3. 将 UI、业务服务和压缩后端保持解耦；
4. 不在 React 组件中拼接 7-Zip 命令；
5. 不通过 Shell 字符串执行外部命令；
6. 不在日志、配置、事件或错误报告中暴露密码；
7. 为新增核心逻辑补充测试；
8. 修改数据结构时同步更新 Rust、TypeScript 类型和文档；
9. 在无法确认需求时采用本文件中最保守、最安全的默认值；
10. 不为了“看起来完成”而伪造压缩结果、进度或平台集成。

Codex 不得：

- 擅自增加云服务、账号系统、广告或遥测；
- 使用 Electron 替代 Tauri；
- 将 7-Zip GUI 程序直接嵌入界面；
- 将项目做成通用文件管理器；
- 首版实现 RAR 创建；
- 在默认界面暴露大量算法参数；
- 为低频功能引入大型运行时依赖；
- 使用无法追踪许可证来源的二进制文件；
- 省略危险路径和压缩炸弹检查。

## 0.4 完成定义

一项功能仅在同时满足以下条件时视为完成：

- 功能行为符合本 PRD；
- UI 包含加载、空、成功、失败和禁用状态；
- 核心逻辑有测试；
- 错误提示可理解且可操作；
- 浅色和暗夜模式可用；
- 至少默认薄荷绿主题可用；
- 不产生明显控制台错误；
- 不泄露密码或完整敏感路径；
- 通过当前里程碑验收项。

---

# 1. 产品概述

## 1.1 产品背景

现有压缩软件通常存在以下问题：

- 功能强，但界面和交互停留在传统工具软件阶段；
- 高级参数直接暴露，普通用户难以理解；
- 压缩、解压和进度窗口相互割裂；
- 不同桌面平台的产品语言容易失去一致性；
- 免费版本常带有广告或功能限制；
- 部分现代化产品只改善视觉，没有重构操作流程。

轻压 · QZip 采用成熟的 7-Zip 控制台能力作为首个压缩后端，通过独立的后端适配层、任务系统和现代桌面界面，形成一款强调轻量、简洁、可靠并可持续扩展的平台化开源压缩工具。

## 1.2 产品定位

> 轻压 · QZip 是一款以拖拽、智能默认配置和现代任务流为核心的 Windows 优先压缩软件。

产品价值不在于重新发明压缩算法，而在于：

```text
成熟压缩能力
+ 解耦的后端架构
+ 简洁的交互流程
+ 现代化 UI
+ 安全可靠的任务系统
+ 可持续的开源商业模式
```

## 1.3 产品愿景

让用户无需理解 LZMA2、字典大小、Solid Block 等专业概念，也能快速、安全地完成压缩、解压和压缩包内容管理。

## 1.4 产品口号

> 压缩和解压，本来就应该简单。

## 1.5 品牌名称释义

正式名称采用：

```text
中文名称：轻压
英文品牌：QZip
统一展示：轻压 · QZip
```

名称含义：

- `Q` 取自中文“轻”的拼音 `Qing`，表达轻量、快捷和低负担；
- `Zip` 表达压缩、解压和归档能力，不代表产品仅支持 ZIP 格式；
- 中文界面可优先展示“轻压”，国际界面和发行物优先展示“QZip”；
- 对外正式材料、官网标题和应用关于页面统一使用“轻压 · QZip”。

## 1.6 工程命名规范

Codex 和开发人员必须使用以下工程标识，不得继续使用早期临时工程名。

| 对象 | 固定名称 |
|---|---|
| 统一品牌 | `轻压 · QZip` |
| 中文简称 | `轻压` |
| 英文品牌和应用显示名 | `QZip` |
| GitHub 仓库建议名 | `qzip-desktop` |
| 仓库根目录 | `qzip-desktop/` |
| Rust Workspace 名称 | `qzip` |
| 前端包作用域 | `@qzip/*` |
| Tauri Product Name | `QZip` |
| Bundle Identifier | `app.qzip.desktop` |
| 配置目录名 | `qzip` |
| 环境变量前缀 | `QZIP_` |
| CSS Design Token 前缀 | `--qzip-` |
| 日志目标前缀 | `qzip` |
| CLI 命令 | `qzip-cli` |

命名约束：

1. 不将 CLI 命名为单独的 `qzip`，避免与现有命令行生态中的同名工具产生冲突；
2. 用户可见界面不展示 `qzip-desktop`、`qzip-cli` 等工程名称；
3. 文件关联、安装包、系统通知和任务栏显示名统一使用 `QZip`；
4. 中文官网、中文商店介绍和中文文档标题使用“轻压 · QZip”；
5. 英文环境可以仅显示 `QZip`；
6. 代码类型名使用 `QZip` 仅限应用级命名，通用领域类型继续使用 `Archive`，例如 `ArchiveBackend`；
7. 不因品牌名称包含 `Zip` 而限制 7z、RAR、TAR 等格式能力；
8. 若未来商标审查要求调整名称，只允许通过集中配置替换用户可见品牌，不修改领域模型和后端接口。

## 1.7 核心设计原则

1. **默认即合理**：首次使用不需要配置即可完成高频任务。
2. **复杂度渐进展示**：高级选项按需展开。
3. **界面与后端解耦**：前端只处理结构化业务数据。
4. **本地优先**：默认不联网、不上传文件。
5. **安全优先**：路径、容量、链接、覆盖和密码均有保护。
6. **可移植架构**：V1.0 保持平台边界清晰，为 V1.1 平台适配预留接口。
7. **轻量克制**：不增加无明确高频价值的功能。

---

# 2. 产品目标与非目标

## 2.1 V1.0 产品目标

V1.0 面向 Windows 10/11 x64，必须实现：

- 创建 7z、ZIP、TAR、TAR.GZ、TAR.XZ；
- 解压 7z、ZIP、RAR、TAR、GZ、XZ、BZ2；
- 浏览压缩包目录和文件；
- 选择部分文件解压；
- 文件和文件夹拖拽；
- 密码压缩、密码解压；
- 7z 文件名加密；
- 分卷压缩；
- 压缩包完整性测试；
- 任务进度、取消、失败重试；
- 文件关联；
- 高频右键菜单；
- 浅色、暗夜、跟随系统；
- 至少 6 套主题色；
- Windows 10/11 x64 基础发行、安装、卸载与系统集成；
- 7-Zip Sidecar 合规分发；
- 独立 `ArchiveBackend` 后端接口。

## 2.2 V1.0 非目标

V1.0 不实现：

- RAR 创建；
- 压缩包修复；
- 自解压 EXE；
- 云盘同步；
- 在线分享；
- 账号登录；
- 广告系统；
- 插件市场；
- 完整文件管理器；
- 压缩包内直接编辑文档；
- 远程挂载；
- 跨重启任务恢复；
- 高级文件预览；
- 移动端；
- 企业授权服务器。

---

# 3. 目标用户与使用场景

## 3.1 普通桌面用户

需求：

- 将文件夹压缩后发送；
- 解压收到的文件；
- 不理解专业压缩参数；
- 希望软件无广告、无捆绑、操作简单。

成功标准：

- 第一次打开即可理解；
- 拖入文件后两步内开始压缩；
- 拖入压缩包后一次确认即可解压；
- 失败时知道原因和解决方法。

## 3.2 开发者和技术人员

需求：

- 处理 7z、ZIP、TAR.GZ、TAR.XZ；
- 查看压缩包目录；
- 测试完整性；
- 使用分卷、排除规则和高级参数；
- 未来可使用 CLI。

## 3.3 企业办公用户

需求：

- 统一压缩格式和默认策略；
- 密码压缩；
- 右键快速操作；
- 静默安装；
- 内网离线部署；
- 无后台常驻、无广告。

## 3.4 核心场景

| 场景 | 用户动作 | 默认结果 |
|---|---|---|
| 快速压缩文件夹 | 拖入普通文件夹 | 进入创建压缩包页面 |
| 快速压缩多个文件 | 拖入多个普通文件 | 创建一个压缩包 |
| 快速解压 | 拖入单个压缩包 | 进入快速解压页面 |
| 批量解压 | 拖入多个压缩包 | 创建多个解压任务 |
| 浏览压缩包 | 双击或点击打开 | 进入压缩包浏览页面 |
| 部分解压 | 浏览页选择文件后点击解压 | 创建选中项解压任务 |
| 完整性测试 | 点击测试 | 创建测试任务 |
| 密码错误重试 | 失败任务点击重新输入 | 保留原参数重新执行 |

---

# 4. 版本与商业分层

## 4.1 Community 社区版

必须保持完整可用，包含：

- 常用格式压缩和解压；
- 7z 和 ZIP 创建；
- RAR 解压；
- 压缩包浏览；
- 密码功能；
- 文件关联；
- 基础右键菜单；
- 任务中心；
- 主题和语言；
- 安全更新。

## 4.2 Supporter 支持版

可通过官方商店一次性付费，主要提供：

- 官方商店安装；
- 正式签名；
- 稳定更新通道；
- 支持者标识；
- 提前体验版本。

不得删除社区版的核心压缩能力以推动付费。

## 4.3 Pro 效率模块

后续可选：

- 批量格式转换；
- 压缩预设；
- 目录监听；
- 定时归档；
- 高级命名规则；
- 高级排除规则；
- 任务队列策略；
- CLI 增强；
- 完成后动作。

Pro 应独立于核心模块，避免污染社区代码。

## 4.4 Enterprise

可收费服务：

- MSI、MSIX、PKG、DEB、RPM；
- 静默安装；
- 内网更新；
- 企业默认策略；
- 禁止特定格式；
- 品牌定制；
- 国产系统适配；
- 定制开发和技术支持。

---

# 5. 技术栈与工程约束

## 5.1 固定技术栈

工程基础标识必须使用：

```text
productName = "QZip"
identifier = "app.qzip.desktop"
repository = "qzip-desktop"
CLI = "qzip-cli"
```

### 桌面框架

- Tauri 2；
- Rust stable；
- Tokio 异步运行时。

### 前端

- React；
- TypeScript，开启 `strict`；
- Vite；
- Zustand；
- CSS Variables；
- Lucide Icons；
- 不引入重量级 UI 框架。

### 后端

- Rust；
- `serde`；
- `thiserror`；
- `tokio`；
- `tokio-util` 的取消令牌；
- 7-Zip CLI Sidecar；
- Tauri Command 和 Channel/Event。

### 测试

- Rust 单元测试和集成测试；
- Vitest；
- React Testing Library；
- Playwright 或等价桌面 UI 测试方案可在 V0.5 引入。

## 5.2 前端依赖原则

允许：

- 小型、职责单一、维护活跃的库；
- 能显著降低无障碍或平台适配风险的库。

禁止：

- 为少量按钮引入完整企业 UI 框架；
- 依赖在线 CDN；
- 在生产构建中引入远程字体；
- 使用不支持树摇优化的大型图标包。

## 5.3 Rust 依赖原则

- 每个依赖必须有明确用途；
- 引入前检查许可证；
- 核心模块不依赖 Tauri；
- `archive-core` 不依赖具体 7-Zip 实现；
- 平台集成与压缩逻辑分离。

---

# 6. 总体架构

## 6.1 分层结构

```text
React UI
  ↓ 结构化请求 / 状态事件
Tauri Adapter
  ↓
Application Service
  ├─ Task Manager
  ├─ Security Service
  ├─ Settings Service
  ├─ Temporary File Service
  └─ Backend Registry
        ↓
ArchiveBackend Trait
        ↓
SevenZipCliBackend
        ↓
7zz / 7z.exe
```

## 6.2 依赖方向

必须满足：

```text
UI → Tauri Adapter → Application Service → Domain/Core
                                          ↓
                                  Archive Backend Adapter
```

禁止：

```text
React Component → 直接调用 7zz
React Component → 解析 stdout
archive-core → 依赖 Tauri
archive-core → 依赖 React 数据结构
```

## 6.3 模块职责

### `archive-core`

负责：

- 领域类型；
- 格式枚举；
- 请求和结果模型；
- 后端 Trait；
- 统一错误；
- 任务状态；
- 能力描述。

不得负责：

- Tauri Command；
- 具体 UI；
- 7-Zip 参数；
- 平台文件关联。

### `archive-sevenzip`

负责：

- Sidecar 路径；
- 能力探测；
- 参数映射；
- 进程启动；
- stdout、stderr 解析；
- 退出码映射；
- 取消进程；
- 版本适配。

### `archive-security`

负责：

- 路径规范化；
- 路径穿越检查；
- 绝对路径检查；
- 符号链接策略；
- 文件数量和容量限制；
- 压缩炸弹风险判断；
- 文件名兼容检查。

### `task-runtime`

负责：

- 任务创建；
- 队列；
- 运行状态；
- 取消；
- 进度广播；
- 重试参数；
- 任务历史。

### `platform-integration`

负责：

- 文件关联；
- 右键菜单；
- 系统通知；
- 打开文件或目录；
- 单实例；
- 平台路径处理。

### `apps/desktop`

负责：

- Tauri 入口；
- Command；
- Channel；
- 窗口；
- Sidecar 注册；
- 前端打包。

### `packages/ui`

负责：

- 设计 Token；
- 基础组件；
- 主题；
- 图标；
- 无业务状态组件。

---

# 7. 仓库结构

```text
qzip-desktop/
├─ apps/
│  └─ desktop/
│     ├─ src/
│     │  ├─ app/
│     │  ├─ pages/
│     │  ├─ features/
│     │  ├─ components/
│     │  ├─ stores/
│     │  ├─ services/
│     │  ├─ types/
│     │  └─ styles/
│     ├─ src-tauri/
│     │  ├─ src/
│     │  │  ├─ commands/
│     │  │  ├─ app_state.rs
│     │  │  ├─ events.rs
│     │  │  └─ lib.rs
│     │  ├─ binaries/
│     │  └─ tauri.conf.json
│     └─ package.json
├─ crates/
│  ├─ archive-core/
│  ├─ archive-sevenzip/
│  ├─ archive-security/
│  ├─ task-runtime/
│  └─ platform-integration/
├─ packages/
│  └─ ui/
│     ├─ src/components/
│     ├─ src/tokens/
│     ├─ src/themes/
│     └─ src/icons/
├─ tests/
│  ├─ fixtures/
│  ├─ compatibility/
│  ├─ security/
│  └─ e2e/
├─ third_party/
│  └─ 7zip/
├─ docs/
│  ├─ architecture/
│  ├─ design/
│  └─ licensing/
├─ scripts/
├─ .github/workflows/
├─ LICENSE
├─ NOTICE
├─ THIRD_PARTY_LICENSES.md
└─ README.md
```

---

# 8. 领域模型

以下模型是 Rust 和 TypeScript 的共享业务契约。字段改动必须同步。

## 8.1 格式枚举

```rust
pub enum ArchiveFormat {
    SevenZip,
    Zip,
    Tar,
    TarGzip,
    TarXz,
    TarZstd,
    Rar,
    Gzip,
    Xz,
    Bzip2,
    Iso,
    Cab,
    Wim,
    Unknown,
}
```

规则：

- `Rar` 在 V1.0 只读；
- `Unknown` 不得用于创建任务；
- UI 展示名称与枚举分离；
- 以后端能力决定是否启用。

## 8.2 压缩预设

```rust
pub enum CompressionProfile {
    Store,
    Fast,
    Balanced,
    Small,
    Maximum,
}
```

默认 UI 只展示：

- Fast；
- Balanced；
- Small。

Store 和 Maximum 放入更多设置或专家模式。

## 8.3 冲突策略

```rust
pub enum ConflictPolicy {
    Ask,
    Rename,
    Overwrite,
    Skip,
}
```

默认：`Rename`。

## 8.4 任务类型

```rust
pub enum ArchiveOperation {
    Create,
    Extract,
    List,
    Test,
    Update,
}
```

## 8.5 任务状态

```rust
pub enum TaskStatus {
    Queued,
    Scanning,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}
```

## 8.6 压缩请求

```rust
pub struct CreateArchiveRequest {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub format: ArchiveFormat,
    pub compression_profile: CompressionProfile,
    pub password: Option<SecretString>,
    pub encrypt_file_names: bool,
    pub volume_size: Option<u64>,
    pub solid_mode: Option<bool>,
    pub test_after_create: bool,
    pub exclude_patterns: Vec<String>,
    pub delete_sources_after_success: bool,
}
```

约束：

- `inputs` 不得为空；
- `output` 不得指向输入目录内部导致递归压缩；
- `encrypt_file_names` 仅在后端支持时启用；
- `delete_sources_after_success` 默认 false；
- 删除源文件必须二次确认；
- 密码不得序列化到任务历史。

## 8.7 解压请求

```rust
pub struct ExtractArchiveRequest {
    pub archive_path: PathBuf,
    pub output_dir: PathBuf,
    pub selected_entries: Option<Vec<String>>,
    pub password: Option<SecretString>,
    pub conflict_policy: ConflictPolicy,
    pub security_policy: ExtractionSecurityPolicy,
}
```

## 8.8 列表请求

```rust
pub struct ListArchiveRequest {
    pub archive_path: PathBuf,
    pub password: Option<SecretString>,
}
```

## 8.9 测试请求

```rust
pub struct TestArchiveRequest {
    pub archive_path: PathBuf,
    pub password: Option<SecretString>,
}
```

## 8.10 压缩包条目

```rust
pub struct ArchiveEntry {
    pub path: String,
    pub display_name: String,
    pub is_directory: bool,
    pub uncompressed_size: Option<u64>,
    pub compressed_size: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    pub crc: Option<String>,
    pub attributes: Option<String>,
    pub encrypted: bool,
}
```

`path` 使用压缩包内部标准 `/` 分隔符，不直接作为系统输出路径。

## 8.11 后端能力

```rust
pub struct BackendCapabilities {
    pub backend_id: String,
    pub backend_version: String,
    pub readable_formats: Vec<ArchiveFormat>,
    pub writable_formats: Vec<ArchiveFormat>,
    pub supports_password: bool,
    pub supports_header_encryption: bool,
    pub supports_multivolume: bool,
    pub supports_update: bool,
    pub supports_partial_extract: bool,
    pub supports_progress: bool,
}
```

---

# 9. ArchiveBackend 接口

```rust
#[async_trait]
pub trait ArchiveBackend: Send + Sync {
    fn id(&self) -> &'static str;

    async fn capabilities(
        &self,
    ) -> Result<BackendCapabilities, ArchiveError>;

    async fn list(
        &self,
        request: ListArchiveRequest,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError>;

    async fn create(
        &self,
        request: CreateArchiveRequest,
        reporter: ProgressReporter,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError>;

    async fn extract(
        &self,
        request: ExtractArchiveRequest,
        reporter: ProgressReporter,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError>;

    async fn test(
        &self,
        request: TestArchiveRequest,
        reporter: ProgressReporter,
        cancellation: CancellationToken,
    ) -> Result<TestResult, ArchiveError>;
}
```

## 9.1 后端注册

应用必须使用后端注册表：

```rust
pub trait BackendRegistry {
    fn default_backend(&self) -> Arc<dyn ArchiveBackend>;
    fn backend_for_read(&self, format: ArchiveFormat) -> Option<Arc<dyn ArchiveBackend>>;
    fn backend_for_write(&self, format: ArchiveFormat) -> Option<Arc<dyn ArchiveBackend>>;
}
```

V1.0 仅注册 `SevenZipCliBackend`，但 UI 和应用服务不得假设只有一个后端。

---

# 10. Tauri Command 与事件契约

前端不得通过任意命令字符串调用后端，只允许调用结构化 Command。

## 10.1 Command 列表

```text
get_backend_capabilities
detect_archive_format
scan_input_paths
create_archive_task
extract_archive_task
test_archive_task
list_archive_entries
cancel_task
retry_task
get_tasks
clear_completed_tasks
get_settings
update_settings
open_path
reveal_in_file_manager
```

## 10.2 创建任务返回值

```ts
interface TaskCreatedResponse {
  taskId: string;
  status: "queued" | "scanning";
}
```

## 10.3 任务事件

```ts
type TaskEvent =
  | { type: "task.created"; task: TaskSnapshot }
  | { type: "task.updated"; task: TaskSnapshot }
  | { type: "task.progress"; progress: TaskProgress }
  | { type: "task.completed"; task: TaskSnapshot }
  | { type: "task.failed"; task: TaskSnapshot; error: ArchiveErrorDto }
  | { type: "task.cancelled"; task: TaskSnapshot };
```

## 10.4 进度模型

```ts
interface TaskProgress {
  taskId: string;
  phase: "scanning" | "compressing" | "extracting" | "testing";
  percent?: number;
  currentEntry?: string;
  processedEntries?: number;
  totalEntries?: number;
  processedBytes?: number;
  totalBytes?: number;
  speedBytesPerSecond?: number;
  elapsedSeconds: number;
  estimatedRemainingSeconds?: number;
  outputBytes?: number;
  compressionRatio?: number;
}
```

规则：

- 不支持确定进度时 `percent` 为空；
- UI 显示不确定进度；
- 更新频率建议 100～250ms；
- 不得每读取一行 stdout 就触发 React 全局重渲染；
- 相同值不重复广播。

## 10.5 统一错误 DTO

```ts
interface ArchiveErrorDto {
  code:
    | "FILE_NOT_FOUND"
    | "PERMISSION_DENIED"
    | "FILE_IN_USE"
    | "DISK_FULL"
    | "WRONG_PASSWORD"
    | "CORRUPTED_ARCHIVE"
    | "UNSUPPORTED_FORMAT"
    | "BACKEND_UNAVAILABLE"
    | "UNSUPPORTED_OPTION"
    | "UNSAFE_PATH"
    | "ARCHIVE_BOMB_RISK"
    | "CANCELLED"
    | "INVALID_REQUEST"
    | "UNKNOWN";
  title: string;
  message: string;
  recoverable: boolean;
  suggestedAction?: string;
  technicalDetails?: string;
}
```

普通界面默认不展示 `technicalDetails`。

---

# 11. 任务状态机

## 11.1 状态转换

```text
Queued
  ↓
Scanning
  ↓
Running
  ├─→ Cancelling → Cancelled
  ├─→ Failed
  └─→ Completed
```

允许：

- `Failed → Queued`，通过重试；
- `Cancelled → Queued`，通过再次执行；
- `Completed → Queued`，通过再次执行。

禁止：

- `Completed → Running`；
- `Failed → Running`，必须先创建新的运行实例；
- 同一任务同时绑定两个后端进程。

## 11.2 取消行为

取消时必须：

1. 标记 `Cancelling`；
2. 停止读取并终止子进程；
3. 等待进程结束；
4. 清理未完成文件；
5. 标记 `Cancelled`；
6. 广播最终状态。

若无法删除未完成文件：

- 标记警告；
- 不得把任务标记为正常完成；
- 提供“打开所在位置”。

## 11.3 重试行为

重试必须保留：

- 输入路径；
- 输出路径；
- 格式；
- 压缩配置；
- 冲突策略；
- 选中条目。

不得保留：

- 密码；
- 已失效临时路径；
- 原进程 ID。

---

# 12. SevenZipCliBackend 设计

## 12.1 Sidecar 文件

建议路径：

```text
apps/desktop/src-tauri/binaries/
├─ 7zz-x86_64-pc-windows-msvc.exe
├─ 7zz-aarch64-pc-windows-msvc.exe
├─ 7zz-x86_64-apple-darwin
├─ 7zz-aarch64-apple-darwin
├─ 7zz-x86_64-unknown-linux-gnu
└─ 7zz-aarch64-unknown-linux-gnu
```

构建脚本必须验证目标平台二进制存在。

## 12.2 命令执行

必须：

```rust
Command::new(executable)
    .arg("a")
    .arg(output)
    .args(inputs);
```

不得：

```rust
Command::new("cmd")
    .arg("/C")
    .arg(format!("7zz a {} {}", output, input));
```

原因：

- 路径转义风险；
- 命令注入风险；
- 不同平台或工具版本的参数行为不一致；
- 密码和特殊字符处理不可靠。

## 12.3 参数映射

参数映射必须集中在 `archive-sevenzip`，不得散落在应用服务。

示例映射：

| 业务参数 | 7-Zip 含义 |
|---|---|
| SevenZip | `-t7z` |
| Zip | `-tzip` |
| Fast | 低压缩等级 |
| Balanced | 中等压缩等级 |
| Small | 较高压缩等级 |
| encrypt_file_names | 7z 文件名加密 |
| volume_size | 分卷参数 |
| test_after_create | 创建完成后执行测试 |

实际参数必须通过测试确认后写入版本适配模块。

## 12.4 输出解析

必须建立：

```text
SevenZipOutputParser
SevenZipListParser
SevenZipErrorMapper
SevenZipVersionAdapter
```

解析结果不得直接使用界面文案。

测试样本必须覆盖：

- 英文输出；
- 中文系统环境；
- 密码错误；
- 文件损坏；
- 磁盘空间不足；
- 成功创建；
- 成功解压；
- 列表输出；
- 分卷；
- 路径包含空格和 Unicode。

## 12.5 退出码

退出码映射必须集中管理，映射到 `ArchiveError`。

未知退出码：

- 使用 `UNKNOWN`；
- 保存有限技术详情；
- 不显示完整敏感命令。

## 12.6 密码传递

V1.0 可以使用 7-Zip CLI 支持的密码参数，但必须：

- 不记录完整命令；
- 日志中替换为 `-p******`；
- 不在任务快照中保存；
- 不在前端 LocalStorage 中保存；
- 任务结束后清理内存持有；
- 文档中记录命令行参数可见风险；
- 后续预留库级后端。

---

# 13. 安全与隐私

## 13.1 解压安全策略

```rust
pub struct ExtractionSecurityPolicy {
    pub allow_symlinks: bool,
    pub allow_hardlinks: bool,
    pub limits: ExtractionLimits,
}
```

默认：

```text
allow_symlinks = false
allow_hardlinks = false
```

## 13.2 安全限制

```rust
pub struct ExtractionLimits {
    pub max_entries: u64,
    pub max_total_size: u64,
    pub max_single_file_size: u64,
    pub max_directory_depth: usize,
    pub max_compression_ratio: u64,
}
```

建议默认值由应用配置提供，不在 UI 首页展示。

## 13.3 路径检查

每个条目输出前必须：

1. 拒绝绝对路径；
2. 拒绝 Windows 盘符；
3. 拒绝 UNC 路径；
4. 规范化 `.` 和 `..`；
5. 验证最终路径仍在输出目录；
6. 检查保留文件名；
7. 检查尾部空格和句点；
8. 根据策略拒绝链接。

## 13.4 压缩炸弹

触发条件包括：

- 条目数超过限制；
- 总大小超过限制；
- 单文件超过限制；
- 压缩比异常；
- 目录层级异常；
- 磁盘空间不足。

触发后：

- 暂停或终止；
- 显示具体风险；
- 提供取消；
- 专家用户可明确调整限制；
- 不允许静默忽略。

## 13.5 密码和日志

不得记录：

- 密码；
- 完整命令；
- 压缩包内部敏感文件清单；
- 未脱敏的崩溃参数。

日志中的路径：

- 普通日志使用文件显示名；
- 诊断日志可由用户主动导出；
- 导出前提示可能包含路径；
- 密码始终不导出。

## 13.6 遥测

默认关闭。

若未来添加匿名统计，必须：

- 用户主动开启；
- 不收集文件名、路径、密码和内容；
- 可随时关闭；
- 提供数据说明。

---

# 14. 格式能力矩阵

## 14.1 创建

| 格式 | 创建 | 密码 | 文件名加密 | 分卷 | V1.0 |
|---|---:|---:|---:|---:|---:|
| 7Z | 是 | 是 | 是 | 是 | 必须 |
| ZIP | 是 | 是，视后端能力 | 否 | 是 | 必须 |
| TAR | 是 | 否 | 否 | 否 | 必须 |
| TAR.GZ | 是 | 否 | 否 | 否 | 必须 |
| TAR.XZ | 是 | 否 | 否 | 否 | 必须 |
| TAR.ZST | 可选 | 否 | 否 | 否 | 可选 |
| RAR | 否 | — | — | — | 禁止 |

## 14.2 读取和解压

| 格式 | 浏览 | 解压 | 创建 |
|---|---:|---:|---:|
| 7Z | 是 | 是 | 是 |
| ZIP | 是 | 是 | 是 |
| RAR | 是 | 是 | 否 |
| TAR | 是 | 是 | 是 |
| GZ | 是 | 是 | 组合创建 |
| XZ | 是 | 是 | 组合创建 |
| BZ2 | 是 | 是 | 可选 |
| ISO | 可选 | 可选 | 否 |
| CAB | 可选 | 可选 | 否 |
| WIM | 可选 | 可选 | 可选 |

UI 必须以后端能力探测为准。

---

# 15. 信息架构

```text
轻压 · QZip
├─ 首页
│  ├─ 拖拽区
│  ├─ 压缩文件
│  └─ 打开压缩包
├─ 创建压缩包
│  ├─ 输入摘要
│  ├─ 基础参数
│  └─ 更多设置
├─ 快速解压
│  ├─ 压缩包摘要
│  ├─ 解压参数
│  └─ 查看内容
├─ 压缩包浏览
│  ├─ 工具栏
│  ├─ 面包屑
│  ├─ 搜索
│  ├─ 文件列表
│  └─ 状态栏
├─ 任务中心
│  ├─ 进行中
│  ├─ 已完成
│  └─ 失败
└─ 设置
   ├─ 常规
   ├─ 压缩
   ├─ 解压
   ├─ 文件关联
   ├─ 外观
   ├─ 隐私与安全
   ├─ 更新
   └─ 关于
```

---

# 16. UI 总体设计规范

## 16.1 视觉方向

关键词：

```text
现代
轻量
温和
清晰
克制
可靠
精致
```

要求：

- 默认浅色为暖白和浅灰；
- 不使用刺眼纯白作为大面积背景；
- 主题色仅用于强调；
- 卡片层级弱而清晰；
- 不使用大面积霓虹或毛玻璃；
- 不把桌面应用设计成网页后台；
- 不模仿 7-Zip 或 WinRAR 工具栏；
- 页面只保留一个主操作。

## 16.2 Design Token 分层

```text
基础色 Token
→ 语义色 Token
→ 组件 Token
→ 页面 Token
```

组件不得直接引用散落的十六进制色值。

## 16.3 CSS Token 命名

```text
--qzip-color-bg-app
--qzip-color-bg-surface
--qzip-color-bg-elevated
--qzip-color-bg-subtle
--qzip-color-text-primary
--qzip-color-text-secondary
--qzip-color-border-default
--qzip-color-accent
--qzip-color-accent-hover
--qzip-color-accent-soft
--qzip-color-success
--qzip-color-warning
--qzip-color-danger
```

---

# 17. 基础色系统

## 17.1 浅色模式

| Token | 色值 |
|---|---|
| `bg-app` | `#F5F6F4` |
| `bg-surface` | `#FBFCFA` |
| `bg-elevated` | `#FFFFFF` |
| `bg-subtle` | `#F1F3F0` |
| `text-primary` | `#262A28` |
| `text-secondary` | `#626A66` |
| `text-tertiary` | `#8A918D` |
| `text-disabled` | `#B4BAB6` |
| `border-default` | `#E1E5E2` |
| `border-strong` | `#CDD3CF` |
| `divider` | `#E8EBE9` |

## 17.2 暗夜模式

不得使用纯黑。

| Token | 色值 |
|---|---|
| `bg-app` | `#171A19` |
| `bg-surface` | `#1D211F` |
| `bg-elevated` | `#252A27` |
| `bg-subtle` | `#202522` |
| `text-primary` | `#F1F4F2` |
| `text-secondary` | `#B7BEB9` |
| `text-tertiary` | `#89918C` |
| `text-disabled` | `#626A65` |
| `border-default` | `#343A36` |
| `border-strong` | `#454D48` |
| `divider` | `#2B302D` |

## 17.3 语义色

主题切换不得改变语义色含义。

| 语义 | 浅色 | 暗夜 |
|---|---|---|
| 成功 | `#2FA36B` | `#4BC889` |
| 警告 | `#D78C22` | `#E7A84B` |
| 错误 | `#D94E4E` | `#F16B6B` |
| 信息 | `#3E7FC4` | `#69A5E4` |

---

# 18. 主题色系统

必须支持以下 6 套色系，每套均支持浅色和暗夜模式。

## 18.1 薄荷绿 Mint，默认

| 模式 | Accent | Hover | Soft |
|---|---|---|---|
| 浅色 | `#42B883` | `#329C6D` | `#EFFAF5` |
| 暗夜 | `#55D19A` | `#6BDAA9` | `#17352A` |

## 18.2 海洋蓝 Ocean

| 模式 | Accent | Hover | Soft |
|---|---|---|---|
| 浅色 | `#3B8ED0` | `#2F76B0` | `#EEF6FD` |
| 暗夜 | `#5AAAE5` | `#75BAEA` | `#162C3C` |

## 18.3 紫藤 Lavender

| 模式 | Accent | Hover | Soft |
|---|---|---|---|
| 浅色 | `#9366C7` | `#7B50AB` | `#F7F2FC` |
| 暗夜 | `#B58AE0` | `#C5A0E8` | `#30233B` |

## 18.4 琥珀橙 Amber

| 模式 | Accent | Hover | Soft |
|---|---|---|---|
| 浅色 | `#DFA536` | `#BF8526` | `#FFF7E8` |
| 暗夜 | `#EDB84F` | `#F3C86C` | `#3A2B16` |

## 18.5 珊瑚红 Coral

| 模式 | Accent | Hover | Soft |
|---|---|---|---|
| 浅色 | `#DD6F61` | `#C3594D` | `#FFF1EF` |
| 暗夜 | `#EF8A7C` | `#F3A195` | `#3B2321` |

珊瑚主题不得替代错误语义色。

## 18.6 青灰 Cyan Slate

| 模式 | Accent | Hover | Soft |
|---|---|---|---|
| 浅色 | `#4D9D9A` | `#3D817F` | `#EFF7F7` |
| 暗夜 | `#63BAB6` | `#7BC8C4` | `#183130` |

## 18.7 主题应用规则

主题色可用于：

- 主按钮；
- 选中项；
- 进度条；
- 焦点；
- 当前导航；
- 链接；
- 开关；
- 运行中状态；
- 插画小面积强调。

主题色不得用于：

- 大面积页面背景；
- 正文；
- 所有卡片；
- 警告和错误；
- 所有文件类型图标。

---

# 19. 字体与排版

## 19.1 字体栈

```css
font-family:
  "Segoe UI Variable",
  "Segoe UI",
  "PingFang SC",
  "Microsoft YaHei UI",
  "Noto Sans CJK SC",
  system-ui,
  sans-serif;
```

不得随应用分发第三方字体文件。

## 19.2 字号

| 层级 | 字号 | 行高 | 字重 |
|---|---:|---:|---:|
| Display | 36 | 48 | 650 |
| H1 | 28 | 38 | 650 |
| H2 | 22 | 32 | 600 |
| H3 | 18 | 28 | 600 |
| Body Large | 16 | 26 | 400 |
| Body | 14 | 22 | 400 |
| Body Small | 13 | 20 | 400 |
| Caption | 12 | 18 | 400 |

中文不得低于 12px。

数字使用：

```css
font-variant-numeric: tabular-nums;
```

---

# 20. 间距、圆角、阴影

## 20.1 间距

采用 4px 网格：

```text
4, 8, 12, 16, 20, 24, 32, 40, 48, 64
```

常用：

- 图标与文字 8px；
- 页面边距 32～40px；
- 卡片内部 20～24px；
- 卡片间距 16px；
- 工具栏间距 12px。

## 20.2 圆角

| Token | 值 |
|---|---:|
| XS | 4 |
| S | 6 |
| M | 8 |
| L | 12 |
| XL | 16 |
| Round | 999 |

## 20.3 阴影

浅色：

```css
--shadow-sm: 0 1px 2px rgba(28, 36, 31, 0.06);
--shadow-md: 0 6px 18px rgba(28, 36, 31, 0.08);
--shadow-lg: 0 16px 40px rgba(28, 36, 31, 0.12);
```

暗夜优先使用边框，仅在浮层使用阴影。

---

# 21. 应用窗口

默认：

```text
1080 × 720
```

最小：

```text
760 × 520
```

标题栏建议：

```text
56px
```

窗口右侧：

- 任务；
- 设置；
- 系统控制。

仅任务中心和设置页面允许局部侧栏。

---

# 22. 核心组件

## 22.1 Button

类型：

- Primary；
- Secondary；
- Tertiary；
- Danger；
- Icon Button。

Primary：

```text
高度 44px
圆角 8px
左右内边距 20～24px
```

必须包含：

- default；
- hover；
- pressed；
- focus；
- disabled；
- loading。

## 22.2 Segmented Control

用于：

- 7Z / ZIP / TAR；
- 快速 / 均衡 / 更小；
- 询问 / 重命名 / 覆盖 / 跳过。

规则：

- 高度 40px；
- 最多 4 项；
- 选中使用 Accent Soft；
- 不使用高饱和实心背景。

## 22.3 Input

```text
高度 40px
圆角 8px
边框 1px
左右内边距 12px
```

必须有：

- default；
- hover；
- focus；
- error；
- disabled；
- readonly。

## 22.4 Card

仅用于：

- 文件摘要；
- 任务；
- 统计摘要；
- 设置分组。

不得每一行表单都做成单独卡片。

## 22.5 Progress

```text
普通 6px
重点 8px
```

不确定进度使用柔和流动动画。

## 22.6 Table

默认行高：

```text
舒适 48px
紧凑 40px
```

选中行：

- Accent Soft；
- 可加左侧 2px Accent；
- 不使用高饱和整行背景。

## 22.7 Toast

仅用于非阻塞反馈。

不得用于：

- 密码错误；
- 危险压缩包；
- 覆盖确认；
- 严重失败。

---

# 23. 页面需求

# 23.1 首页

## 页面目标

提供最短的压缩和解压入口。

## 元素

- 插画；
- “将文件拖到这里”；
- “快速压缩、解压与浏览压缩包”；
- 压缩文件；
- 打开压缩包；
- 格式提示；
- 顶部任务和设置。

## 拖拽判断

| 内容 | 行为 |
|---|---|
| 普通文件 | 创建压缩包 |
| 普通文件夹 | 创建压缩包 |
| 多个普通对象 | 创建一个压缩包 |
| 单个压缩包 | 快速解压 |
| 多个压缩包 | 多任务解压 |
| 混合对象 | 选择处理方式 |
| 不支持 | 明确提示 |

## 拖拽激活

显示：

- “释放以创建压缩包”；
- 或“释放以解压此文件”。

不使用强虚线框，使用柔和边框和背景变化。

## 验收

- 类型识别不阻塞 UI；
- 拖入后在当前窗口切换页面；
- 不弹出传统参数窗口。

# 23.2 创建压缩包

## 默认显示

- 输入摘要；
- 文件名；
- 保存位置；
- 格式；
- 压缩方式；
- 密码；
- 开始压缩；
- 更多设置。

## 默认规则

- 单文件夹：使用文件夹名；
- 单文件：使用不含扩展名的文件名；
- 多文件：使用“压缩文件-日期时间”；
- 首次默认 7z；
- 默认 Balanced；
- 默认输出到源目录；
- 同名文件自动追加序号；
- 不直接覆盖。

## 更多设置

- 分卷；
- 文件名加密；
- Solid；
- 压缩后测试；
- 删除源文件；
- 排除规则；
- 线程；
- 字典大小；
- 算法。

线程、字典和算法仅专家模式显示。

## 验收

- 默认最多两次点击开始；
- 格式切换后动态禁用不支持选项；
- 创建任务立即返回任务 ID；
- 页面不等待任务完成。

# 23.3 快速解压

## 摘要

- 格式；
- 压缩大小；
- 预计解压大小；
- 文件数量。

## 设置

- 解压位置；
- 冲突处理；
- 密码；
- 开始解压；
- 查看压缩包内容。

## 默认

- 解压到同名目录；
- 冲突策略 Rename；
- 默认不创建链接；
- 风险检查先于写文件。

## 验收

- 普通压缩包一键解压；
- 密码错误可原地重试；
- 风险压缩包明确说明；
- 不静默覆盖。

# 23.4 压缩包浏览

## 工具栏

```text
返回
项目资料.7z
添加 ▼
解压 ▼
测试 ▼
更多 ▼
```

## 第二行

- 面包屑；
- 搜索。

## 默认列

- 名称；
- 大小；
- 类型；
- 修改时间。

## 操作

- 双击目录；
- 面包屑导航；
- 多选；
- 搜索；
- 部分解压；
- 测试；
- 查看属性。

## 性能

- 大列表使用虚拟列表；
- 列表加载不阻塞；
- 搜索首先进行前端即时过滤；
- 极大压缩包可分批载入。

# 23.5 任务中心

## 局部侧栏

- 进行中；
- 已完成；
- 失败。

## 任务卡

进行中显示：

- 文件名；
- 状态；
- 进度；
- 当前文件；
- 速度；
- 剩余时间；
- 大小；
- 暂停，占位可禁用；
- 取消。

首版不支持真实暂停时，暂停按钮不得伪装可用，应隐藏或禁用并说明。

完成显示：

- 打开结果；
- 打开位置；
- 再次执行。

失败显示：

- 明确原因；
- 修复动作；
- 重试；
- 技术详情。

# 23.6 设置

分类：

- 常规；
- 压缩；
- 解压；
- 文件关联；
- 外观；
- 隐私与安全；
- 更新；
- 关于。

外观必须包含：

- 浅色；
- 暗夜；
- 跟随系统；
- 6 套色系；
- 列表密度；
- 减少动态效果；
- 90%、100%、110%、125% 缩放。

---

# 24. 交互和动效

## 24.1 时长

| 类型 | 时长 |
|---|---:|
| Hover | 100～140ms |
| Press | 80～120ms |
| 浮层 | 140～180ms |
| 页面局部切换 | 180～220ms |
| 折叠 | 180～240ms |

缓动：

```css
cubic-bezier(0.2, 0, 0, 1)
```

## 24.2 原则

- 不使用夸张弹跳；
- 不使用长粒子动画；
- 不使用文件飞行动画；
- 任务完成仅平滑到 100% 并切换状态；
- 支持减少动态效果。

---

# 25. 无障碍

必须：

- 键盘访问核心功能；
- 焦点顺序符合视觉顺序；
- 焦点有 2px 主题色边框和外环；
- 正文对比度至少 4.5:1；
- 大文字至少 3:1；
- 状态不只依靠颜色；
- 图标按钮有可访问名称和 Tooltip；
- 最小点击区 32×32，建议 36×36。

快捷键：

| 快捷键 | 功能 |
|---|---|
| Ctrl/Cmd + O | 打开压缩包 |
| Ctrl/Cmd + N | 创建压缩包 |
| Ctrl/Cmd + F | 搜索 |
| Ctrl/Cmd + A | 全选 |
| Enter | 主操作 |
| Esc | 关闭或取消 |
| Delete | 删除选中项 |
| Space | 切换选择 |

---

# 26. 响应式窗口

## 26.1 大于 1100px

- 完整工具栏；
- 完整表格列；
- 完整任务信息。

## 26.2 900～1099px

- 缩小工具栏间距；
- 隐藏低优先级列；
- 任务详情允许换行。

## 26.3 760～899px

- 测试和更多可合并；
- 隐藏类型或修改时间；
- 双按钮可垂直排列；
- 任务侧栏缩窄。

不得出现页面横向滚动。

---

# 27. 设置与本地存储

## 27.1 配置模型

```ts
interface AppSettings {
  themeMode: "light" | "dark" | "system";
  accentTheme:
    | "mint"
    | "ocean"
    | "lavender"
    | "amber"
    | "coral"
    | "cyan-slate";
  uiScale: 0.9 | 1 | 1.1 | 1.25;
  listDensity: "comfortable" | "compact";
  reduceMotion: boolean;
  defaultFormat: ArchiveFormat;
  compressionProfile: CompressionProfile;
  conflictPolicy: ConflictPolicy;
  extractToNamedFolder: boolean;
  avoidDuplicateRootFolder: boolean;
  openFolderAfterExtract: boolean;
  testAfterCreate: boolean;
  telemetryEnabled: false;
}
```

## 27.2 存储

使用应用配置文件或 Tauri Store 类能力。

不得保存：

- 密码；
- 完整命令；
- 未脱敏诊断数据。

## 27.3 任务历史

可以保存：

- 任务类型；
- 显示名称；
- 状态；
- 时间；
- 输出位置；
- 错误代码。

默认不保存压缩包内部文件清单。

---

# 28. 系统集成

## 28.1 文件关联

支持：

- 7z；
- ZIP；
- RAR；
- TAR；
- GZ；
- XZ；
- BZ2。

双击：

- 复用单实例；
- 打开压缩包；
- 必要时请求密码。

## 28.2 右键菜单

V1.0：

- 使用 QZip 打开；
- 压缩为“名称.7z”；
- 压缩为“名称.zip”；
- 解压到当前目录；
- 解压到“名称”；
- 更多压缩选项。

低频项进入二级菜单。

## 28.3 Windows 平台适配

- Explorer；
- 长路径；
- Windows 保留名称；
- 100%～200% DPI；
- NSIS 为主要安装包，MSI 为补充安装包；
- 安装、升级和卸载不得残留关键文件关联；
- 正式发行物必须使用受信任 Authenticode 签名。

其他桌面平台的要求统一见文首“V1.1 跨平台路线图”，不属于 V1.0 验收范围。

---

# 29. 性能指标

工程目标：

- 冷启动尽量 < 1s；
- 首页可交互尽量 < 1.5s；
- 空闲内存尽量 < 100MB；
- 主线程不执行文件扫描或压缩；
- 大列表使用虚拟化；
- 进度事件节流；
- 文件扫描可取消；
- 任务异常不导致主窗口崩溃。

这些是内部目标，未实测前不得对外承诺。

---

# 30. 兼容性要求

必须测试：

- 中文、日文、韩文；
- Emoji；
- 历史 ZIP 编码；
- 超长路径；
- 空目录；
- 隐藏文件；
- 只读文件；
- 软链接；
- 大小写冲突；
- Windows 保留名称；
- 尾部空格和句点；
- 文件占用；
- 磁盘不足；
- 单文件 > 4GB；
- 十万级文件；
- 分卷；
- 密码包；
- 损坏包。

---

# 31. 错误提示规范

提示必须包含：

1. 发生了什么；
2. 可能原因；
3. 下一步；
4. 是否可重试；
5. 技术详情入口。

示例：

```text
无法解压此压缩包

密码不正确，压缩包内容尚未解压。

[重新输入密码] [取消]
查看技术详情
```

禁止直接显示：

```text
ERROR 0x80004005
```

---

# 32. 测试策略

## 32.1 Rust 单元测试

必须覆盖：

- 参数映射；
- 路径规范化；
- 路径穿越；
- 错误映射；
- 状态机；
- 能力判断；
- 输出解析；
- 配置迁移。

## 32.2 集成测试

必须覆盖：

- 创建 7z；
- 创建 ZIP；
- 解压 7z；
- 解压 ZIP；
- 解压 RAR；
- 列表；
- 测试完整性；
- 密码；
- 取消；
- 子进程异常；
- 磁盘不足；
- 文件被占用。

## 32.3 安全测试

必须覆盖：

- Zip Slip；
- 绝对路径；
- UNC；
- 盘符路径；
- 软链接；
- 硬链接；
- 压缩炸弹；
- 深层目录；
- 命令注入；
- 恶意文件名；
- 密码日志泄漏；
- 临时文件泄漏。

## 32.4 前端测试

必须覆盖：

- 首页拖拽判断；
- 创建表单；
- 格式能力动态禁用；
- 任务状态更新；
- 错误重试；
- 主题切换；
- 暗夜模式；
- 键盘焦点；
- 窗口窄屏布局。

## 32.5 交叉兼容样本

使用以下软件生成测试文件：

- 7-Zip；
- WinRAR；
- Bandizip；
- PeaZip；
- Keka；
- 系统 ZIP；
- tar、gzip、xz。

测试样本不得包含受版权或隐私限制的数据。

---

# 33. 编码规范

## 33.1 Rust

- `cargo fmt`；
- `cargo clippy -- -D warnings`；
- 公共类型有文档；
- 错误使用枚举；
- 禁止大量 `unwrap()`；
- 进程资源必须可释放；
- 异步任务必须支持取消。

## 33.2 TypeScript

- `strict: true`；
- 禁止无理由 `any`；
- API DTO 有明确类型；
- 业务状态放 feature/store；
- 页面不直接维护后端进程状态；
- 组件保持单一职责；
- 样式使用 Token，不硬编码主题色。

## 33.3 React

推荐目录：

```text
features/archive-create
features/archive-extract
features/archive-browser
features/tasks
features/settings
```

每个 feature 包含：

```text
components
hooks
store
types
services
tests
```

## 33.4 日志

日志级别：

- error；
- warn；
- info；
- debug。

生产环境默认不输出 debug。

---

# 34. CI 质量门禁

Pull Request 必须通过：

- 格式检查；
- TypeScript 类型检查；
- ESLint；
- 前端单测；
- Rust 测试；
- Clippy；
- 安全测试子集；
- 构建验证；
- 许可证检查；
- 不提交密钥或密码。

发布构建必须：

- 生成校验和；
- 包含第三方许可；
- 记录 7-Zip 版本；
- 对应源码可获取；
- 校验 Sidecar 文件。

---

# 35. 里程碑实施计划

## M0：仓库和设计系统

目标：

- 初始化 Monorepo；
- Tauri 2 桌面壳；
- React；
- Rust crates；
- Design Token；
- 浅色、暗夜；
- 6 套主题；
- 基础组件；
- 首页静态页面。

验收：

- Windows x64 至少能启动开发构建；macOS/Linux 开发构建与发行验收属于 V1.1；
- 主题切换即时生效；
- 不刷新页面；
- UI Token 无散落主题硬编码；
- 首页适配 760px 最小宽度。

## M1：后端技术验证

目标：

- `ArchiveBackend`；
- `SevenZipCliBackend`；
- Sidecar；
- 能力探测；
- 创建 7z、ZIP；
- 解压 7z、ZIP、RAR；
- 列表；
- 测试；
- CLI 测试入口；
- 取消。

验收：

- 所有操作不经 Shell；
- 中文路径成功；
- 密码不进入日志；
- 错误映射可用；
- 核心解析有测试。

## M2：创建压缩包

目标：

- 拖入普通文件；
- 扫描；
- 创建页面；
- 默认命名；
- 格式；
- 压缩方式；
- 密码；
- 任务创建；
- 底部任务条。

验收：

- 默认两次点击开始；
- 7z 创建成功；
- ZIP 创建成功；
- 任务可取消；
- 不阻塞界面。

## M3：快速解压和安全

目标：

- 拖入压缩包；
- 格式识别；
- 摘要；
- 输出目录；
- 冲突策略；
- 密码；
- 安全策略；
- 风险提示；
- 解压。

验收：

- 路径穿越全部拦截；
- 密码错误可重试；
- 同名文件不静默覆盖；
- 取消清理临时文件。

## M4：压缩包浏览

目标：

- 列表；
- 面包屑；
- 搜索；
- 多选；
- 部分解压；
- 完整性测试；
- 状态栏；
- 虚拟列表。

验收：

- 大型列表可滚动；
- 搜索不阻塞；
- 部分解压正确；
- 暗夜模式可读。

## M5：任务中心

目标：

- 进行中；
- 已完成；
- 失败；
- 重试；
- 清理；
- 打开位置；
- 错误修复入口。

验收：

- 状态机无非法跳转；
- 重试不保留密码；
- 多任务 UI 不混乱；
- 失败原因明确。

## M6：设置与系统集成

目标：

- 设置；
- 文件关联；
- 右键菜单；
- 单实例；
- 通知；
- 自动更新基础；
- Windows 安装、升级和卸载。

验收：

- 设置持久化；
- 主题和缩放持久化；
- 双击打开；
- 右键操作可用；
- 安装卸载无残留关键关联。

## M7：发布候选

目标：

- 性能；
- 兼容测试；
- 安全测试；
- 文档；
- 许可证；
- GitHub Actions；
- 发布包。

验收：

- RC1 必须通过 Windows 安全、安装和发布门禁，可携带完整披露的功能已知问题；
- V1.0 稳定版必须通过全部 Windows 验收；
- 已知问题文档完整；
- 安装包具有受信任签名和校验和；
- 第三方许可完整。

---

# 36. V1.0 总体验收

## 36.1 功能

- 创建 7z；
- 创建 ZIP；
- 创建 TAR 系列；
- 解压 7z、ZIP、RAR；
- 浏览内容；
- 部分解压；
- 密码；
- 文件名加密；
- 分卷；
- 取消；
- 测试；
- 拖拽；
- 文件关联；
- 右键菜单。

## 36.2 体验

- 普通压缩最多两次点击；
- 普通解压一次确认；
- 默认不展示专家参数；
- 不使用阻塞式独立进度窗口；
- 失败提示可执行；
- 浅色和暗夜完整；
- 6 套色系完整；
- 键盘可操作；
- 最小窗口不溢出。

## 36.3 安全

- 路径穿越拦截；
- 绝对路径拦截；
- 密码不在日志；
- 压缩炸弹检测；
- 不静默覆盖；
- 取消后清理；
- 不经 Shell。

## 36.4 发布

- Windows 10/11 x64 可安装、升级和卸载；
- Explorer 文件关联和右键菜单可用且卸载后正确清理；
- NSIS、MSI 和便携包具有受信任 Authenticode 签名；
- 校验和；
- 许可；
- 隐私说明；
- 卸载说明；
- 已知问题；
- GitHub Release。

---

# 37. 发布产物命名规范

公开发行文件应使用统一英文品牌，避免中文文件名在不同平台和下载工具中出现兼容问题。

建议格式：

```text
QZip-{version}-windows-x64-setup.exe
QZip-{version}-windows-x64.msi
QZip-{version}-windows-x64-portable.zip
QZip-{version}-checksums.txt
```

系统内显示：

- 开始菜单、任务栏、Dock、应用列表：`QZip`；
- 中文应用关于页标题：`轻压 · QZip`；
- 系统右键菜单：“使用 QZip 打开”；
- 默认窗口标题：`QZip`；
- 打开压缩包时：`文件名 — QZip`。

不得使用早期临时工程名，也不得使用：

- `QZIP` 全大写作为用户可见品牌；
- `7-Zip UI`、`7-Zip Modern` 等可能造成官方关联误解的名称。

---

# 38. 开源与许可

## 37.1 项目许可证

建议：

- 核心项目 Apache-2.0；
- 品牌名称、Logo 和官方发行标识单独管理；
- Pro 模块独立许可；
- 外部贡献采用明确贡献协议。

## 37.2 7-Zip 合规文件

```text
LICENSE
NOTICE
THIRD_PARTY_LICENSES.md
DEPENDENCIES.md
third_party/7zip/
├─ LICENSE.txt
├─ VERSION
├─ SOURCE_INFO.md
└─ source/
```

记录：

- 版本；
- 二进制来源；
- 是否修改；
- 对应源码；
- 第三方许可；
- 产品不是 7-Zip 官方版本。

正式商业发布前应进行许可证审核。

---

# 39. 风险与应对

| 风险 | 应对 |
|---|---|
| 7-Zip 输出变化 | 版本适配器、固定测试样本 |
| 密码命令参数可见 | 脱敏、后续库级后端 |
| Sidecar 打包复杂 | 自动化矩阵和构建校验 |
| Windows 签名凭据缺失 | 发布工作流失败关闭，不创建公开 Release |
| 大列表卡顿 | 后台解析、虚拟列表 |
| 格式能力不一致 | 动态能力探测 |
| 开源重打包 | 商标、签名、官方更新 |
| 商业化影响口碑 | 社区版完整、无广告 |
| 视觉过重 | Token、性能门禁、减少动效 |
| 功能膨胀 | 严格执行非目标和里程碑 |

---

# 40. Codex 首次实施顺序

Codex 首次开始开发时，按以下顺序执行：

1. 使用 `qzip-desktop` 创建仓库结构；
2. 初始化 Tauri、React 和 Rust Workspace；
3. 配置 `productName = "QZip"` 和 `identifier = "app.qzip.desktop"`；
4. 创建 `archive-core` 类型和 Trait；
5. 创建 `--qzip-` Design Token 和主题系统；
6. 实现基础 Button、Input、SegmentedControl、Card、Progress；
7. 完成首页静态界面，并使用“轻压 · QZip”品牌资源；
8. 接入开发环境 Sidecar；
9. 实现能力探测；
10. 实现最小 7z 创建验证；
11. 为参数映射和输出解析补测试。

在 M0 和 M1 验收前，不应开发文件关联、自动更新、Pro 或企业功能。

---

# 41. CSS 主题变量参考

```css
:root {
  --qzip-color-bg-app: #f5f6f4;
  --qzip-color-bg-surface: #fbfcfa;
  --qzip-color-bg-elevated: #ffffff;
  --qzip-color-bg-subtle: #f1f3f0;

  --qzip-color-text-primary: #262a28;
  --qzip-color-text-secondary: #626a66;
  --qzip-color-text-tertiary: #8a918d;

  --qzip-color-border-default: #e1e5e2;
  --qzip-color-divider: #e8ebe9;

  --qzip-color-accent: #42b883;
  --qzip-color-accent-hover: #329c6d;
  --qzip-color-accent-soft: #effaf5;

  --qzip-color-success: #2fa36b;
  --qzip-color-warning: #d78c22;
  --qzip-color-danger: #d94e4e;
  --qzip-color-info: #3e7fc4;

  --qzip-radius-sm: 6px;
  --qzip-radius-md: 8px;
  --qzip-radius-lg: 12px;
  --qzip-radius-xl: 16px;

  --qzip-shadow-sm: 0 1px 2px rgba(28, 36, 31, 0.06);
  --qzip-shadow-md: 0 6px 18px rgba(28, 36, 31, 0.08);
}

[data-mode="dark"] {
  --qzip-color-bg-app: #171a19;
  --qzip-color-bg-surface: #1d211f;
  --qzip-color-bg-elevated: #252a27;
  --qzip-color-bg-subtle: #202522;

  --qzip-color-text-primary: #f1f4f2;
  --qzip-color-text-secondary: #b7beb9;
  --qzip-color-text-tertiary: #89918c;

  --qzip-color-border-default: #343a36;
  --qzip-color-divider: #2b302d;

  --qzip-color-accent: #55d19a;
  --qzip-color-accent-hover: #6bdaa9;
  --qzip-color-accent-soft: #17352a;

  --qzip-color-success: #4bc889;
  --qzip-color-warning: #e7a84b;
  --qzip-color-danger: #f16b6b;
  --qzip-color-info: #69a5e4;
}

[data-accent="ocean"] {
  --qzip-color-accent: #3b8ed0;
  --qzip-color-accent-hover: #2f76b0;
  --qzip-color-accent-soft: #eef6fd;
}

[data-mode="dark"][data-accent="ocean"] {
  --qzip-color-accent: #5aaae5;
  --qzip-color-accent-hover: #75baea;
  --qzip-color-accent-soft: #162c3c;
}
```

其余主题使用同一规则实现，不为每套主题复制整套组件 CSS。

---

# 42. 最终开发基线

轻压 · QZip 必须保持以下平衡：

```text
成熟的 7-Zip 能力
+ 可替换后端
+ 现代桌面 UI
+ 简洁任务流
+ 完整安全保护
+ 可移植的平台边界
+ 开源可持续性
```

任何新增功能在进入版本范围前必须回答：

1. 是否属于高频需求；
2. 是否增加主流程步骤；
3. 是否可以放入更多设置；
4. 是否会增加安装体积或维护风险；
5. 是否会破坏 UI、应用服务和后端解耦；
6. 是否有明确验收标准。

无法回答以上问题的功能不得直接进入主分支。
