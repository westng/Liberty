# 更新日志 / Changelog

## 1.1.24 - 2026-07-30

### 中文

#### 新增

- 新增工作中心、结果中心、职业任务与职业市场，集中呈现会议任务、处理结果和宠物工作玩法。
- 新增农场、便利店、工厂和矿场等工作场景及配套地图、角色动画和任务资源。
- 新增桌面构建的手动发布选项，可按需仅验证安装包或在全部校验通过后发布 GitHub Release。

#### 调整

- 重构 Python、FFmpeg 与 FunASR 模型的运行环境管理，各组件可独立选择本机环境或托管下载，模型下载不再等待其他组件完成。
- 优化侧边栏、任务创建、任务列表、结果中心和游戏页面布局，减少重复状态信息并强化主要操作层级。
- 强化桌面权限、单实例运行、后台任务调度、远程任务同步、凭据存储和跨平台质量门禁。

#### 修复

- 修复 Word 会议纪要导出偶发遗漏参会人员或内容的问题，导出时以用户选定的完整 AI 总结结果为唯一数据源并保持纪要格式。
- 修复 Windows x64/x86 编译、安装包生成及 MSI/NSIS 发布资产校验问题。
- 修复桌宠渲染和平台相关代码在严格格式、Clippy 与 Windows 编译检查中的兼容性问题。

### English

#### Added

- Added the Work Hub, Results Center, career tasks, and Work Market to bring meeting jobs, processing results, and pet work activities into a clearer workflow.
- Added farm, convenience store, factory, and mine work scenes with dedicated maps, character animations, and task assets.
- Added a manual publishing option to desktop builds so a run can validate installers only or publish a GitHub Release after every check succeeds.

#### Changed

- Decoupled Python, FFmpeg, and FunASR model runtime management so each component can independently use a detected local environment or a managed download, while model downloads can start immediately.
- Refined the sidebar, task creation, task list, Results Center, and game layouts to remove duplicate status panels and emphasize primary actions.
- Hardened desktop permissions, single-instance behavior, background scheduling, remote job synchronization, credential storage, and cross-platform quality gates.

#### Fixed

- Fixed intermittent missing attendees or content in exported Word meeting notes by using the complete user-selected AI summary run as the authoritative export source while preserving note formatting.
- Fixed Windows x64/x86 compilation, installer generation, and MSI/NSIS release asset validation.
- Fixed desktop pet rendering and platform-specific compatibility issues caught by strict formatting, Clippy, and Windows compile checks.

## 1.1.23 - 2026-06-06

### 中文

#### 新增

- 新增桌宠动作序列帧资源，补齐 `crush`、`defecate`、`drive`、`eat`、`gaming`、`pants`、`reading`、`rope`、`run`、`slack`、`sleep`、`snow`、`studying`、`toy`、`work` 共 15 组动作，每组 9 帧。
- 新增桌宠空闲动作池中的 `gaming`、`reading`、`studying` 动作，使桌宠日常状态更丰富。

#### 调整

- 桌宠动作帧命名调整为 `动作名_01.png` 至 `动作名_09.png`，前端与 Tauri 桌宠加载逻辑均支持按数字序号排序播放。
- 桌宠动画播放间隔由 1000ms 调整为 300ms，使 9 帧序列动画播放更连贯。
- 桌宠资源加载优先读取前端动作资源目录，开发环境下可直接预览最新动作帧。

#### 修复

- 修复 `snow` 动作 4-6 帧顶部残留、1-3 帧脚部像素缺失，以及部分底部切图碎点。
- 修复 `crush`、`pants`、`toy` 动作 7-9 帧头部羊毛区域被误扣透明的问题。
- 清理 `toy` 动作 4-6 帧底部多余切片残留，保持主体位置和透明画布不变。

### English

#### Added

- Added desktop pet sequence-frame assets for 15 action groups: `crush`, `defecate`, `drive`, `eat`, `gaming`, `pants`, `reading`, `rope`, `run`, `slack`, `sleep`, `snow`, `studying`, `toy`, and `work`, with 9 frames per action.
- Added `gaming`, `reading`, and `studying` to the desktop pet idle action pool for richer daily behavior.

#### Changed

- Renamed pet action frames to the `action_01.png` through `action_09.png` pattern, with both frontend and Tauri loaders sorting frames by numeric order.
- Reduced the desktop pet animation interval from 1000ms to 300ms so 9-frame sequences play more smoothly.
- Prioritized loading pet resources from the frontend action asset directory in development so updated frames can be previewed directly.

