# Liberty 每日签到功能任务计划

## 目标

在 `福利` 分组中新增 `每日签到` 功能，作为 `每日盲盒` 之外的确定性本地福利入口。用户每天可领取一次签到奖励，系统需要记录签到事实，发放 LP、宠物成长值和商店物品奖励，并同步刷新宠物状态、LP 余额、仓库库存、事件流水和桌宠反馈。

本功能属于宠物系统的本地福利能力，必须保持 best-effort：签到失败不能影响会议任务创建、转写、总结、导出或结果查看。

## 范围

### 包含

- 新增 `福利 -> 每日签到` 菜单入口和 `/daily-check-in` 路由。
- 新增每日签到页面，展示今日状态、连续签到、里程碑进度、奖励预览和最近记录。
- 新增签到业务表，独立记录每天是否签到。
- 新增 Tauri 命令查询签到状态和领取签到奖励。
- 签到奖励写入宠物经济账本、宠物仓库和宠物事件流水。
- 签到奖励接入现有商店物品机制，支持发放食物、消耗道具、装扮、场景或徽章。
- 领取后刷新宠物状态、LP 余额、仓库库存、事件流水和桌宠反馈。
- 补齐中英文文案、类型定义、Rust 单测和前端类型检查。

### 不包含

- 不做补签卡。
- 不做云同步、账号权益、排行榜、活动配置后台或真钱付费。
- 不把每日签到做成随机抽奖玩法；随机福利继续留给每日盲盒。
- 不新增第二套物品体系；签到物品奖励必须来自现有宠物商店 catalog。
- 不把 `惊喜礼盒` 的开盒随机逻辑塞进签到模块；它属于商店道具能力，签到只负责发放或引导使用。

## 产品规则

### 签到频率

- 每个宠物每天只能签到一次。
- 日期口径使用本地自然日，存储格式为 `YYYY-MM-DD`。
- 同一天重复点击领取应返回当前签到状态，不重复发放奖励。

### 连续签到

- 如果上一次签到日期是昨天，则连续天数 `+1`。
- 如果上一次签到日期不是昨天，则连续天数重置为 `1`。
- 连续天数持续累计，不按 14 天重置。
- 页面首屏展示前 14 天里程碑奖励；第 15 天之后继续显示真实连续天数，并预留长期里程碑扩展。

### 奖励包规则

每日签到奖励以 `reward package` 形式定义。每个奖励包可包含：

- `lp`：LP 点数。
- `growthValue`：宠物成长值。
- `items`：来自宠物商店 catalog 的物品奖励。

V1 采用固定连续签到里程碑奖励包，先不做运营后台配置，但代码结构要按可配置奖励包设计，避免后续加月签、节日奖励、长期连续签到或活动奖励时重写模型。

| 连续签到天数 | LP | 成长值 | 商店物品奖励 |
| ---: | ---: | ---: | --- |
| 1 | 10 | 5 | - |
| 2 | 10 | 5 | - |
| 3 | 15 | 5 | `gift-box-tool` ×1 |
| 4 | 10 | 5 | - |
| 5 | 15 | 5 | `cupcake-food` ×1 |
| 6 | 10 | 5 | - |
| 7 | 20 | 5 | `clover-badge` ×1 |
| 8 | 10 | 5 | - |
| 9 | 10 | 5 | - |
| 10 | 20 | 5 | `gift-box-tool` ×1 |
| 11 | 10 | 5 | - |
| 12 | 15 | 5 | `ice-cream-cone-food` ×1 |
| 13 | 10 | 5 | - |
| 14 | 30 | 5 | `sprout-badge` ×1 |

前 14 天里程碑合计：`195 LP + 70 成长值 + 2 个惊喜礼盒 + 2 个商店食物奖励 + 2 个连续签到勋章奖励`。第 15 天之后不回到第 1 天，默认发放基础签到奖励，后续可以继续追加第 21、30、60、100 天等长期里程碑。

