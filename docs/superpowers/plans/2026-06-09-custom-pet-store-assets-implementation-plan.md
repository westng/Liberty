# Liberty 自定义商店资产导入执行计划

## 目标

支持用户把本机图片作为自定义商品追加到宠物商店中。自定义商品与内置商品并列展示、购买、入库、装备或使用，但不覆盖、不修改、不迁移现有内置商店目录。

第一期只做本机追加目录：

- 内置目录仍由 `pet_store.rs` 的静态商品种子提供。
- 自定义目录写入本机 SQLite 和应用数据目录。
- 商店状态读取时合并 `内置商品 + 自定义商品`。
- 库存、LP 流水、装备状态、食物成长值继续复用现有宠物经济系统。

## 非目标

- 不把用户图片复制到 `apps/desktop/src/assets/images/shop/`。
- 不覆盖内置商品的图片、名称、价格或门槛。
- 不支持同步到云端或跨设备分发。
- 不支持导入完整桌宠动作帧、宠物本体或动画皮肤。
- 不把自定义商品默认加入盲盒或礼盒奖池。
- 不开放自定义 `seed` 种子和农场作物配置。

## 内置资产隔离规则

用户导入资产必须作为追加数据存在，不能影响任何内置资产。

- 导入、更新、删除自定义商品时，只允许写入 `pet_custom_store_items` 和 app data 下的 `pet-store/custom-assets/`。
- 禁止写入、移动、删除或重命名 `apps/desktop/src/assets/images/shop/` 下的内置图片。
- 禁止修改 `CATALOG_SEEDS` 中已有内置商品的 `item_key`、`asset_key`、名称、价格、门槛、排序和启用状态。
- 自定义 `item_key` 必须使用 `custom:` 命名空间；后端导入命令必须拒绝任何与内置商品同 key 的输入。
- 自定义 `asset_key` 也必须使用自定义命名空间，不能复用内置 `asset_key`，避免前端图片解析误命中内置素材。
- 自定义图片解析必须优先走 `assetUrl`；内置图片解析继续走 `shopImageUrlMap[assetKey]`。两条路径不能互相覆盖缓存。
- 删除自定义商品只能软删除自定义记录，不能删除内置商品、内置图片或库存中的内置物品。
- 验收时必须对比导入前后的内置商品数量、内置 `item_key` 集合和内置图片文件列表，确认完全不变。

## 一期支持范围

| 商品类型 | 槽位 | 行为 |
| --- | --- | --- |
| `food` | `consumable` | 可购买、可叠加、可使用，按 `growthValue * quantity` 增加成长值 |
| `tool` | `consumable` | 可购买、可叠加；第一期默认只收藏展示，不提供特殊使用效果 |
| `cosmetic` | `accessory` | 可购买、可装备，沿用同槽位互斥规则 |
| `theme` | `scene` | 可购买、可装备，沿用同槽位互斥规则 |

暂不支持：

- `pet`
- `badge`
- `seed`
- `none`

## 用户导入体验

导入流程要面向普通用户，不能要求用户理解商品表字段。

第一期默认采用极简导入：

1. 用户选择图片。
2. 用户选择类型：食物、道具、装扮、场景。
3. 系统根据图片文件名生成商品名、说明、价格、成长值和排序。
4. 用户确认后直接追加到商店。

普通用户默认只需要提供：

- 图片。
- 商品类型。
- 可选修改商品名称。

高级字段放在“更多设置”里，默认折叠：

- 中文说明。
- 英文名称。
- 英文说明。
- LP 价格。
- 食物成长值。
- 是否上架。

默认值建议：

| 类型 | 默认价格 | 默认成长值 | 默认说明 |
| --- | --- | --- | --- |
| `food` | 30 LP | 8 | `用户导入的自定义食物。` |
| `tool` | 30 LP | 0 | `用户导入的自定义道具。` |
| `cosmetic` | 120 LP | 0 | `用户导入的自定义装扮。` |
| `theme` | 180 LP | 0 | `用户导入的自定义场景。` |

名称生成规则：

- 优先使用文件名去掉扩展名。
- 把 `_`、`-` 替换为空格。
- 如果文件名为空或不可读，默认使用 `自定义商品`。
- 用户可在确认前直接改名。

## 数据模型

