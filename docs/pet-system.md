# Liberty 宠物系统

本文是 Liberty 宠物系统的当前权威说明。旧设计文档和实现计划只保留为历史记录；如果旧文档与本文冲突，以本文为准。

## 定位

宠物系统是 Liberty 桌面端的本地陪伴能力，不属于会议处理主链路。它围绕真实会议工作流记录成长值、LP、事件、库存、装备和每日福利状态，让用户完成任务、转写、AI 总结和导出时获得轻量反馈。

宠物系统必须保持 best-effort：宠物加载、奖励发放、桌宠渲染或商店状态失败时，不能阻塞任务创建、转写、总结、导出或结果查看。

## 当前页面

| 页面 | 路由 | 当前职责 |
| --- | --- | --- |
| 宠物中心 | `/pet` | 宠物名称、等级、累计成长、阶段、心情、本级进度、下一阶段、互动、事件流水、桌宠设置 |
| 宠物商店 | `/pet-store` | LP、商品目录、库存、购买、装备、取消装备、使用食物和消耗道具 |
| 每日盲盒 | `/daily-blind-box` | 每天 10 次免费本地福利，展示奖池、抽取动画、今日历史、重复补偿 |
| 每日签到 | `/daily-check-in` | 连续签到、14 天奖励日历、历史记录、补签窗口和补签票券消耗 |
| 兑换中心 | `/redeem-key` | 输入本地校验的兑换 Key，领取 LP、成长值或道具，并查看本机兑换记录 |
| 商品详情 | `/pet-store-item?itemKey=...` | 商品图片、分类、状态、价格、持有数量、门槛、来源、食物成长值 |
| 牛马市场 | `/work-market` | 打工小游戏地图大厅，展示农场、矿场、工厂和便利店地图状态 |
| 农场种菜 | `/farm` | 播种、浇水、等待成熟并收获宠物商品和 LP |
| 打工小游戏 | `/work-game/:gameKey` | 矿场挖矿、工厂打螺丝、便利店值班的通用任务玩法页 |

每日盲盒不是付费抽卡系统：不消耗 LP，不出售次数，不关联充值、真钱权益或付费概率掉落。
每日签到、补签、礼盒和兑换中心同样只属于本地福利或运营入口，不接入真钱支付、充值、交易、排行榜或服务端账户资产。

## 成长和经济

当前成长系统采用 255 级累计成长曲线。`experience` 表示累计成长值，不是本级内经验。等级快照由 Rust 后端统一计算，前端只消费后端返回的 `PetLevelSnapshot`。

关键字段：

| 字段 | 含义 |
| --- | --- |
| `level` | 当前等级，最大 `255` |
| `currentLevelExp` | 当前等级内已获得成长值 |
| `nextLevelRequired` | 升到下一级需要的成长值 |
| `totalExperience` | 累计成长值 |
| `currentStage` | 当前 8 阶段枚举 |
| `nextStage` | 下一阶段枚举，满级时为空 |
| `progressRatio` | 本级进度比例 |
| `isMaxLevel` | 是否已达到 Lv.255 |

阶段划分：

| 阶段 | 等级范围 | 中文名 |
| --- | ---: | --- |
| `first_meet` | Lv.1-10 | 小小初遇 |
| `familiar` | Lv.11-30 | 轻轻熟悉 |
| `steady_companion` | Lv.31-60 | 稳定陪伴 |
| `grow_together` | Lv.61-100 | 一起成长 |
| `tacit_bond` | Lv.101-140 | 默契养成 |
| `deep_bond` | Lv.141-180 | 深深羁绊 |
| `long_company` | Lv.181-220 | 长久相伴 |
| `bond_forever` | Lv.221-255 | 不离不弃 |

旧阶段兼容映射：

| 旧阶段 | 新阶段 |
| --- | --- |
| `baby` | `first_meet` |
| `growing` | `grow_together` |
| `mature` | `deep_bond` |

工作奖励基础值：

| 工作事件 | 基础成长值 | 基础 LP | 心情 |
| --- | ---: | ---: | --- |
| `daily_open` | +2 | +5 | `idle` |
| `job_created` | +5 | +8 | `cheerful` |
| `transcription_started` | +2 | +3 | `excited` |
| `transcription_completed` | +12 | +18 | `proud` |
| `ai_summary_completed` | +10 | +15 | `proud` |
| `export_completed` | +6 | +10 | `proud` |

互动奖励基础值：

| 互动事件 | 基础成长值 | 基础 LP | 心情 |
| --- | ---: | ---: | --- |
| `tap` | +1 | +1 | `cheerful` |
| `pet` | +1 | +1 | `cheerful` |
| `feed` | +1 | +1 | `proud` |
| `encourage` | +1 | +1 | `cheerful` |