物品奖励必须复用宠物商店现有库存规则：

- 消耗品和食物：如果已拥有，数量叠加。
- 非消耗类物品：如果未拥有则入库；如果已拥有，转换为 LP 补偿或跳过，具体口径复用每日盲盒的重复补偿思路。
- 勋章：从现有商店勋章中抽出 `clover-badge` 和 `sprout-badge` 作为连续签到里程碑奖励。它们不再依赖普通活跃天数自动解锁，避免同一勋章被 `daily_open` 和 `daily_check_in` 两条链路重复授予。

第 14 天保留里程碑元数据，后续可以扩展为长期宝箱、专属徽章或更高价值道具。

### 勋章迁移口径

当前商店里有部分勋章通过 `active_days` 自动解锁。每日签到上线后，建议将以下勋章迁移到连续签到链路：

| 勋章 | 当前语义 | 新获取方式 |
| --- | --- | --- |
| `clover-badge` | 幸运草勋章，原累计活跃 3 天 | 连续签到第 7 天 |
| `sprout-badge` | 一起成长勋章，原累计活跃 7 天 | 连续签到第 14 天 |

迁移后需要同步更新商店 catalog 的描述和解锁条件：

- `clover-badge`：`连续签到 7 天后获得。`
- `sprout-badge`：`连续签到 14 天后获得。`
- 英文描述同步改为 `Earned after a 7-day check-in streak.` / `Earned after a 14-day check-in streak.`
- 自动解锁逻辑不能再通过 `active_days:3` / `active_days:7` 发放这两个勋章。

`friendship-badge` 仍保留在更长期的活跃/陪伴体系中，后续如果要做 14 天或 30 天连续签到奖励，再单独评估是否迁移。

### 惊喜礼盒规则调整

商店中的 `gift-box-tool`（惊喜礼盒）需要从普通庆祝反馈道具调整为福利型开盒道具：

- 每日可免费购买 `3` 个。
- 免费购买按本地自然日重置。
- 购买数量超过当日免费额度时，V1 直接禁止购买，不启用 LP 付费补买。
- 使用惊喜礼盒时，从商店 catalog 中随机开出物品。
- 奖池必须排除：
  - `item_type = pet` 的宠物本体。
  - `item_type = badge` 的勋章。
  - `gift-box-tool` 自身，避免开盒套娃。
  - 未开放、未达到等级/阶段条件或 `coming_soon` 的物品。
- 可开出的类型包括：装扮、场景、普通道具、食物。
- 消耗品/食物重复获得时叠加数量。
- 非消耗类物品重复获得时，按每日盲盒的重复补偿思路转换为 LP 或记录跳过。

惊喜礼盒和每日盲盒的边界：

| 能力 | 入口 | 次数 | 奖池 | 是否消耗库存 |
| --- | --- | ---: | --- | --- |
| 每日盲盒 | 福利页 | 每日 10 次 | 商店物品，现有规则 | 不消耗物品 |
| 惊喜礼盒 | 商店购买后在仓库使用 | 每日免费购买 3 个 | 商店物品，排除宠物和勋章 | 使用 1 个礼盒库存 |

惊喜礼盒需要新增独立的每日购买计数，不能复用 `pet_blind_box_draws`。建议新增商店每日限制表：

```sql
CREATE TABLE IF NOT EXISTS pet_store_daily_limits (
  pet_id TEXT NOT NULL,
  item_key TEXT NOT NULL,
  limit_date TEXT NOT NULL,
  free_claimed INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(pet_id, item_key, limit_date),
  FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
);
```

后端建议新增：

- `free_daily_limit` / `free_claimed_today` / `free_remaining_today` 字段到商品状态。
- `purchase_item_tx` 对 `gift-box-tool` 识别每日免费额度，价格计算为 `0`。
- `use_item_tx` 对 `gift-box-tool` 走 `open_gift_box_tx`，而不是普通 `store_tool` 文案。
- `open_gift_box_tx` 复用 `pet_store::grant_catalog_item_tx` 发放开奖物品。
- 开盒事件写入 `pet_event_ledger`，事件类型建议为 `gift_box_reward` / `gift_box_duplicate`。