### 新增表：`pet_custom_store_items`

建议字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `item_key` | `TEXT PRIMARY KEY` | 自定义商品唯一编码，建议 `custom:<uuid>` |
| `item_type` | `TEXT NOT NULL` | `food`、`tool`、`cosmetic`、`theme` |
| `slot` | `TEXT NOT NULL` | `consumable`、`accessory`、`scene` |
| `name_zh` | `TEXT NOT NULL` | 中文名称 |
| `name_en` | `TEXT NOT NULL` | 英文名称；可默认等于中文名称 |
| `description_zh` | `TEXT NOT NULL` | 中文说明 |
| `description_en` | `TEXT NOT NULL` | 英文说明；可默认等于中文说明 |
| `rarity` | `TEXT NOT NULL` | 第一期默认 `custom` 或 `first_meet` |
| `price_lp` | `INTEGER NOT NULL` | LP 价格，最小 0 |
| `level_gate` | `INTEGER NOT NULL` | 第一期默认 1 |
| `stage_gate` | `TEXT NOT NULL` | 第一期默认空字符串 |
| `milestone_gate` | `TEXT NOT NULL` | 第一期默认空字符串 |
| `asset_key` | `TEXT NOT NULL` | 与 `item_key` 对应的自定义素材键 |
| `asset_path` | `TEXT NOT NULL` | app data 下的相对路径 |
| `growth_value` | `INTEGER NOT NULL` | 食物成长值；非食物为 0 |
| `enabled` | `INTEGER NOT NULL` | 是否上架 |
| `include_in_random_pool` | `INTEGER NOT NULL` | 是否进入随机奖池；第一期默认 0 |
| `sort_order` | `INTEGER NOT NULL` | 排序值，自定义商品放在内置目录之后 |
| `deleted_at` | `TEXT` | 软删除时间 |
| `created_at` | `TEXT NOT NULL` | 创建时间 |
| `updated_at` | `TEXT NOT NULL` | 更新时间 |

### 约束

- `item_key` 必须使用自定义命名空间，不能与内置 `CATALOG_SEEDS` 冲突。
- `asset_key` 必须使用自定义命名空间，不能与内置图片键冲突。
- 删除采用软删除。已进入库存的商品必须继续能解析名称、类型、槽位和图片。
- `enabled = 0` 表示下架，不再展示在商店可购列表，但历史库存仍可展示。
- `deleted_at IS NOT NULL` 表示从商店售卖列表隐藏；库存、装备、事件和历史记录解析必须仍能读取这条软删除记录。
- 商店售卖列表查询过滤 `deleted_at IS NULL`。
- 库存解析查询不能过滤软删除记录，只能把软删除商品标记为 `deleted = true`，避免已持有商品变成空白卡片。

## 文件存储

用户导入图片复制到 Tauri app data 目录，例如：

```text
<app_data>/pet-store/custom-assets/<safe_item_key>.png
```

执行规则：

- 支持输入格式：`png`、`jpg`、`jpeg`、`webp`。
- 最大文件大小：建议 5 MB。
- 最大尺寸：建议 2048 x 2048。
- 导入后统一保存为 PNG，便于前端和原生能力稳定读取。
- Rust 侧需要确认 `image` 依赖在当前目标平台启用 `png`、`jpeg`、`webp` 解码能力；不能只依赖 Windows target 下的 `png` feature。
- 文件复制成功后再写数据库。
- 数据库写入失败时清理刚复制的图片。
- 删除自定义商品时第一期不物理删除图片，只软删除记录，避免库存历史图片丢失。
- 前端不能直接把本机文件路径塞进 `<img>`。后端返回 app data 相对路径或绝对路径后，前端必须通过 Tauri `convertFileSrc` 转成可加载的 asset URL；同时确认 CSP 保留 `asset:` / `http://asset.localhost` 图片来源。

## 图片质量校验

商店资产导入必须严格校验图片质量，不能只检查文件扩展名。

### 基础格式

- 输入可以是 `png`、`jpg`、`jpeg`、`webp`。
- 最终入库文件必须是 PNG。
- 最终 PNG 必须有真实 alpha 通道。
- 最终 PNG 不能是白底、黑底、棋盘格底或其他伪透明背景。
- 如果原图没有透明通道，第一期不自动抠图，直接提示用户更换透明 PNG。