工作成长值按当前等级应用成长值阶段系数，LP 按 LP 阶段系数小幅提升。食物成长值固定，不参与阶段系数。

奖励幂等规则：

- 工作奖励通过 `source_type + source_key` 去重，同一个任务的同一工作事件不会重复发放完整奖励。
- 每日打开按本地自然日去重。
- 互动事件有每日上限，由 `MAX_DAILY_INTERACTION_PER_SOURCE` 控制。
- 食物使用按 `growthValue × quantity` 增加累计成长值。

## 商店、盲盒、签到和兑换

商品目录当前写在 Rust 代码中：

```text
apps/desktop/src-tauri/src/infrastructure/repositories/pet_store.rs
```

商品类型：

| 类型 | 用途 |
| --- | --- |
| `pet` | 当前宠物槽；当前只有默认宠物本体 |
| `cosmetic` | 配饰槽 |
| `theme` | 场景槽 |
| `tool` | 消耗品 |
| `food` | 消耗品，使用后增加固定成长值 |
| `badge` | 徽章槽，原则上通过里程碑自动获得 |

商品状态优先级：

```text
coming_soon
→ equipped
→ owned
→ achievement
→ locked
→ insufficient
→ available
```

盲盒规则：

- 每天最多开启 10 次，按本地自然日重置。
- 奖池来自宠物商店内容，但排除宠物本体。
- 奖池包含空奖。
- 消耗品重复获得时叠加数量。
- 已拥有的非消耗品重复获得时转为少量 LP 补偿。
- 每次开启写入本地盲盒历史，并写入宠物事件流水。

惊喜礼盒规则：

- 商品编码为 `gift-box-tool`，归类为 `tool`。
- 在宠物商店每天最多免费领取 3 个，领取状态写入 `pet_store_daily_limits`。
- 使用礼盒会消耗库存中的 1 个礼盒。
- 礼盒奖池来自商店目录，但排除宠物本体、徽章和礼盒自身。
- 重复获得已拥有的非消耗品时转为 LP 补偿；消耗品按数量叠加。

每日签到规则：

- 签到按本地自然日记录，不能重复领取当天奖励。
- 连续签到奖励按 14 天周期展示，基础奖励为 LP +20、成长值 +5。
- 第 3 天和第 10 天奖励惊喜礼盒，第 5 天奖励纸杯蛋糕，第 7 天奖励四叶草徽章，第 12 天奖励冰淇淋，第 14 天奖励嫩芽徽章。
- 断签后若补签窗口可用，可以消耗 1 张 `gem-ticket-tool` 补签票券补回缺失日期。
- 签到和补签都会写入签到表、成长/经济流水和宠物事件流水。

兑换 Key 规则：

- 兑换中心支持本地校验的短码/紧凑/旧版 Key 格式，当前发放脚本为 `scripts/redeem-key.mjs`。
- Key 可发放 LP、成长值和最多两个道具奖励类型；道具必须存在于当前商店目录。
- 同一宠物对同一 Key 只能兑换一次，重复兑换由 `pet_id + key_hash` 唯一约束阻止。
- 兑换记录只保存 Key 哈希、前缀、活动标识、奖励 JSON、状态、兑换时间和元数据，不保存明文 Key。
- 私密运营文档、私钥和生成 CSV 应放在 `.liberty-secrets/`，该目录不进入仓库。

## 打工小游戏和农场

打工小游戏是宠物商品体系的独立玩法入口，不属于会议任务、转写、AI 总结或导出链路。第一期菜单结构为：

```text
打工
└─ 牛马市场
   ├─ 农场种菜
   ├─ 矿场挖矿
   ├─ 工厂打螺丝
   └─ 便利店值班
```

牛马市场是小游戏地图大厅。当前开放 `农场种菜`、`矿场挖矿`、`工厂打螺丝` 和 `便利店值班`。地图状态按 `可收获/可领取 > 需要照看 > 进行中 > 空闲 > 未开放` 汇总。

农场种菜第一期规则：

| 规则 | 内容 |
| --- | --- |
| 地块 | 初始 3 块 |
| 作物 | 小麦、胡萝卜、番茄、南瓜 |
| 成长 | 按本地真实时间推进，读取农场状态时补算 |
| 操作 | 播种、浇水、收获 |
| 惩罚 | 不枯萎、不失败、不扣奖励 |
| 收益 | 收获宠物商品和少量 LP |

作物数值：

| 作物 | 总成熟时间 | 浇水次数 | 主要收益 |
| --- | ---: | ---: | --- |
| 小麦 | 5 分钟 | 1 次 | 低级食物、少量 LP |
| 胡萝卜 | 15 分钟 | 2 次 | 食物、低概率额外食物 |
| 番茄 | 30 分钟 | 2 次 | 食物、低概率工具 |
| 南瓜 | 60 分钟 | 3 次 | 高级食物、低概率惊喜礼盒 |