#### Fixed

- Fixed `snow` frame issues including top-center leftovers in frames 4-6, missing foot pixels in frames 1-3, and small bottom trimming artifacts.
- Fixed incorrectly transparent head-wool areas in `crush`, `pants`, and `toy` frames 7-9.
- Removed extra bottom-slice remnants from `toy` frames 4-6 while preserving sprite position and transparent canvas size.

## 1.1.21 - 2026-06-05

### 中文

#### 新增

- 新增宠物每日签到中心，支持连续签到奖励、奖励日历、历史记录查看。
- 新增断签补签能力，7 天内断签可消耗「补签票券」补回。
- 新增「补签票券」道具用途，由原「宝石票券」调整为补签消耗道具。
- 新增本机运行环境自动检测，检测到可用的 Python、FFmpeg 与 FunASR 依赖时，可优先使用本机环境处理本地任务。
- 新增离线兑换 Key 能力，支持通过本地 HMAC 短码发放 LP、成长值与宠物道具，并提供本地批量生成工具。

#### 调整

- Python 运行环境与 FFmpeg 改为外置下载，不再随主安装包内置，以降低 CI 构建耗时和安装包体积。
- 系统设置中的「模型下载」调整为「环境&模型」，以列表形式展示 Python、FFmpeg、FunASR 模型；下载源需配置真实可用的资源地址后启用。
- 运行环境资源改为由客户端内下载入口获取，CI 不再单独构建 Python、FFmpeg 与模型资源包。
- 优化「环境&模型」设置布局，运行资源、下载源、平台标签与安装日志按设置列表风格展示，减少拥挤和多余标题占位。
- 优化本机环境检测失败提示，托管环境未安装时会展示缺少 Python 依赖、FFmpeg 等具体原因。
- 优化 Windows 安装程序、macOS 任务栏菜单与原生系统弹窗语言，默认使用简体中文显示。
- 调整 LP 基础收入，使 79 级用户轻量日常收入约为 112-152 LP/天，完整活跃日约为 239-279 LP/天。
- 调整签到奖励轨道，14 天周期内 LP 奖励提升为 20-60 LP。
- 优化抽奖、签到、补签、礼盒等宠物来源文案映射，避免界面显示原始枚举值。

#### 说明

- 每日盲盒当前为权重抽取：食物 57.80%、道具 31.61%、空奖 1.88%、补签票券 1.04%。
- 抽奖重复非消耗品会转换为 LP 补偿；消耗品重复进入库存，不作为稳定 LP 日收入。

### English

#### Added

- Added the pet daily check-in center with streak rewards, a reward calendar, and history viewing.
- Added missed check-in recovery. Missed days within 7 days can be restored by consuming makeup tickets.
- Repurposed the former Gem Ticket item into the Makeup Ticket used for check-in recovery.
- Added local runtime detection for Python, FFmpeg, and FunASR so available local dependencies can be used first.
- Added offline redeem key support with local HMAC short codes for granting LP, growth value, and pet items, plus a local batch generation tool.

#### Changed

- Moved Python runtime and FFmpeg to external downloads instead of bundling them in the main installer, reducing CI build time and installer size.
- Renamed Settings > Model Download to Environment & Models, showing Python, FFmpeg, and FunASR as list items; download sources must be configured with real available resource URLs before use.
- Runtime resources are now downloaded from inside the client; CI no longer builds separate Python, FFmpeg, or model resource packages.
- Improved the Environment & Models layout so runtime resources, download sources, platform tags, and install logs follow the settings list style with less crowding.
- Improved local environment detection messages with specific reasons when Python dependencies, FFmpeg, or managed runtimes are missing.
- Localized the Windows installer, macOS tray menu, and native system dialogs to Simplified Chinese by default.
- Adjusted base LP income so a level 79 user earns about 112-152 LP on a light daily routine and about 239-279 LP on a fully active day.
- Increased the 14-day check-in reward track to 20-60 LP.
- Improved pet source labels for blind boxes, check-ins, makeup check-ins, gift boxes, and related events to avoid showing raw enum values.

#### Notes

- The daily blind box currently uses weighted draws: food 57.80%, tools 31.61%, empty prize 1.88%, and makeup tickets 1.04%.
- Duplicate non-consumable blind box prizes convert to LP compensation; duplicate consumables go into inventory and are not counted as stable daily LP income.