### 透明度

导入后需要读取像素 alpha：

- 图片至少存在一批 `alpha = 0` 或接近透明的背景像素。
- 主体区域不能整体半透明。
- 图片四个角必须接近透明。
- 如果四个角存在明显不透明像素，判定为背景未清理。

建议阈值：

- `alpha <= 8` 视作透明背景。
- `alpha >= 240` 视作主体不透明。
- 透明像素占比应大于 20%。
- 主体不透明像素占比不应小于 5%，避免导入空图。

### 主体居中和边距

导入后根据不透明像素计算主体包围盒：

- 主体包围盒必须位于画布中间区域。
- 主体中心点相对画布中心的偏移不超过画布宽高的 8%。
- 主体四周必须留出安全边距。
- 主体不能贴边、裁切或超出画布。

建议边距：

- 左、右、上、下至少各保留画布尺寸的 8% 透明边距。
- 主体最大宽度不超过画布宽度的 84%。
- 主体最大高度不超过画布高度的 84%。
- 主体最小宽度和高度不低于画布尺寸的 30%，避免图片太小。

### 画布比例和尺寸

- 推荐方形画布。
- 第一期开启严格模式：最终 PNG 统一归一化为 512 x 512。
- 如果输入不是方形，系统可以把图片等比放入 512 x 512 透明画布中。
- 归一化后仍必须重新执行透明度、主体居中和边距校验。

### 失败提示

校验失败时不要静默修正，给用户明确原因：

- 图片需要透明背景。
- 物体太靠边，请留出透明边距。
- 物体没有居中。
- 物体太小或太大。
- 图片像是白底/黑底/棋盘格背景，不是真透明。

第一期只做确定性校验和等比归一化，不做 AI 抠图、不做自动重绘、不猜测用户图片主体。

## 后端任务

### 1. Schema 迁移

- 在 `apps/desktop/src-tauri/src/local_db/schema.rs` 增加 `pet_custom_store_items` 建表 SQL。
- 增加必要索引：
  - `idx_pet_custom_store_items_enabled`
  - `idx_pet_custom_store_items_deleted`
  - `idx_pet_custom_store_items_sort`
- 确保迁移幂等，旧数据库升级不影响现有宠物表。

### 2. 自定义目录仓储

在 `apps/desktop/src-tauri/src/infrastructure/repositories/pet_store.rs` 中新增或拆分以下能力：

- `list_custom_catalog_items_tx`
- `load_custom_catalog_item_tx`
- `insert_custom_catalog_item_tx`
- `update_custom_catalog_item_tx`
- `soft_delete_custom_catalog_item_tx`
- `find_any_catalog_item_tx`
- `find_inventory_catalog_item_tx`

注意：

- `catalog_items()` 保持只返回内置商品。
- 新增 `catalog_items_for_state(conn)` 或等价函数，返回 `内置 + 自定义`。
- 购买、使用、装备等用户行为应改用能读取自定义商品的查找函数。
- 里程碑自动解锁、默认宠物初始化、徽章奖励等系统逻辑继续只使用内置目录。
- `find_inventory_catalog_item_tx` 必须能读取软删除自定义商品，用于库存、装备、事件和详情页历史解析。

### 2.1 Catalog 查找链路矩阵

| 场景 | 查找范围 | 说明 |
| --- | --- | --- |
| 商店售卖列表 | 内置 + 未删除自定义 | `enabled = 0` 仍可展示为下架态，`deleted_at IS NOT NULL` 不进入售卖列表 |
| 库存列表 | 内置 + 全部自定义 | 包含软删除自定义商品，避免历史库存丢失展示信息 |
| 商品详情页 | 内置 + 全部自定义 | 软删除商品可显示“已删除/已下架”，但不能再次购买 |
| 购买 | 内置 + 未删除自定义 | 自定义商品必须 `enabled = 1` 才能购买 |
| 使用 | 内置 + 全部自定义 | 自定义 `tool` 必须在扣库存前拒绝使用 |
| 装备 | 内置 + 全部自定义 | 已持有的自定义 `cosmetic/theme` 即使下架或软删除也可继续装备 |
| 礼盒奖池 | 仅内置 | 第一期不纳入自定义商品 |
| 盲盒奖池 | 仅内置 | 第一期不纳入自定义商品 |
| 每日签到奖励 | 仅内置 | 签到奖励配置仍依赖内置商品 |
| 兑换 Key 奖励 | 仅内置 | 第一期运营 Key 不发放本机自定义商品 |
| 农场/打工奖励 | 仅内置 | 第一期小游戏奖励不发放本机自定义商品 |
| 阶段/里程碑自动解锁 | 仅内置 | 保持系统成就规则稳定 |