农场只维护地块、作物阶段和收获记录。奖励发放复用宠物商品目录、`pet_inventory`、`pet_wallets` 和 `pet_economy_ledger`，不新增农场货币，不新增农作物库存。

矿场、工厂和便利店共用 `work_game` 引擎，不复用农场地块模型。统一生命周期为：

| 状态 | 含义 |
| --- | --- |
| `idle` | 岗位空闲，可开始任务 |
| `running` | 任务按本地真实时间推进 |
| `needsCare` | 到达照看节点，需要完成一次短动作 |
| `claimable` | 任务完成，可领取奖励 |

新增三款小游戏规则：

| 地图 | 岗位 | 周期 | 照看次数 | 收益倾向 |
| --- | --- | ---: | ---: | --- |
| 矿场挖矿 | 浅层矿脉 / 深层矿脉 / 闪光富矿 | 10 / 25 / 45 分钟 | 1 / 2 / 3 次 | 工具、稀有工具、礼盒概率、LP |
| 工厂打螺丝 | 基础装配 / 加急订单 / 精密质检 | 8 / 18 / 35 分钟 | 1 / 2 / 3 次 | 稳定 LP、工具、秒表、稀有工具 |
| 便利店值班 | 白班 / 晚班 / 夜班 | 12 / 24 / 40 分钟 | 1 / 2 / 3 次 | 食物、礼盒概率、日常工具、LP |

三款小游戏只维护岗位任务和奖励记录。奖励发放同样复用宠物商品目录、`pet_inventory`、`pet_wallets` 和 `pet_economy_ledger`，不新增矿石、零件、便利店商品库存或新的打工货币。

## 本地数据

宠物系统使用 SQLite 表：

| 表 | 作用 |
| --- | --- |
| `pet_profile` | 宠物名称、等级、累计成长值、阶段、心情 |
| `pet_settings` | 桌宠启用、置顶、静音、专注模式、主动程度、窗口位置 |
| `pet_cosmetic_unlocks` | 阶段成长解锁的旧装扮记录 |
| `pet_event_ledger` | 互动、工作、装备、食物、盲盒等事件流水 |
| `pet_wallets` | LP 余额、累计获得、累计消耗 |
| `pet_inventory` | 个人仓库、数量、装备状态、来源 |
| `pet_economy_ledger` | LP 获取和消耗流水，带唯一键防重复 |
| `pet_milestone_counters` | 任务、转写、总结、导出、活跃天数等计数 |
| `pet_blind_box_draws` | 每日盲盒开启历史 |
| `pet_daily_check_ins` | 每日签到和补签记录 |
| `pet_store_daily_limits` | 礼盒等每日免费领取状态 |
| `pet_redeem_key_redemptions` | 兑换 Key 的本机兑换记录 |
| `farm_plots` | 农场地块、作物、阶段、浇水节点和成熟状态 |
| `farm_harvest_ledger` | 农场收获记录、奖励明细和 LP 收益 |
| `work_game_tasks` | 矿场、工厂、便利店岗位任务、阶段、照看节点和可领取状态 |
| `work_game_reward_ledger` | 打工小游戏奖励明细和 LP 收益 |

## 模块边界

前端模块：

```text
apps/desktop/src/features/pet/
apps/desktop/src/features/pet-store/
apps/desktop/src/features/pet-blind-box/
apps/desktop/src/features/pet-check-in/
apps/desktop/src/features/pet-redeem-key/
apps/desktop/src/features/work-market/
apps/desktop/src/features/farm-work/
apps/desktop/src/features/work-game/
apps/desktop/src/shared/services/tauri/pet.ts
apps/desktop/src/shared/services/tauri/farm.ts
apps/desktop/src/shared/services/tauri/workGame.ts
apps/desktop/src/shared/types/meeting.ts
```

Rust 模块：

```text
apps/desktop/src-tauri/src/local_pet.rs
apps/desktop/src-tauri/src/local_db/pet_leveling.rs
apps/desktop/src-tauri/src/local_db/pet_growth.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet_store.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet_blind_box.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet_check_in.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet_redeem_key.rs
apps/desktop/src-tauri/src/local_farm.rs
apps/desktop/src-tauri/src/local_work_game.rs
apps/desktop/src-tauri/src/infrastructure/repositories/farm.rs
apps/desktop/src-tauri/src/infrastructure/repositories/work_game.rs
apps/desktop/src-tauri/src/desktop_pet.rs
apps/desktop/src-tauri/src/desktop_pet/
```

职责：

