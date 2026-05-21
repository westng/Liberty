# Liberty 宠物系统说明

## 权威口径

宠物系统的产品、经济、等级和奖励规则以 [宠物 255 级成长生态策略](./superpowers/specs/2026-05-21-宠物255级成长生态策略.md) 为准。旧的 3 阶段线性等级、按价格推导食物成长值、付费抽卡或可购买次数等描述均不再作为当前口径。

本文只说明当前项目中的宠物系统结构和实现边界。

## 定位

Liberty 的宠物系统是桌面端本地陪伴能力。它围绕真实会议工作流记录成长值、LP、事件、库存、装备和每日福利状态，让用户完成任务、转写、AI 总结和导出时获得轻量反馈。

宠物系统不属于会议处理主链路。它不能阻塞任务创建、转写、总结、导出或结果查看；主窗口初始化时会尝试同步桌宠状态，但失败后只记录错误并继续保持应用可用。

## 当前页面

### 宠物中心

入口：`/pet`

当前页面能力：

- 展示宠物名称、等级、累计成长值、当前阶段、心情、本级进度和下一阶段。
- 满级时展示 `Lv.255 · 不离不弃`，继续展示累计成长值，不再展示下一级进度。
- 支持修改宠物名称，名称通过原生输入弹窗处理。
- 支持点击、抚摸、投喂、鼓励四类互动。
- 展示最近宠物事件，包括工作事件、互动事件、商店装备、食物使用和盲盒事件。
- 配置桌宠行为：启用桌宠、始终置顶、静音提示、专注模式、主动程度。
- 支持多开一个桌面宠物窗口。

### 宠物商店

入口：`/pet-store`

当前页面能力：

- 展示 LP 余额、等级、库存数量、可购买数量和锁定数量。
- 支持商店和个人仓库两个视图。
- 支持按全部、宠物、装扮、场景、道具、食物、徽章筛选。
- 支持购买商品、装备商品、取消装备槽位、使用食物和消耗道具。
- 食物卡展示固定成长值，使用后按 `growthValue × quantity` 增加累计成长值并写入事件流水。
- 商品卡可打开独立商品详情窗口。

### 每日盲盒

入口：`/pet-blind-box`

当前页面能力：

- 每天最多开启 10 次，按本地自然日重置。
- 开启不消耗 LP，不提供付费购买次数，不关联充值或真钱权益。
- 奖池来自宠物商店内容，但排除宠物本体。
- 奖池可完整展示，并包含“什么都没抽中”的空奖。
- 消耗品重复获得时叠加数量。
- 已拥有的非消耗品重复获得时转为少量 LP 补偿。
- 每次开启写入本地盲盒历史，并通过宠物事件流水触发轻量反馈。

每日盲盒是本地免费福利，不是付费抽卡系统。

### 商品详情

入口：`/pet-store-item?itemKey=...`

当前页面能力：

- 展示商品图片、分类、羁绊阶梯、状态、价格、持有数量、等级门槛、阶段门槛和里程碑门槛。
- 对已有商品展示来源：默认、成长、购买、成就、每日盲盒或盲盒重复补偿。
- 对食物展示固定成长值。
- 商品图支持翻转预览。

## 成长规则

当前成长系统采用 255 级累计经验曲线。`experience` 表示累计成长值，不是本级内经验。

等级快照由 Rust 后端统一计算并返回给前端，前端不再自行计算 `experience / 20` 或 `experience % 20`。

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

当前阶段：

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

当前交互事件：

| 事件 | 基础成长值 | 基础 LP | 心情 |
| --- | ---: | ---: | --- |
| `tap` | +1 | +1 | `cheerful` |
| `pet` | +1 | +1 | `cheerful` |
| `feed` | +1 | +1 | `proud` |
| `encourage` | +1 | +1 | `cheerful` |

互动事件有每日上限，当前由 `MAX_DAILY_INTERACTION_PER_SOURCE` 控制。超过上限后仍可播放动作或提示，但不继续发放成长值和 LP。

## 工作奖励

会议工作流通过 `apply_pet_workflow_event` 触发宠物成长和 LP 奖励。

基础奖励：