现有无连接的 `find_catalog_item(item_key)` 和 `find_catalog_item_by_key(item_key)` 应保留为内置-only 工具函数。需要读取自定义商品的流程必须显式使用带数据库连接或事务的 `find_any_catalog_item_tx` / `find_inventory_catalog_item_tx`，避免误把自定义商品当成不存在。

### 3. 商店状态合并

调整 `store_state(conn, profile)`：

- 读取现有库存。
- 读取内置商品。
- 读取未删除的自定义商品。
- 合并并排序。
- 对每个商品复用现有 `catalog_item_state` 状态计算。

排序建议：

1. 内置商品按原 `sort_order`。
2. 自定义商品整体排在内置商品之后。
3. 自定义商品内部按 `updated_at DESC` 或 `sort_order ASC`。

### 4. 购买、使用、装备

调整以下流程，使其支持自定义商品：

- `purchase_item_tx`
- `grant_catalog_item_tx`
- `equip_item_tx`
- `use_item_tx`
- `open_gift_box_tx` 的奖池读取保持内置优先，第一期不纳入自定义商品。

规则：

- `food` 使用后增加成长值，并写宠物事件流水。
- `cosmetic` 和 `theme` 装备时复用同槽位互斥规则。
- `tool` 第一期购买后进入库存；如果用户点击使用，必须在扣减库存前返回明确提示：自定义道具暂不支持使用效果。
- 删除或下架后的自定义商品，若库存已持有，仍允许展示和装备已拥有的 `cosmetic/theme`。
- `use_item_tx` 当前会先扣减 `consumable` 库存，再只对 `food` 执行成长值逻辑；实现时必须先判断自定义 `tool` 并提前返回错误，不能让它被静默消耗。

### 5. Tauri 命令

新增命令：

- `import_pet_store_asset`
- `update_custom_pet_store_item`
- `delete_custom_pet_store_item`
- `list_custom_pet_store_items`

`import_pet_store_asset` 入参建议：

```text
source_file_path
item_type
name_zh?
name_en?
description_zh?
description_en?
price_lp?
growth_value?
enabled?
```

返回：

- 最新 `PetStoreState`，方便前端立即刷新。

后端校验：

- 文件存在且可读。
- 文件扩展名在白名单内。
- 文件大小不超过上限。
- 最终 PNG 必须通过透明度、主体居中、边距和画布尺寸校验。
- 生成的 `item_key` 和 `asset_key` 必须确认不在内置目录中。
- `item_type` 在一期支持范围内。
- `slot` 由 `item_type` 推导，不信任前端传入。
- 未传名称时，后端必须从文件名生成默认名称。
- 未传说明时，后端必须按类型生成默认说明。
- 未传价格时，后端必须按类型生成默认价格。
- 未传成长值时，`food` 使用默认成长值，非 `food` 使用 0。
- 用户传入时仍需校验 `price_lp >= 0`。
- 用户传入时仍需校验 `food` 的 `growth_value > 0`。
- 非 `food` 的 `growth_value = 0`。

## 前端任务

### 1. 类型扩展

在 `apps/desktop/src/shared/types/meeting.ts` 中扩展 `PetStoreCatalogItem`：

```text
assetSource?: "builtIn" | "custom"
assetUrl?: string
custom?: boolean
deleted?: boolean
```

内置商品：

- `assetSource = "builtIn"`
- 继续使用 `assetKey`

自定义商品：

- `assetSource = "custom"`
- 使用后端返回的 `assetUrl`
- `assetKey` 仍保留为稳定键

`assetUrl` 生成规则：

