# 更新日志 / Changelog

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
- 系统设置中的「模型下载」调整为「环境&模型」，以列表形式展示 Python、FFmpeg、FunASR 模型，并支持选择阿里云、腾讯云、华为云或官方源。
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
- Renamed Settings > Model Download to Environment & Models, showing Python, FFmpeg, and FunASR as list items with selectable Alibaba Cloud, Tencent Cloud, Huawei Cloud, and official download sources.
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