前端需要在商店卡片、详情页和仓库使用弹窗展示：

- `每日免费 0/3` 或 `今日剩余 3/3`。
- 惊喜礼盒价格显示为 `每日免费`，不是 `120 LP`。
- 使用后展示开出的物品、重复补偿和库存变化。

## 数据设计

### 新增表

```sql
CREATE TABLE IF NOT EXISTS pet_daily_check_ins (
  id TEXT PRIMARY KEY,
  pet_id TEXT NOT NULL,
  check_in_date TEXT NOT NULL,
  streak_count INTEGER NOT NULL,
  cycle_day INTEGER NOT NULL,
  lp_reward INTEGER NOT NULL DEFAULT 0,
  growth_value INTEGER NOT NULL DEFAULT 0,
  item_rewards TEXT,
  claimed_at TEXT NOT NULL,
  metadata TEXT,
  UNIQUE(pet_id, check_in_date),
  FOREIGN KEY(pet_id) REFERENCES pet_profile(id) ON DELETE CASCADE
);
```

### 表职责

| 表 | 职责 |
| --- | --- |
| `pet_daily_check_ins` | 签到业务事实，回答某天是否签到、连续天数是多少 |
| `pet_economy_ledger` | LP 发放账本，通过唯一键保证奖励不重复入账 |
| `pet_inventory` | 商店物品奖励入库，继续复用现有库存和装备体系 |
| `pet_event_ledger` | 宠物事件展示和桌宠反馈，不作为签到事实来源 |
| `pet_profile` | 领取成长值后的等级、阶段和心情 |

### 幂等键

- 签到事实唯一键：`pet_id + check_in_date`。
- LP 发放唯一键：`source_type = daily_check_in`，`source_key = daily-check-in:{date}`。
- 物品奖励来源：`source = daily_check_in`，metadata 中记录 `itemKey`、`quantity`、`duplicate` 和 `compensationLp`。
- 事件流水来源：`event_source = daily_check_in`。

## 后端任务

### 1. Schema

- 在 `apps/desktop/src-tauri/src/local_db/schema.rs` 增加 `pet_daily_check_ins` 建表语句。
- 确认 `schema::apply_test_schema(&conn)` 覆盖新表，保证单测使用内存库即可运行。

### 2. Repository

新增文件：

```text
apps/desktop/src-tauri/src/infrastructure/repositories/pet_check_in.rs
```

建议方法：

- `get_check_in_state(conn) -> PetDailyCheckInState`
- `claim_daily_check_in(conn) -> PetDailyCheckInClaimResult`
- `latest_check_in_tx(tx, pet_id)`
- `check_in_exists_tx(tx, pet_id, check_in_date)`
- `insert_check_in_tx(tx, record)`
- `reward_package_for_cycle_day(cycle_day)`
- `grant_check_in_items_tx(tx, reward_package, source_key, now)`

领取逻辑必须在一个 SQLite transaction 内完成：

1. 确保默认宠物存在。
2. 计算本地日期、上一条签到记录、`streak_count` 和 `cycle_day`。
3. 如果今日已签到，直接返回状态，不重复发放奖励。
4. 插入 `pet_daily_check_ins`。
5. 更新 `pet_profile.experience`、等级快照、阶段和心情。
6. 调用 `pet_store::grant_reward_tx` 发放 LP。
7. 调用 `pet_store::grant_catalog_item_tx` 发放商店物品奖励。
8. 对已拥有的非消耗类物品，按统一重复补偿口径发放 LP 或记录跳过。
9. 写入 `pet_event_ledger`，metadata 使用结构化 JSON。
10. 提交事务并返回完整状态。