- 后端保存并返回 app data 下的自定义素材路径。
- 前端收到路径后用 Tauri `convertFileSrc(path)` 生成 `<img>` 可用 URL。
- `assetUrl` 存在时优先使用；不能把原始本机路径直接写入 `<img src>`。
- 详情页、库存、盲盒/礼盒历史、签到奖励、兑换奖励等所有商品图片入口都要经过同一个解析函数。

### 2. Tauri service

在 `apps/desktop/src/shared/services/tauri/pet.ts` 增加方法：

- `importStoreAsset(input)`
- `updateCustomStoreItem(input)`
- `deleteCustomStoreItem(itemKey)`
- `listCustomStoreItems()`

预览模式可先返回不支持提示，避免浏览器预览误以为真实导入可用。

### 3. 图片解析

调整 `apps/desktop/src/features/pet-store/services/petStorePresentation.ts`：

- 内置商品继续从 `shopImageUrlMap` 读取。
- 自定义商品优先使用 `item.assetUrl`。
- 图片缺失时回退到 `gift_box`。

建议新增函数：

```text
resolvePetStoreImageUrl(storeState, item)
```

避免调用处只传 `assetKey`，导致自定义商品拿不到 URL。

影响面需要同步检查：

- 宠物商店卡片
- 商品详情页
- 盲盒展示
- 礼盒结果
- 今日抽取历史
- 库存列表

### 4. 商店导入 UI

在 `apps/desktop/src/features/pet-store/views/PetStoreView.tsx` 增加导入入口。

第一期 UI：

- 顶部工具区增加“导入资产”按钮。
- 打开导入弹窗。
- 默认只展示：
  - 图片选择。
  - 图片预览。
  - 商品类型。
  - 商品名称。
- 商品名称从文件名自动生成，用户可以改。
- “更多设置”默认折叠，里面放：
  - 中文说明。
  - 英文名称。
  - 英文说明。
  - LP 价格。
  - 食物成长值。
  - 是否上架。
- 导入成功后刷新商店状态。

卡片展示：

- 自定义商品增加“自定义”标签。
- 自定义商品显示“下架”或“删除”管理入口。
- 内置商品不显示删除入口。

### 5. 商品详情页

若当前商品详情页只按 `assetKey` 解析图片，需要同步改为支持自定义 `assetUrl`。

详情页需要展示：

- 自定义标签。
- 类型、价格、持有数量。
- 下架状态。
- 删除后的库存商品仍能显示历史信息。

## 错误处理

用户可见错误：

- 图片格式不支持。
- 图片过大。
- 图片尺寸过大。
- 图片不是透明背景。
- 图片主体没有居中。
- 图片主体太靠边，需要留出透明边距。
- 图片主体太小或太大。
- 文件无法读取。
- 商品名称不能为空。
- LP 价格不能为负数。
- 食物成长值必须大于 0。
- 自定义道具暂不支持使用效果。
- 商品已删除或已下架，不能再次购买。
- 自定义商品已删除，但库存中的历史物品仍可查看或装备。

后端一致性：

- 复制图片失败时不写数据库。
- 写数据库失败时清理本次复制的图片。
- 图片质量校验失败时不写数据库、不复制入正式自定义素材目录。
- 删除自定义商品不删除库存。
- 内置商品永远不能通过自定义删除命令删除。
- 自定义 `tool` 不支持使用时不能扣库存。
- 软删除自定义商品后，库存和装备解析仍读取该商品的历史元数据。

## 验收清单

### 数据和状态

- [ ] 首次启动后内置商店目录不变。
- [ ] 导入、更新、删除自定义商品后，内置商品数量不变。
- [ ] 导入、更新、删除自定义商品后，内置 `item_key` 和 `asset_key` 集合不变。
- [ ] 导入、更新、删除自定义商品后，`apps/desktop/src/assets/images/shop/` 文件列表不变。
- [ ] 导入自定义 `food` 后，商店目录显示 `内置 + 自定义`。
- [ ] 重启应用后，自定义商品仍存在。
- [ ] 下架自定义商品后，商店不再允许购买。
- [ ] 删除自定义商品后，内置商品完全不受影响。
- [ ] 自定义商品 `item_key` 不会与内置商品冲突。
- [ ] 软删除自定义商品后，库存和装备中的历史商品仍显示名称、类型和图片。

### 购买和库存

