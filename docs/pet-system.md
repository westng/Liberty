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
| 商品详情 | `/pet-store-item?itemKey=...` | 商品图片、分类、状态、价格、持有数量、门槛、来源、食物成长值 |

每日盲盒不是付费抽卡系统：不消耗 LP，不出售次数，不关联充值、真钱权益或付费概率掉落。

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

## 商店和盲盒

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

## 模块边界

前端模块：

```text
apps/desktop/src/features/pet/
apps/desktop/src/features/pet-store/
apps/desktop/src/features/pet-blind-box/
apps/desktop/src/shared/services/tauri/pet.ts
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
apps/desktop/src-tauri/src/desktop_pet.rs
apps/desktop/src-tauri/src/desktop_pet/
```

职责：

- `local_pet.rs`：Tauri 命令入口和参数转换。
- `pet_leveling.rs`：255 级曲线、8 阶段映射、等级快照、奖励系数。
- `pet_growth.rs`：成长事件、LP 奖励、幂等键、里程碑计数。
- `repositories/pet.rs`：宠物档案、设置、事件、阶段装扮。
- `repositories/pet_store.rs`：商品目录、钱包、购买、装备、使用、成就自动解锁。
- `repositories/pet_blind_box.rs`：每日盲盒状态、开启、奖池、历史、重复补偿、空奖记录。
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
- `get_pet_blind_box_state`
- `draw_pet_blind_box`
- `set_pet_store_item_detail_item`
- `get_pet_store_item_detail_item`
- `show_desktop_pet`
- `hide_desktop_pet`
- `open_extra_desktop_pet`
- `get_desktop_pet_status`
- `start_desktop_pet_drag`
- `prompt_pet_name`

## 资源

桌宠动作资源：

```text
apps/desktop/src-tauri/resources/pet/
apps/desktop/src/assets/images/action/
```

商店素材：

```text
apps/desktop/src/assets/images/shop/
```

`tauri.conf.json` 会把 `apps/desktop/src/assets/images/action/` 打包为 `pet` 资源，供原生桌宠窗口读取。

## 验收命令

宠物系统变更至少跑：

```bash
/Users/west/Library/pnpm/pnpm check
```

重点覆盖：

- 前端 typecheck 和生产构建。
- Rust fmt/test/clippy。
- release 版本、平台矩阵和安全基线检查。
- `pet_leveling` 等级边界、阶段阈值、满级处理、奖励倍率测试。
- `pet_store` 食物固定成长值、商品状态优先级测试。

## 历史记录

以下文件只保留设计背景和决策来源，不再作为当前规则来源：

- [桌面宠物设计记录](./superpowers/specs/2026-05-06-desktop-pet-design.md)
- [桌面宠物实现计划记录](./superpowers/plans/2026-05-06-desktop-pet-implementation-plan.md)
- [宠物商店玩法设计记录](./superpowers/specs/2026-05-21-pet-store-gameplay-design.md)
- [255 级成长策略记录](./superpowers/specs/2026-05-21-宠物255级成长生态策略.md)