如果当前 `grant_catalog_item_tx` 不是公开方法，需要将其提升为 `pub(crate)`，或抽出更通用的 `grant_store_item_reward_tx`，供每日盲盒和每日签到共同使用。不要在签到模块里复制库存 upsert 逻辑。

### 3. Tauri 命令

在 `apps/desktop/src-tauri/src/local_pet.rs` 增加命令：

- `get_pet_daily_check_in_state`
- `claim_pet_daily_check_in`

在 `apps/desktop/src-tauri/src/lib.rs` 注册命令。

### 4. 类型

在 Rust model 和前端 `apps/desktop/src/shared/types/meeting.ts` 中新增：

```text
PetDailyCheckInState
PetDailyCheckInRecord
PetDailyCheckInReward
PetDailyCheckInItemReward
PetDailyCheckInClaimResult
```

状态建议包含：

- `today`
- `checkedInToday`
- `currentStreak`
- `cycleDay`
- `todayReward`
- `weeklyRewards`
- `recentRecords`
- `storeState`
- `wallet`
- `profile`

## 前端任务

### 1. 服务封装

在 `apps/desktop/src/shared/services/tauri/pet.ts` 增加：

- `getDailyCheckInState`
- `claimDailyCheckIn`

领取成功后调用 `notifyPetStateChanged("daily-check-in")`，并刷新宠物商店状态，以便 LP 和库存数量同步更新。

### 2. 路由和菜单

- `apps/desktop/src/app/router/index.ts` 新增 `/daily-check-in`。
- `apps/desktop/src/app/App.tsx` 在 `福利` 分组里新增 `每日签到`，放在 `每日盲盒` 之前。
- `apps/desktop/src/shared/services/ui/navIcons.ts` 复用现有图标或新增签到图标键。
- `apps/desktop/src/shared/i18n/messages/*` 增加中英文导航和页面文案。

### 3. 页面

新增：

```text
apps/desktop/src/features/pet-check-in/views/PetDailyCheckInView.tsx
apps/desktop/src/features/pet-check-in/views/PetDailyCheckInView.css
```

页面模块：

- 今日签到卡片：今日日期、是否已签到、主按钮。
- 连续状态：当前连续天数、下一次领取天数、下一里程碑。
- 奖励预览：LP、成长值、今日商店物品。
- 里程碑进度：显示前 14 天奖励、物品图标和已完成状态；第 15 天后连续天数继续累计。
- 最近记录：显示最近签到日期、连续天数、LP、成长值、物品奖励。

按钮状态：

- 未签到：`签到领取`
- 领取中：`领取中...`
- 已签到：`今日已签到`
- 失败：显示错误提示，可重试。

## 事件和桌宠反馈

签到事件建议写入：

```json
{
  "source": "daily_check_in",
  "date": "2026-05-26",
  "streakCount": 3,
  "cycleDay": 3,
  "finalLp": 15,
  "growthValue": 5,
  "items": [
    {
      "itemKey": "clover-badge",
      "quantity": 1,
      "duplicate": false,
      "compensationLp": 0
    }
  ]
}
```

需要同步更新：

- `apps/desktop/src/features/pet/services/petEventFormatters.ts`
- `apps/desktop/src/features/pet/services/petSprites.ts`
- `apps/desktop/src-tauri/src/local_pet.rs` 事件心情映射
- `apps/desktop/src-tauri/src/desktop_pet/behavior.rs`

建议心情：`cheerful`。

## 测试计划

### Rust 单测

覆盖：