- [ ] 自定义 `food` 可购买并叠加库存数量。
- [ ] 自定义 `food` 可使用，成长值增加正确。
- [ ] 自定义 `cosmetic` 可购买并装备到 `accessory`。
- [ ] 自定义 `theme` 可购买并装备到 `scene`。
- [ ] 自定义 `tool` 可购买，但使用时给出明确不支持提示且库存数量不变。
- [ ] 已删除的自定义商品若库存中仍持有，库存列表不空白、不报错。

### 图片

- [ ] 内置图片仍走打包资源。
- [ ] 自定义图片走 app data 文件 URL。
- [ ] 自定义图片 URL 通过 `convertFileSrc` 或等价 Tauri asset URL 生成，不直接使用原始本机路径。
- [ ] 图片缺失时显示占位图。
- [ ] PNG、JPG、WebP 导入结果都能显示。
- [ ] 非透明背景图片会被拒绝。
- [ ] 白底、黑底、棋盘格伪透明图片会被拒绝。
- [ ] 主体贴边或被裁切的图片会被拒绝。
- [ ] 主体明显偏离中心的图片会被拒绝。
- [ ] 主体太小或太大的图片会被拒绝。
- [ ] 合格图片会归一化为 512 x 512 透明 PNG。

### 随机奖池

- [ ] 盲盒奖池第一期不包含自定义商品。
- [ ] 礼盒奖池第一期不包含自定义商品。
- [ ] 内置礼盒逻辑不受自定义商品影响。

## 验证命令

前端：

```bash
pnpm --dir apps/desktop run typecheck
pnpm --dir apps/desktop run build:web
```

Rust：

```bash
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml pet_store
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
```

如需完整仓库验证：

```bash
pnpm check
```

## 推荐实施顺序

1. 新增 `pet_custom_store_items` 表和模型字段。
2. 调整 Rust 图片解码依赖，确保 PNG、JPG、WebP 在当前目标平台可导入并统一输出 PNG。
3. 增加自定义目录仓储函数。
4. 增加 catalog 查找矩阵对应的内置-only、any-catalog、inventory-catalog 函数。
5. 改造 `store_state` 合并内置和自定义商品。
6. 改造购买、使用、装备查找逻辑，支持自定义商品。
7. 在 `use_item_tx` 扣库存前拦截自定义 `tool`。
8. 增加图片复制、格式校验、透明背景校验、主体居中/边距校验和 app data 存储。
9. 增加 Tauri 导入、更新、删除、列表命令。
10. 扩展前端类型和 Tauri service。
11. 改造商店图片解析函数，支持 `assetUrl` 和 `convertFileSrc`。
12. 增加商店导入弹窗和自定义商品管理入口。
13. 同步商品详情页、库存、盲盒、礼盒、签到和兑换展示的图片解析。
14. 补 Rust 单测和前端类型检查。
15. 更新 `docs/pet-system.md` 的商店素材与自定义商品说明。

## 风险和注意事项

- 现有部分逻辑通过无连接的 `find_catalog_item(item_key)` 只查内置静态目录，必须逐个替换为能读取数据库的查找函数。
- 库存表目前不存商品名称和图片，自定义商品删除后仍需要通过自定义表软删除记录解析历史库存。
- 前端当前图片解析以 `assetKey` 为中心，自定义商品需要传入完整 catalog item，否则无法读取 `assetUrl`。
- 自定义 `asset_key` 如果复用内置图片键，会导致前端误显示或污染内置素材语义，导入时必须拒绝。
- 当前 `use_item_tx` 会先扣减 `consumable` 库存，自定义 `tool` 必须提前拦截，否则会出现“提示不支持但库存已消耗”的严重体验问题。
- 当前 Rust `image` 依赖配置不等于已经支持 JPG/WebP 导入，实现前必须补齐跨平台解码 feature。
- 本机路径不能直接作为图片 URL 使用，需要经过 Tauri asset URL 转换，并确认 CSP/capability 不阻断读取。
- 图片校验不能只看扩展名；必须读取像素 alpha 和主体包围盒，否则会把白底图、偏心图、贴边图带进商店。
- 自定义商品如果未来进入盲盒或礼盒奖池，需要额外处理重复补偿、权重、下架状态和用户期望。
- 自定义图片是本机文件，不能出现在导出包、源码素材目录或 git 跟踪中。