- `local_pet.rs`：Tauri 命令入口和参数转换。
- `pet_leveling.rs`：255 级曲线、8 阶段映射、等级快照、奖励系数。
- `pet_growth.rs`：成长事件、LP 奖励、幂等键、里程碑计数。
- `repositories/pet.rs`：宠物档案、设置、事件、阶段装扮。
- `repositories/pet_store.rs`：商品目录、钱包、购买、每日免费领取、装备、使用、礼盒、成就自动解锁。
- `repositories/pet_blind_box.rs`：每日盲盒状态、开启、奖池、历史、重复补偿、空奖记录。
- `repositories/pet_check_in.rs`：签到状态、14 天奖励规则、补签窗口、补签票券消耗和签到记录。
- `repositories/pet_redeem_key.rs`：兑换 Key 格式校验、签名/HMAC 校验、奖励发放和兑换记录。
- `local_farm.rs`：牛马市场和农场种菜小游戏的 Tauri 命令入口。
- `local_work_game.rs`：矿场、工厂、便利店通用打工小游戏的 Tauri 命令入口。
- `repositories/farm.rs`：农场地块状态、作物配置、读取时补算、播种、浇水、收获和奖励结算。
- `repositories/work_game.rs`：岗位配置、任务状态、读取时补算、开始任务、照看、领奖和奖励结算。
- `desktop_pet.rs`：桌宠窗口生命周期、多开、拖拽、状态管理。
- `desktop_pet/*renderer.rs`：macOS / Windows 原生渲染。

## Tauri 命令

宠物相关命令注册在 `apps/desktop/src-tauri/src/lib.rs`：

- `get_pet_profile`
- `save_pet_profile`
- `get_pet_settings`
- `save_pet_settings`
- `list_pet_event_ledger`
- `list_pet_cosmetic_unlocks`
- `apply_pet_interaction`
- `apply_pet_workflow_event`
- `get_pet_store_state`
- `purchase_pet_store_item`
- `equip_pet_inventory_item`
- `unequip_pet_inventory_slot`
- `use_pet_inventory_item`
- `open_pet_gift_box`
- `get_pet_blind_box_state`
- `draw_pet_blind_box`
- `get_pet_daily_check_in_state`
- `claim_pet_daily_check_in`
- `repair_pet_daily_check_in`
- `redeem_pet_key`
- `list_pet_redeem_key_redemptions`
- `get_work_market_state`
- `get_farm_state`
- `plant_farm_crop`
- `water_farm_plot`
- `harvest_farm_plot`
- `list_farm_harvest_ledger`
- `get_work_game_state`
- `start_work_game_task`
- `care_work_game_task`
- `claim_work_game_task`
- `show_desktop_pet`
- `hide_desktop_pet`
- `open_extra_desktop_pet`
- `get_desktop_pet_status`
- `start_desktop_pet_drag`
- `prompt_pet_name`

诊断相关命令不属于宠物业务命令，但会参与桌宠排障：

- `get_diagnostics`
- `export_desktop_pet_diagnostic_log`

## 资源

桌宠动作资源：

```text
apps/desktop/src-tauri/resources/pet/
apps/desktop/src/assets/images/action/
```

当前前端动作资源按 `动作名/动作名_01.png` 到 `动作名/动作名_09.png` 命名，包含 `crush`、`defecate`、`drive`、`eat`、`gaming`、`pants`、`reading`、`rope`、`run`、`slack`、`sleep`、`snow`、`studying`、`toy`、`work` 共 15 组。前端预览和 Tauri 桌宠都会按数字序号排序播放，原生桌宠帧间隔为 300ms。

商店素材：

```text
apps/desktop/src/assets/images/shop/
```

`tauri.conf.json` 会把 `apps/desktop/src/assets/images/action/` 打包为 `pet` 资源，供原生桌宠窗口读取。开发模式下原生桌宠优先从前端动作资源目录加载，方便直接预览新帧；打包后从应用资源目录读取。

## 验收命令

宠物系统变更至少跑：

```bash
pnpm check
```

重点覆盖：

- 前端 typecheck 和生产构建。
- Rust fmt/test/clippy。
- release 版本、平台矩阵和安全基线检查。
- `pet_leveling` 等级边界、阶段阈值、满级处理、奖励倍率测试。
- `pet_store` 食物固定成长值、商品状态优先级测试。
- `pet_check_in` 签到周期、补签窗口、补签票券消耗和奖励发放测试。
- `pet_redeem_key` Key 规范化、哈希、签名/HMAC 校验和重复兑换测试。

## 历史记录

以下文件只保留设计背景和决策来源，不再作为当前规则来源：

- [桌面宠物设计记录](./ai/designs/2026-05-06-desktop-pet-design.md)
- [桌面宠物实现计划记录](./ai/plans/2026-05-06-desktop-pet-implementation-plan.md)
- [宠物商店玩法设计记录](./ai/designs/2026-05-21-pet-store-gameplay-design.md)
- [255 级成长策略记录](./ai/designs/2026-05-21-宠物255级成长生态策略.md)