- 首次签到生成 `streak_count = 1`。
- 同日重复签到不重复写入、不重复发 LP。
- 连续第 3 天发放 `15 LP` 和 `gift-box-tool`。
- 连续第 5 天发放 `15 LP` 和商店食物。
- 连续第 7 天发放 `20 LP` 和 `clover-badge`。
- 连续第 10 天发放 `20 LP` 和 `gift-box-tool`。
- 连续第 14 天发放 `30 LP` 和 `sprout-badge`。
- 第 5、12 天正确发放商店食物，消耗品数量可叠加。
- `clover-badge` 和 `sprout-badge` 不再由普通 `active_days` 自动解锁。
- 已拥有的非消耗类物品按重复补偿口径处理，不重复入库。
- `gift-box-tool` 每日免费购买限制为 3 个，超过后不能继续免费购买。
- 使用 `gift-box-tool` 能从商店 catalog 开出非宠物、非勋章、非礼盒自身的开放物品。
- 断签后 `streak_count` 重置为 `1`。
- 领取后 `pet_profile.experience` 增加成长值，等级快照保持一致。
- `pet_economy_ledger` 唯一键防重复。

### 前端检查

- `/Users/west/Library/pnpm/pnpm --dir apps/desktop typecheck`
- `/Users/west/Library/pnpm/pnpm --dir apps/desktop build:web`

### 全量质量门

变更完成后运行：

```bash
/Users/west/Library/pnpm/pnpm check
```

如涉及 release 或安装包验收，再跑 GitHub Actions `Build Desktop Bundles`。

## 验收标准

- `福利` 分组出现 `每日签到` 菜单。
- 进入 `/daily-check-in` 可看到今日签到状态、连续天数、里程碑奖励和最近记录。
- 第一次签到后 LP、成长值、连续天数立即刷新。
- 签到获得商店物品后，宠物商店和仓库库存立即刷新。
- 同一天重复点击不重复发放奖励。
- 连续第 3、5、7、10、14 天 LP 奖励正确。
- 第 3、10 天签到奖励获得惊喜礼盒，第 5、12 天获得商店食物。
- 连续第 7 天获得 `clover-badge`，连续第 14 天获得 `sprout-badge`。
- `clover-badge` / `sprout-badge` 的商店描述、详情页来源和事件流水文案都指向连续签到。
- 商店中 `gift-box-tool` 显示每日免费购买额度 `3`，价格不再表现为普通 LP 商品。
- 使用 `gift-box-tool` 时不会开出宠物和勋章，也不会开出礼盒自身。
- 断签后连续天数重置。
- 宠物事件流水显示为本地化文案，不泄露 raw JSON。
- 桌宠能收到签到反馈。
- 中英文文案完整。
- Rust 单测、前端 typecheck、生产构建通过。

## 实施顺序

1. 增加 schema、Rust 类型和 `pet_check_in` repository。
2. 增加 Tauri 命令并注册。
3. 增加前端类型和 service 方法。
4. 增加路由、菜单和 i18n 文案。
5. 实现签到页面和样式。
6. 调整惊喜礼盒商店规则：每日免费购买 3 个，使用时从限定奖池开出商店物品。
7. 接入商店物品奖励展示、库存刷新和重复补偿展示。
8. 接入宠物事件格式化和桌宠反馈。
9. 补 Rust 单测。
10. 运行 typecheck、build、Rust 相关测试和全量质量门。
11. 手动验收本地 SQLite 中签到记录、LP 余额、物品库存、事件流水和 UI 状态。

## 风险和约束

- 本地日期必须统一从后端生成，避免前端和 Rust 日期不一致。
- 签到领取必须在事务内完成，避免写入签到记录成功但奖励失败。
- 事件流水只用于展示，不应反向承担签到状态判断。
- `pet_economy_ledger` 的唯一键必须保留，避免并发点击导致重复 LP。
- 商店物品奖励必须复用商店 catalog 和库存方法，不允许在签到模块维护第二套 item key 或库存写入逻辑。
- 非消耗类物品的重复处理要和每日盲盒保持一致，否则用户会看到同一件商品在不同福利入口表现不一致。
- 惊喜礼盒的每日免费购买次数需要独立计数，不要和每日盲盒次数混在一起。
- 惊喜礼盒开奖奖池要显式排除宠物、勋章和礼盒自身，避免破坏宠物本体/成就勋章的获取边界。
- 宠物奖励失败不能影响会议主流程；但签到页面本身需要清晰提示领取失败原因。