| 工作事件 | 基础成长值 | 基础 LP | 心情 |
| --- | ---: | ---: | --- |
| `daily_open` | +2 | +5 | `idle` |
| `job_created` | +5 | +8 | `cheerful` |
| `transcription_started` | +2 | +3 | `excited` |
| `transcription_completed` | +12 | +18 | `proud` |
| `ai_summary_completed` | +10 | +15 | `proud` |
| `export_completed` | +6 | +10 | `proud` |

工作成长值会按当前等级应用成长值阶段系数，LP 会按 LP 阶段系数小幅提升。食物成长值固定，不参与这些阶段系数。

奖励通过 `source_type + source_key` 保持幂等。同一个任务的同一工作事件不会重复发放完整奖励；每日打开按本地自然日去重；互动事件会生成独立事件键。

## 商店商品

商品目录当前写在 Rust 代码中：

```text
apps/desktop/src-tauri/src/infrastructure/repositories/pet_store.rs
```

商品类型：

| 类型 | 槽位/用途 |
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

食物成长值来自显式 `growthValue` 字段。商店卡片、商品详情、个人仓库、使用确认、事件流水和宠物中心最近事件必须展示同一个成长值。

徽章原则上不售卖，主要通过里程碑自动入库。若未来出现可购买纪念章，必须和成就徽章视觉区分。

## 本地数据表

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
- `show_desktop_pet`
- `hide_desktop_pet`
- `open_extra_desktop_pet`
- `get_desktop_pet_status`
- `start_desktop_pet_drag`
- `prompt_pet_name`

## 前端模块

```text
apps/desktop/src/features/pet/
├─ services/petDialogues.ts
├─ services/petEventFormatters.ts
├─ services/petSprites.ts
├─ stores/usePetStore.ts
└─ views/PetManagementView.tsx

apps/desktop/src/features/pet-store/
├─ services/petStorePresentation.ts
└─ views/
   ├─ PetStoreView.tsx
   └─ PetStoreItemDetailView.tsx

apps/desktop/src/features/pet-blind-box/
└─ views/
   ├─ PetBlindBoxView.tsx
   └─ PetBlindBoxThreeStage.tsx
```

前端通过 `apps/desktop/src/shared/services/tauri/pet.ts` 调用 Tauri 命令。

## Rust 模块

```text
apps/desktop/src-tauri/src/local_pet.rs
apps/desktop/src-tauri/src/local_db/pet_growth.rs
apps/desktop/src-tauri/src/local_db/pet_leveling.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet_store.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet_blind_box.rs
apps/desktop/src-tauri/src/desktop_pet.rs
apps/desktop/src-tauri/src/desktop_pet/
```

职责划分：

- `local_pet.rs`：Tauri 命令入参和调用入口。
- `pet_leveling.rs`：255 级曲线、8 阶段映射、等级快照和奖励系数。
- `pet_growth.rs`：成长事件、LP 奖励、幂等键和里程碑计数。
- `repositories/pet.rs`：宠物档案、设置、事件和阶段装扮。
- `repositories/pet_store.rs`：商品目录、钱包、购买、装备、使用、成就自动解锁。
- `repositories/pet_blind_box.rs`：每日盲盒状态、开启、奖池、历史、重复补偿和空奖记录。
- `desktop_pet.rs`：桌宠窗口生命周期、多开、拖拽和状态管理。
- `desktop_pet/*renderer.rs`：macOS / Windows 原生渲染。

## 桌宠资源

桌宠动作资源位于：

```text
apps/desktop/src-tauri/resources/pet/
```

前端预览和打包资源使用：

```text
apps/desktop/src/assets/images/action/
apps/desktop/src/assets/images/shop/
```

`tauri.conf.json` 会把 `apps/desktop/src/assets/images/action/` 打包为 `pet` 资源，供原生桌宠窗口读取。

## 当前边界

- 宠物系统是本地能力，不依赖账号。
- LP 是本地奖励点数，不是现实货币。
- 当前没有真钱支付、充值、交易、排行榜或云同步。
- 每日盲盒是本地免费福利，不消耗 LP、不出售次数、不关联付费概率掉落。
- 宠物系统对会议主流程是 best-effort：失败不应影响会议任务。

## 关联文档

- [桌面宠物设计](./superpowers/specs/2026-05-06-desktop-pet-design.md)
- [宠物商店玩法设计](./superpowers/specs/2026-05-21-pet-store-gameplay-design.md)
- [宠物 255 级成长生态策略](./superpowers/specs/2026-05-21-宠物255级成长生态策略.md)
