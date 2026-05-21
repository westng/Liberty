# Liberty 宠物系统说明

## 定位

Liberty 的宠物系统是桌面端本地陪伴能力。它围绕真实会议工作流记录成长、LP、事件和装备状态，让用户完成任务、转写、AI 总结和导出时获得轻量反馈。

宠物系统不属于会议处理主链路。它不能阻塞任务创建、转写、总结、导出或结果查看；主窗口初始化时会尝试同步桌宠状态，但失败后只记录错误并继续保持应用可用。

## 当前页面

### 宠物中心

入口：`/pet`

当前页面能力：

- 展示宠物名称、等级、经验、阶段、心情、阶段进度和已解锁装扮数量。
- 支持修改宠物名称，名称通过原生输入弹窗处理。
- 支持点击、抚摸、投喂、鼓励四类互动。
- 展示最近宠物事件，包括工作事件、互动事件、商店装备和食物使用。
- 配置桌宠行为：启用桌宠、始终置顶、静音提示、专注模式、主动程度。
- 支持多开一个桌面宠物窗口。

### 宠物商店

入口：`/pet-store`

当前页面能力：

- 展示 LP 余额、等级、库存数量、可购买数量和锁定数量。
- 支持商店和个人仓库两个视图。
- 支持按全部、宠物、装扮、场景、道具、食物、徽章筛选。
- 支持购买商品、装备商品、取消装备槽位、使用食物和消耗道具。
- 食物卡展示固定成长值，使用后增加宠物经验并写入事件流水。
- 商品卡可打开独立商品详情窗口。

### 商品详情

入口：`/pet-store-item?itemKey=...`

当前页面能力：

- 展示商品图片、分类、羁绊阶梯、状态、价格、持有数量、等级门槛、阶段门槛和里程碑门槛。
- 对已有商品展示来源：默认、成长、购买或成就。
- 对食物展示成长值。
- 商品图支持翻转预览。

## 成长规则

当前代码仍使用简单线性等级：

```text
level = floor(experience / 20) + 1
stageProgress = experience % 20
```

当前阶段：

| 阶段 | 条件 |
| --- | --- |
| `baby` | Lv.1-3 |
| `growing` | Lv.4-7 |
| `mature` | Lv.8+ |

当前交互事件：

| 事件 | 成长值 | 心情 |
| --- | ---: | --- |
| `tap` | +1 | `cheerful` |
| `pet` | +1 | `cheerful` |
| `feed` | +1 | `proud` |
| `encourage` | +1 | `cheerful` |

互动事件有每日上限，当前由 `MAX_DAILY_INTERACTION_PER_SOURCE` 控制。

## 工作奖励

会议工作流通过 `apply_pet_workflow_event` 触发宠物成长和 LP 奖励。

当前成长值：

| 工作事件 | 成长值 | 心情 |
| --- | ---: | --- |
| `daily_open` | +2 | `idle` |
| `job_created` | +5 | `cheerful` |
| `transcription_started` | +3 | `excited` |
| `transcription_completed` | +12 | `proud` |
| `ai_summary_completed` | +10 | `proud` |
| `export_completed` | +6 | `proud` |

当前 LP：

| 工作事件 | LP |
| --- | ---: |
| `daily_open` | +5 |
| `job_created` | +8 |
| `transcription_started` | +3 |
| `transcription_completed` | +18 |
| `ai_summary_completed` | +15 |
| `export_completed` | +10 |
| 互动事件 | +1 |

奖励通过 `source_type + source_key` 保持幂等。同一个任务的同一工作事件不会重复发放完整奖励；互动事件会生成独立事件键。

## 商店商品

商品目录当前写在 Rust 代码中：

```text
apps/desktop/src-tauri/src/infrastructure/repositories/pet_store.rs
```

商品类型：

| 类型 | 槽位/用途 |
| --- | --- |
| `pet` | 当前宠物槽 |
| `cosmetic` | 配饰槽 |
| `theme` | 场景槽 |
| `tool` | 消耗品 |
| `food` | 消耗品，使用后增加成长值 |
| `badge` | 徽章槽，多数通过里程碑自动获得 |

商品状态：

| 状态 | 含义 |
| --- | --- |
| `equipped` | 已装备 |
| `owned` | 已拥有 |
| `available` | 可购买 |
| `insufficient` | LP 不足 |
| `locked` | 等级、阶段或里程碑未满足 |
| `achievement` | 成就类商品，达成条件后自动入库 |
| `coming_soon` | 商品未开放 |

食物成长值不是固定表，而是由价格、羁绊阶梯和阶段门槛计算，范围当前限制在 2 到 12。

## 本地数据表

宠物系统使用 SQLite 表：

| 表 | 作用 |
| --- | --- |
| `pet_profile` | 宠物名称、等级、经验、阶段、心情 |
| `pet_settings` | 桌宠启用、置顶、静音、专注模式、主动程度、窗口位置 |
| `pet_cosmetic_unlocks` | 阶段成长解锁的旧装扮记录 |
| `pet_event_ledger` | 互动、工作、装备、食物等事件流水 |
| `pet_wallets` | LP 余额、累计获得、累计消耗 |
| `pet_inventory` | 个人仓库、数量、装备状态、来源 |
| `pet_economy_ledger` | LP 获取和消耗流水，带唯一键防重复 |
| `pet_milestone_counters` | 任务、转写、总结、导出、活跃天数等计数 |

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
```

前端通过 `apps/desktop/src/shared/services/tauri/pet.ts` 调用 Tauri 命令。

## Rust 模块

```text
apps/desktop/src-tauri/src/local_pet.rs
apps/desktop/src-tauri/src/local_db/pet_growth.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet.rs
apps/desktop/src-tauri/src/infrastructure/repositories/pet_store.rs
apps/desktop/src-tauri/src/desktop_pet.rs
apps/desktop/src-tauri/src/desktop_pet/
```

职责划分：

- `local_pet.rs`：Tauri 命令入参和调用入口。
- `pet_growth.rs`：成长事件、LP 奖励、幂等键和里程碑计数。
- `repositories/pet.rs`：宠物档案、设置、事件和阶段装扮。
- `repositories/pet_store.rs`：商品目录、钱包、购买、装备、使用、成就自动解锁。
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
- 当前没有真钱支付、抽卡、交易、排行榜或云同步。
- 当前等级曲线尚未实现 255 级分段曲线；255 级方案仍是设计文档，不是当前代码行为。
- 宠物系统对会议主流程是 best-effort：失败不应影响会议任务。

## 关联文档

- [桌面宠物设计](./superpowers/specs/2026-05-06-desktop-pet-design.md)
- [宠物商店玩法设计](./superpowers/specs/2026-05-21-pet-store-gameplay-design.md)
- [宠物 255 级成长生态策略](./superpowers/specs/2026-05-21-宠物255级成长生态策略.md)
