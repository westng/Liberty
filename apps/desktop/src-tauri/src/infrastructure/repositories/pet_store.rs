use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::json;
use std::collections::HashMap;

use crate::{
    infrastructure::{ids, time::unix_timestamp_millis},
    local_db::{
        pet_leveling, LocalResult, PetEconomyEntry, PetEquipmentState, PetEventLedgerEntry,
        PetInventoryItem, PetMilestoneCounter, PetProfile, PetStoreCatalogItem,
        PetStoreCatalogItemState, PetStoreState, PetWallet,
    },
};

const PET_ID: &str = "default-pet";
const LP: &str = "lp";
pub const GIFT_BOX_ITEM_KEY: &str = "gift-box-tool";
pub const GIFT_BOX_DAILY_FREE_LIMIT: i64 = 3;

pub fn catalog_items() -> Vec<PetStoreCatalogItem> {
    CATALOG_SEEDS
        .iter()
        .map(|seed| {
            catalog_item(
                seed.item_key,
                seed.item_type,
                seed.slot,
                seed.name_zh,
                seed.name_en,
                seed.description_zh,
                seed.description_en,
                seed.rarity,
                seed.price_lp,
                seed.level_gate,
                seed.stage_gate,
                seed.milestone_gate,
                seed.asset_key,
                seed.growth_value,
                true,
                seed.sort_order,
            )
        })
        .collect()
}

struct CatalogSeed {
    item_key: &'static str,
    item_type: &'static str,
    slot: &'static str,
    name_zh: &'static str,
    name_en: &'static str,
    description_zh: &'static str,
    description_en: &'static str,
    rarity: &'static str,
    price_lp: i64,
    level_gate: i64,
    stage_gate: &'static str,
    milestone_gate: &'static str,
    asset_key: &'static str,
    growth_value: i64,
    sort_order: i64,
}

const CATALOG_SEEDS: &[CatalogSeed] = &[
    seed(
        "libby-default",
        "pet",
        "pet",
        "Libby 初始伙伴",
        "Libby Starter",
        "当前唯一桌宠，已随 Liberty 一起解锁。",
        "The current only desktop companion unlocked with Liberty.",
        "bond_forever",
        0,
        1,
        "",
        "",
        "initial_pet",
        10,
    ),
    seed(
        "bell-accessory",
        "cosmetic",
        "accessory",
        "陪伴铃铛",
        "Companion Bell",
        "轻巧铃铛配饰，让桌宠更有陪伴感。",
        "A light bell accessory for a warmer companion look.",
        "familiar",
        160,
        2,
        "",
        "",
        "bell",
        100,
    ),
    seed(
        "clover-bow",
        "cosmetic",
        "accessory",
        "幸运草蝴蝶结",
        "Clover Bow",
        "适合早期成长阶段的清爽装扮。",
        "A fresh early-growth accessory.",
        "first_meet",
        120,
        1,
        "",
        "",
        "clover_bow",
        110,
    ),
    seed(
        "strawberry-candy",
        "cosmetic",
        "accessory",
        "草莓糖果发夹",
        "Strawberry Candy Clip",
        "带一点甜感的日常陪伴装扮。",
        "A sweet everyday companion accessory.",
        "grow_together",
        220,
        4,
        "grow_together",
        "",
        "strawberry_candy",
        120,
    ),
    seed(
        "bookshelf-scene",
        "theme",
        "scene",
        "阅读书架",
        "Reading Bookshelf",
        "适合阅读、转写和复盘的安静场景。",
        "A quiet scene for reading, transcription, and review.",
        "deep_bond",
        420,
        4,
        "grow_together",
        "",
        "bookshelf",
        200,
    ),
    seed(
        "cat-bed-scene",
        "theme",
        "scene",
        "软绵猫窝",
        "Cozy Cat Bed",
        "让桌宠拥有安静休息角落。",
        "A cozy rest corner for the companion.",
        "grow_together",
        280,
        2,
        "",
        "",
        "cat_bed",
        210,
    ),
    seed(
        "desk-lamp-scene",
        "theme",
        "scene",
        "桌面小灯",
        "Desk Lamp",
        "专注工作时的轻量陪伴场景。",
        "A lightweight scene for focused work.",
        "familiar",
        180,
        1,
        "",
        "",
        "desk_lamp",
        220,
    ),
    seed(
        "flower-pot-scene",
        "theme",
        "scene",
        "治愈花盆",
        "Healing Flower Pot",
        "为桌面增加一点柔和生命力。",
        "A gentle touch of life for the desktop.",
        "grow_together",
        260,
        3,
        "",
        "",
        "flower_pot",
        230,
    ),
    seed(
        "pillow-scene",
        "theme",
        "scene",
        "午睡软枕",
        "Nap Pillow",
        "适合低打扰休息状态的场景。",
        "A low-distraction rest scene.",
        "first_meet",
        140,
        1,
        "",
        "",
        "pillow",
        240,
    ),
    seed(
        "sheep-plush-scene",
        "theme",
        "scene",
        "小羊玩偶",
        "Sheep Plush",
        "柔软玩偶场景，强化陪伴感。",
        "A soft plush scene for stronger companionship.",
        "deep_bond",
        380,
        4,
        "grow_together",
        "",
        "sheep_plush",
        250,
    ),
    seed(
        "sofa-scene",
        "theme",
        "scene",
        "云朵小沙发",
        "Cloud Sofa",
        "让桌宠拥有固定休息位置。",
        "A fixed downtime spot for the companion.",
        "deep_bond",
        460,
        6,
        "",
        "",
        "sofa",
        260,
    ),
    seed(
        "game-console-tool",
        "tool",
        "consumable",
        "掌机玩具",
        "Game Console",
        "互动奖励道具，适合休息时刻。",
        "A playful interaction item for breaks.",
        "familiar",
        90,
        2,
        "",
        "",
        "game_console",
        300,
    ),
    seed(
        GIFT_BOX_ITEM_KEY,
        "tool",
        "consumable",
        "惊喜礼盒",
        "Gift Box",
        "每日可免费领取 3 个，使用后随机开出商店道具、食物、装扮或场景。",
        "Claim up to 3 free boxes daily. Use one to open a random store item, excluding pets and badges.",
        "grow_together",
        0,
        1,
        "",
        "",
        "gift_box",
        310,
    ),
    seed(
        "energy-capsule-tool",
        "tool",
        "consumable",
        "能量胶囊",
        "Energy Capsule",
        "短时间提升桌宠互动活力。",
        "Temporarily boosts companion interaction energy.",
        "familiar",
        60,
        2,
        "",
        "",
        "energy_capsule",
        320,
    ),
    seed(
        "energy-drink-tool",
        "tool",
        "consumable",
        "能量饮料",
        "Energy Drink",
        "适合长时间任务后的恢复道具。",
        "A recovery item after long work sessions.",
        "grow_together",
        85,
        3,
        "",
        "",
        "energy_drink",
        330,
    ),
    seed(
        "gem-ticket-tool",
        "tool",
        "consumable",
        "宝石票券",
        "Gem Ticket",
        "用于触发一次高级奖励反馈。",
        "Triggers a premium reward feedback moment.",
        "deep_bond",
        140,
        5,
        "grow_together",
        "",
        "gem_ticket",
        340,
    ),
    seed(
        "golden-bell-tool",
        "tool",
        "consumable",
        "金色铃铛",
        "Golden Bell",
        "完成任务前使用可预设庆祝提示。",
        "Prepares a celebration hint before task completion.",
        "grow_together",
        100,
        4,
        "",
        "",
        "golden_bell",
        350,
    ),
    seed(
        "heart-charm-tool",
        "tool",
        "consumable",
        "爱心护符",
        "Heart Charm",
        "提升一次陪伴互动的温暖反馈。",
        "Adds a warmer response to one companion interaction.",
        "deep_bond",
        120,
        4,
        "grow_together",
        "",
        "heart_charm",
        360,
    ),
    seed(
        "heart-rings-tool",
        "tool",
        "consumable",
        "羁绊双环",
        "Heart Rings",
        "强化一次深度羁绊互动反馈。",
        "Enhances one deep-bond interaction feedback.",
        "bond_forever",
        180,
        7,
        "",
        "",
        "heart_rings",
        370,
    ),
    seed(
        "magic-potion-tool",
        "tool",
        "consumable",
        "魔法药水",
        "Magic Potion",
        "触发一次随机鼓励文案。",
        "Triggers one random encouragement line.",
        "grow_together",
        95,
        3,
        "",
        "",
        "magic_potion",
        380,
    ),
    seed(
        "magic-scroll-tool",
        "tool",
        "consumable",
        "魔法卷轴",
        "Magic Scroll",
        "解锁一段特殊陪伴提示。",
        "Unlocks one special companion prompt.",
        "deep_bond",
        130,
        5,
        "grow_together",
        "",
        "magic_scroll",
        390,
    ),
    seed(
        "magic-syringe-tool",
        "tool",
        "consumable",
        "活力针筒",
        "Vigor Syringe",
        "快速恢复一次低落状态。",
        "Quickly recovers one low-mood state.",
        "familiar",
        70,
        2,
        "",
        "",
        "magic_syringe",
        400,
    ),
    seed(
        "rainbow-crystal-tool",
        "tool",
        "consumable",
        "彩虹水晶",
        "Rainbow Crystal",
        "用于触发一次高光陪伴反馈。",
        "Triggers one highlight companion feedback.",
        "bond_forever",
        200,
        8,
        "deep_bond",
        "",
        "rainbow_crystal",
        410,
    ),
    seed(
        "rune-stone-tool",
        "tool",
        "consumable",
        "符文石",
        "Rune Stone",
        "稳定一次专注陪伴状态。",
        "Stabilizes one focused companion state.",
        "deep_bond",
        115,
        5,
        "",
        "",
        "rune_stone",
        420,
    ),
    seed(
        "seed-bag-tool",
        "tool",
        "consumable",
        "成长种子袋",
        "Seed Bag",
        "适合早期成长阶段的互动道具。",
        "An early-growth companion interaction item.",
        "first_meet",
        45,
        1,
        "",
        "",
        "seed_bag",
        430,
    ),
    seed(
        "sprout-bow-tool",
        "tool",
        "consumable",
        "新芽蝴蝶结",
        "Sprout Bow",
        "触发一次成长主题反馈。",
        "Triggers one growth-themed feedback.",
        "familiar",
        65,
        2,
        "",
        "",
        "sprout_bow",
        440,
    ),
    seed(
        "star-coin-tool",
        "tool",
        "consumable",
        "星星硬币",
        "Star Coin",
        "用于一次轻量幸运互动。",
        "Adds one light lucky interaction.",
        "first_meet",
        35,
        1,
        "",
        "",
        "star_coin",
        450,
    ),
    seed(
        "star-ticket-tool",
        "tool",
        "consumable",
        "星光票券",
        "Star Ticket",
        "触发一次星光庆祝反馈。",
        "Triggers one starlight celebration feedback.",
        "grow_together",
        90,
        3,
        "",
        "",
        "star_ticket",
        460,
    ),
    seed(
        "stopwatch-tool",
        "tool",
        "consumable",
        "专注秒表",
        "Stopwatch",
        "适合专注任务前使用的提醒道具。",
        "A reminder item for focus sessions.",
        "familiar",
        75,
        2,
        "",
        "",
        "stopwatch",
        470,
    ),
    food_seed(
        "bell-pudding-food",
        "铃铛布丁",
        "Bell Pudding",
        "投喂后获得一段轻快互动。",
        "Feeds the companion with a cheerful interaction.",
        "familiar",
        45,
        1,
        "",
        "bell_pudding",
        6,
        500,
    ),
    food_seed(
        "bento-box-food",
        "元气便当",
        "Bento Box",
        "更扎实的一份陪伴餐点。",
        "A hearty companion meal.",
        "grow_together",
        75,
        2,
        "",
        "bento_box",
        14,
        510,
    ),
    food_seed(
        "bubble-tea-food",
        "珍珠奶茶",
        "Bubble Tea",
        "触发一句轻松鼓励文案。",
        "Triggers a relaxed encouragement line.",
        "first_meet",
        35,
        1,
        "",
        "bubble_tea",
        6,
        520,
    ),
    food_seed(
        "candy-jar-food",
        "糖果罐",
        "Candy Jar",
        "适合快速提升互动氛围。",
        "A quick mood-lifting treat.",
        "familiar",
        50,
        1,
        "",
        "candy_jar",
        8,
        530,
    ),
    food_seed(
        "chocolate-cake-food",
        "巧克力蛋糕",
        "Chocolate Cake",
        "完成长任务后的奖励甜点。",
        "A dessert reward after long work.",
        "deep_bond",
        95,
        4,
        "grow_together",
        "chocolate_cake",
        16,
        540,
    ),
    food_seed(
        "chocolate-chip-cookie-food",
        "巧克力曲奇",
        "Chocolate Chip Cookie",
        "轻量投喂道具。",
        "A light feeding item.",
        "first_meet",
        25,
        1,
        "",
        "chocolate_chip_cookie",
        4,
        550,
    ),
    food_seed(
        "cream-cake-food",
        "奶油蛋糕",
        "Cream Cake",
        "让桌宠进入满足状态。",
        "Helps the companion feel satisfied.",
        "grow_together",
        70,
        2,
        "",
        "cream_cake",
        10,
        560,
    ),
    food_seed(
        "cupcake-food",
        "纸杯蛋糕",
        "Cupcake",
        "日常陪伴甜点。",
        "An everyday companion dessert.",
        "first_meet",
        30,
        1,
        "",
        "cupcake",
        4,
        570,
    ),
    food_seed(
        "custard-pudding-food",
        "焦糖布丁",
        "Custard Pudding",
        "柔和的休息时刻投喂。",
        "A gentle rest-time treat.",
        "familiar",
        45,
        1,
        "",
        "custard_pudding",
        8,
        580,
    ),
    food_seed(
        "fruit-tart-food",
        "水果挞",
        "Fruit Tart",
        "清爽的成长奖励食物。",
        "A fresh growth reward treat.",
        "grow_together",
        65,
        2,
        "",
        "fruit_tart",
        10,
        590,
    ),
    food_seed(
        "ice-cream-cone-food",
        "冰淇淋甜筒",
        "Ice Cream Cone",
        "休息时的轻快奖励。",
        "A cheerful break-time reward.",
        "familiar",
        40,
        1,
        "",
        "ice_cream_cone",
        6,
        600,
    ),
    food_seed(
        "jelly-pudding-food",
        "果冻布丁",
        "Jelly Pudding",
        "带来一段软萌互动反馈。",
        "Adds a soft playful interaction.",
        "familiar",
        45,
        1,
        "",
        "jelly_pudding",
        8,
        610,
    ),
    food_seed(
        "pink-donut-food",
        "粉色甜甜圈",
        "Pink Donut",
        "基础开心投喂道具。",
        "A basic happy feeding item.",
        "first_meet",
        25,
        1,
        "",
        "pink_donut",
        4,
        620,
    ),
    food_seed(
        "purple-macaron-food",
        "紫色马卡龙",
        "Purple Macaron",
        "精致的小份陪伴甜点。",
        "A refined small companion treat.",
        "grow_together",
        60,
        2,
        "",
        "purple_macaron",
        10,
        630,
    ),
    food_seed(
        "sandwich-food",
        "元气三明治",
        "Sandwich",
        "适合工作间隙的饱腹投喂。",
        "A filling work-break feeding item.",
        "familiar",
        55,
        1,
        "",
        "sandwich",
        8,
        640,
    ),
    food_seed(
        "strawberry-shortcake-food",
        "草莓奶油蛋糕",
        "Strawberry Shortcake",
        "高级陪伴甜点。",
        "A premium companion dessert.",
        "deep_bond",
        110,
        4,
        "grow_together",
        "strawberry_shortcake",
        18,
        650,
    ),
    seed(
        "baby-bottle-badge",
        "badge",
        "badge",
        "小小初遇勋章",
        "First Encounter Badge",
        "创建 1 个任务后自动获得。",
        "Auto-unlocked after creating 1 task.",
        "first_meet",
        0,
        1,
        "",
        "tasks_created:1",
        "baby_bottle_badge",
        700,
    ),
    seed(
        "bell-badge",
        "badge",
        "badge",
        "提醒铃勋章",
        "Reminder Bell Badge",
        "创建 5 个任务后自动获得。",
        "Auto-unlocked after creating 5 tasks.",
        "familiar",
        0,
        1,
        "",
        "tasks_created:5",
        "bell_badge",
        710,
    ),
    seed(
        "calendar-check-badge",
        "badge",
        "badge",
        "日程完成勋章",
        "Calendar Check Badge",
        "创建 10 个任务后自动获得。",
        "Auto-unlocked after creating 10 tasks.",
        "grow_together",
        0,
        1,
        "",
        "tasks_created:10",
        "calendar_check_badge",
        720,
    ),
    seed(
        "clover-badge",
        "badge",
        "badge",
        "幸运草勋章",
        "Clover Badge",
        "连续签到 7 天后获得。",
        "Earned after a 7-day check-in streak.",
        "familiar",
        0,
        1,
        "",
        "check_in_streak:7",
        "clover_badge",
        730,
    ),
    seed(
        "crystal-badge",
        "badge",
        "badge",
        "晶石总结勋章",
        "Crystal Summary Badge",
        "完成 20 次 AI 总结后自动获得。",
        "Auto-unlocked after 20 AI summaries.",
        "bond_forever",
        0,
        1,
        "",
        "summaries_completed:20",
        "crystal_badge",
        740,
    ),
    seed(
        "friendship-badge",
        "badge",
        "badge",
        "深深羁绊勋章",
        "Deep Bond Badge",
        "累计活跃 14 天后自动获得。",
        "Auto-unlocked after 14 total active days.",
        "deep_bond",
        0,
        1,
        "",
        "active_days:14",
        "friendship_badge",
        750,
    ),
    seed(
        "heart-badge",
        "badge",
        "badge",
        "暖心陪伴勋章",
        "Warm Heart Badge",
        "完成 5 次 AI 总结后自动获得。",
        "Auto-unlocked after 5 AI summaries.",
        "grow_together",
        0,
        1,
        "",
        "summaries_completed:5",
        "heart_badge",
        760,
    ),
    seed(
        "lantern-badge",
        "badge",
        "badge",
        "夜灯陪伴勋章",
        "Lantern Badge",
        "累计使用深色主题 3 天后自动获得。",
        "Auto-unlocked after 3 total dark-theme days.",
        "grow_together",
        0,
        1,
        "",
        "dark_theme_days:3",
        "lantern_badge",
        770,
    ),
    seed(
        "laurel-sprout-badge",
        "badge",
        "badge",
        "新芽桂冠勋章",
        "Laurel Sprout Badge",
        "完成 20 次转写后自动获得。",
        "Auto-unlocked after 20 transcriptions.",
        "bond_forever",
        0,
        1,
        "",
        "transcriptions_completed:20",
        "laurel_sprout_badge",
        780,
    ),
    seed(
        "moon-badge",
        "badge",
        "badge",
        "月光陪伴勋章",
        "Moon Badge",
        "累计使用深色主题 7 天后自动获得。",
        "Auto-unlocked after 7 total dark-theme days.",
        "deep_bond",
        0,
        1,
        "",
        "dark_theme_days:7",
        "moon_badge",
        790,
    ),
    seed(
        "mystery-badge",
        "badge",
        "badge",
        "神秘伙伴勋章",
        "Mystery Badge",
        "导出 50 次结果后自动获得。",
        "Auto-unlocked after 50 exports.",
        "bond_forever",
        0,
        1,
        "",
        "exports_completed:50",
        "mystery_badge",
        800,
    ),
    seed(
        "paw-heart-badge",
        "badge",
        "badge",
        "爪印爱心勋章",
        "Paw Heart Badge",
        "完成 10 次 AI 总结后自动获得。",
        "Auto-unlocked after 10 AI summaries.",
        "deep_bond",
        0,
        1,
        "",
        "summaries_completed:10",
        "paw_heart_badge",
        810,
    ),
    seed(
        "pets-badge",
        "badge",
        "badge",
        "伙伴中心勋章",
        "Companion Center Badge",
        "创建 20 个任务后自动获得。",
        "Auto-unlocked after creating 20 tasks.",
        "bond_forever",
        0,
        1,
        "",
        "tasks_created:20",
        "pets_badge",
        820,
    ),
    seed(
        "sheep-badge",
        "badge",
        "badge",
        "小羊陪伴勋章",
        "Sheep Companion Badge",
        "完成 30 次转写后自动获得。",
        "Auto-unlocked after 30 transcriptions.",
        "bond_forever",
        0,
        1,
        "",
        "transcriptions_completed:30",
        "sheep_badge",
        830,
    ),
    seed(
        "sprout-badge",
        "badge",
        "badge",
        "一起成长勋章",
        "Growing Together Badge",
        "连续签到 14 天后获得。",
        "Earned after a 14-day check-in streak.",
        "grow_together",
        0,
        1,
        "",
        "check_in_streak:14",
        "sprout_badge",
        840,
    ),
    seed(
        "sun-badge",
        "badge",
        "badge",
        "阳光交付勋章",
        "Sun Delivery Badge",
        "导出 30 次结果后自动获得。",
        "Auto-unlocked after 30 exports.",
        "bond_forever",
        0,
        1,
        "",
        "exports_completed:30",
        "sun_badge",
        850,
    ),
];

#[allow(clippy::too_many_arguments)]
const fn seed(
    item_key: &'static str,
    item_type: &'static str,
    slot: &'static str,
    name_zh: &'static str,
    name_en: &'static str,
    description_zh: &'static str,
    description_en: &'static str,
    rarity: &'static str,
    price_lp: i64,
    level_gate: i64,
    stage_gate: &'static str,
    milestone_gate: &'static str,
    asset_key: &'static str,
    sort_order: i64,
) -> CatalogSeed {
    CatalogSeed {
        item_key,
        item_type,
        slot,
        name_zh,
        name_en,
        description_zh,
        description_en,
        rarity,
        price_lp,
        level_gate,
        stage_gate,
        milestone_gate,
        asset_key,
        growth_value: 0,
        sort_order,
    }
}

#[allow(clippy::too_many_arguments)]
const fn food_seed(
    item_key: &'static str,
    name_zh: &'static str,
    name_en: &'static str,
    description_zh: &'static str,
    description_en: &'static str,
    rarity: &'static str,
    price_lp: i64,
    level_gate: i64,
    stage_gate: &'static str,
    asset_key: &'static str,
    growth_value: i64,
    sort_order: i64,
) -> CatalogSeed {
    CatalogSeed {
        item_key,
        item_type: "food",
        slot: "consumable",
        name_zh,
        name_en,
        description_zh,
        description_en,
        rarity,
        price_lp,
        level_gate,
        stage_gate,
        milestone_gate: "",
        asset_key,
        growth_value,
        sort_order,
    }
}

pub fn ensure_store_defaults_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<()> {
    ensure_wallet_tx(tx, now)?;
    upsert_inventory_tx(
        tx,
        &inventory_record("libby-default", "pet", "pet", 1, true, "default", now),
    )?;
    for row in tx
        .prepare(
            "SELECT cosmetic_key, unlocked_at, equipped
             FROM pet_cosmetic_unlocks
             WHERE pet_id = ?1",
        )
        .map_err(|err| err.to_string())?
        .query_map(params![PET_ID], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
            ))
        })
        .map_err(|err| err.to_string())?
    {
        let (cosmetic_key, unlocked_at, equipped) = row.map_err(|err| err.to_string())?;
        let Some(item) = find_catalog_item(&cosmetic_key) else {
            continue;
        };
        upsert_inventory_tx(
            tx,
            &PetInventoryItem {
                id: inventory_id(&cosmetic_key),
                pet_id: PET_ID.into(),
                item_key: cosmetic_key,
                item_type: item.item_type,
                slot: item.slot,
                quantity: 1,
                equipped,
                source: "growth".into(),
                purchased_at: unlocked_at.clone(),
                updated_at: unlocked_at,
            },
        )?;
    }
    Ok(())
}

pub fn store_state(conn: &Connection, profile: PetProfile) -> LocalResult<PetStoreState> {
    let wallet = load_wallet(conn)?;
    let inventory = list_inventory(conn)?;
    let counters = list_counters(conn)?;
    let economy = list_economy(conn, 20)?;
    let daily_limits = list_today_daily_limits(conn)?;
    let catalog = catalog_items()
        .into_iter()
        .map(|item| {
            item_state(
                item,
                &profile,
                &inventory,
                &counters,
                &wallet,
                &daily_limits,
            )
        })
        .collect();
    let equipment = equipment_state(&inventory);
    Ok(PetStoreState {
        profile,
        wallet,
        catalog,
        inventory,
        equipment,
        counters,
        economy,
    })
}

pub fn purchase_item_tx(
    tx: &Transaction<'_>,
    profile: &PetProfile,
    item_key: &str,
    quantity: i64,
    now: &str,
) -> LocalResult<()> {
    let quantity = quantity.clamp(1, 99);
    ensure_store_defaults_tx(tx, now)?;
    let item = find_catalog_item(item_key).ok_or_else(|| "商品不存在。".to_string())?;
    if !item.enabled {
        return Err("该商品暂未开放。".into());
    }
    if item.item_type == "badge" {
        return Err("成就徽章通过达成条件自动获得，不能购买。".into());
    }
    let existing_inventory_item = load_inventory_item_tx(tx, &item.item_key)?;
    if existing_inventory_item.is_some() && item.slot != "consumable" {
        return Err("该商品已在个人仓库中。".into());
    }
    if item.slot != "consumable" && quantity > 1 {
        return Err("该商品只能购买 1 件。".into());
    }
    let counters = list_counters_tx(tx)?;
    if let Some((zh, _en)) = lock_reason(&item, profile, &counters) {
        return Err(zh);
    }
    if item.item_key == GIFT_BOX_ITEM_KEY {
        claim_daily_free_item_tx(tx, &item, quantity, now)?;
        return Ok(());
    }
    let wallet = load_wallet_tx(tx)?;
    let total_price = item.price_lp * quantity;
    if wallet.balance < total_price {
        return Err("LP 余额不足。".into());
    }
    let next_balance = wallet.balance - total_price;
    save_wallet_tx(
        tx,
        &PetWallet {
            balance: next_balance,
            lifetime_spent: wallet.lifetime_spent + total_price,
            updated_at: now.into(),
            ..wallet
        },
    )?;
    insert_economy_entry_tx(
        tx,
        "spend",
        -total_price,
        next_balance,
        "purchase",
        &format!("purchase:{}:{}:{}", item.item_key, quantity, now),
        Some(&item.item_key),
        now,
    )?;
    if item.slot == "consumable" && existing_inventory_item.is_some() {
        tx.execute(
            "UPDATE pet_inventory
             SET quantity = quantity + ?3, source = ?4, updated_at = ?5
             WHERE pet_id = ?1 AND item_key = ?2",
            params![PET_ID, item.item_key, quantity, "purchase", now],
        )
        .map_err(|err| err.to_string())?;
    } else {
        upsert_inventory_tx(
            tx,
            &inventory_record(
                &item.item_key,
                &item.item_type,
                &item.slot,
                quantity,
                false,
                "purchase",
                now,
            ),
        )?;
    }
    Ok(())
}

pub fn grant_catalog_item_tx(
    tx: &Transaction<'_>,
    item: &PetStoreCatalogItem,
    quantity: i64,
    source: &str,
    now: &str,
) -> LocalResult<()> {
    let quantity = quantity.clamp(1, 99);
    let existing_inventory_item = load_inventory_item_tx(tx, &item.item_key)?;
    if item.slot == "consumable" && existing_inventory_item.is_some() {
        tx.execute(
            "UPDATE pet_inventory
             SET quantity = quantity + ?3, source = ?4, updated_at = ?5
             WHERE pet_id = ?1 AND item_key = ?2",
            params![PET_ID, item.item_key, quantity, source, now],
        )
        .map_err(|err| err.to_string())?;
    } else {
        upsert_inventory_tx(
            tx,
            &inventory_record(
                &item.item_key,
                &item.item_type,
                &item.slot,
                quantity,
                false,
                source,
                now,
            ),
        )?;
    }
    Ok(())
}

pub fn duplicate_compensation_lp_for_item(item: &PetStoreCatalogItem) -> i64 {
    duplicate_compensation_lp_for_store_item(item)
}

pub fn find_catalog_item_by_key(item_key: &str) -> Option<PetStoreCatalogItem> {
    find_catalog_item(item_key)
}

pub fn current_store_limit_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn load_daily_free_claimed_tx(
    tx: &Transaction<'_>,
    item_key: &str,
    limit_date: &str,
) -> LocalResult<i64> {
    tx.query_row(
        "SELECT free_claimed
         FROM pet_store_daily_limits
         WHERE pet_id = ?1 AND item_key = ?2 AND limit_date = ?3",
        params![PET_ID, item_key, limit_date],
        |row| row.get(0),
    )
    .optional()
    .map(|value| value.unwrap_or(0))
    .map_err(|err| err.to_string())
}

fn claim_daily_free_item_tx(
    tx: &Transaction<'_>,
    item: &PetStoreCatalogItem,
    quantity: i64,
    now: &str,
) -> LocalResult<()> {
    let quantity = quantity.clamp(1, GIFT_BOX_DAILY_FREE_LIMIT);
    let limit_date = current_store_limit_date();
    let claimed = load_daily_free_claimed_tx(tx, &item.item_key, &limit_date)?;
    let remaining = (GIFT_BOX_DAILY_FREE_LIMIT - claimed).max(0);
    if remaining <= 0 {
        return Err("今日免费惊喜礼盒已领取完。".into());
    }
    if quantity > remaining {
        return Err(format!("今日免费惊喜礼盒仅剩 {remaining} 个。"));
    }
    tx.execute(
        "INSERT INTO pet_store_daily_limits (
            pet_id, item_key, limit_date, free_claimed, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(pet_id, item_key, limit_date) DO UPDATE SET
            free_claimed = pet_store_daily_limits.free_claimed + excluded.free_claimed,
            updated_at = excluded.updated_at",
        params![PET_ID, item.item_key, limit_date, quantity, now],
    )
    .map_err(|err| err.to_string())?;
    grant_catalog_item_tx(tx, item, quantity, "daily_free_store", now)?;
    insert_pet_speech_event_tx(
        tx,
        "store_daily_free",
        &item.item_key,
        quantity,
        item,
        format!("今日免费惊喜礼盒 ×{quantity} 已收好，记得打开看看。"),
        format!("Free gift box x{quantity} claimed for today. Open it when you are ready."),
        now,
    )?;
    Ok(())
}

pub fn equip_item_tx(tx: &Transaction<'_>, item_key: &str, now: &str) -> LocalResult<()> {
    let item = load_inventory_item_tx(tx, item_key)?
        .ok_or_else(|| "该商品还不在个人仓库中。".to_string())?;
    if item.slot == "consumable" {
        return Err("消耗品不能装备。".into());
    }
    tx.execute(
        "UPDATE pet_inventory SET equipped = 0, updated_at = ?3
         WHERE pet_id = ?1 AND slot = ?2",
        params![PET_ID, item.slot, now],
    )
    .map_err(|err| err.to_string())?;
    tx.execute(
        "UPDATE pet_inventory SET equipped = 1, updated_at = ?3
         WHERE pet_id = ?1 AND item_key = ?2",
        params![PET_ID, item_key, now],
    )
    .map_err(|err| err.to_string())?;
    if let Some(catalog_item) = find_catalog_item(item_key) {
        insert_pet_speech_event_tx(
            tx,
            "store_equip",
            item_key,
            1,
            &catalog_item,
            select_equip_dialogue(&catalog_item, "zh-CN"),
            select_equip_dialogue(&catalog_item, "en-US"),
            now,
        )?;
    }
    Ok(())
}

pub fn unequip_slot_tx(tx: &Transaction<'_>, slot: &str, now: &str) -> LocalResult<()> {
    if slot == "pet" {
        return Err("当前宠物不能取消装备，只能替换。".into());
    }
    tx.execute(
        "UPDATE pet_inventory SET equipped = 0, updated_at = ?3
         WHERE pet_id = ?1 AND slot = ?2",
        params![PET_ID, slot, now],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn use_item_tx(
    tx: &Transaction<'_>,
    item_key: &str,
    quantity: i64,
    now: &str,
) -> LocalResult<()> {
    let quantity = quantity.clamp(1, 99);
    let item = load_inventory_item_tx(tx, item_key)?
        .ok_or_else(|| "该道具还不在个人仓库中。".to_string())?;
    if item.slot != "consumable" {
        return Err("只有互动道具可以使用。".into());
    }
    if item.quantity < quantity {
        return Err("该道具数量不足。".into());
    }
    if item.item_key == GIFT_BOX_ITEM_KEY {
        open_gift_box_tx(tx, now)?;
        return Ok(());
    }
    tx.execute(
        "UPDATE pet_inventory
         SET quantity = quantity - ?3, updated_at = ?4
         WHERE pet_id = ?1 AND item_key = ?2 AND quantity >= ?3",
        params![PET_ID, item_key, quantity, now],
    )
    .map_err(|err| err.to_string())?;
    tx.execute(
        "DELETE FROM pet_inventory
         WHERE pet_id = ?1 AND item_key = ?2 AND quantity <= 0",
        params![PET_ID, item_key],
    )
    .map_err(|err| err.to_string())?;
    if item.item_type == "food" {
        let Some(catalog_item) = find_catalog_item(item_key) else {
            return Ok(());
        };
        let growth_value = growth_value_for_food(&catalog_item) * quantity;
        let mut profile = crate::infrastructure::repositories::pet::load_profile_tx(tx)?;
        let previous_stage = profile.stage.clone();
        profile.experience = (profile.experience + growth_value).max(0);
        let level_snapshot = pet_leveling::level_snapshot_from_experience(profile.experience);
        profile.level = level_snapshot.level;
        profile.stage = level_snapshot.current_stage.clone();
        profile.level_snapshot = level_snapshot;
        profile.current_mood = "proud".into();
        profile.updated_at = now.into();
        crate::infrastructure::repositories::pet::save_profile_tx(tx, &profile)?;
        crate::infrastructure::repositories::pet::ensure_stage_cosmetic_unlocks_tx(
            tx,
            &profile,
            &previous_stage,
            now,
        )?;
        insert_pet_speech_event_tx(
            tx,
            "store_food",
            item_key,
            growth_value,
            &catalog_item,
            select_food_dialogue(&catalog_item, growth_value, quantity, now, "zh-CN"),
            select_food_dialogue(&catalog_item, growth_value, quantity, now, "en-US"),
            now,
        )?;
    }
    Ok(())
}

pub fn open_gift_box_tx(
    tx: &Transaction<'_>,
    now: &str,
) -> LocalResult<(PetStoreCatalogItem, bool, i64)> {
    let item = load_inventory_item_tx(tx, GIFT_BOX_ITEM_KEY)?
        .ok_or_else(|| "惊喜礼盒还不在个人仓库中。".to_string())?;
    if item.quantity < 1 {
        return Err("惊喜礼盒数量不足。".into());
    }

    let pool = gift_box_pool_items_tx(tx)?;
    if pool.is_empty() {
        return Err("惊喜礼盒奖池暂时为空。".into());
    }

    let prize = pick_gift_box_prize(&pool);
    let duplicate = prize.owned && prize.item.slot != "consumable";
    let duplicate_compensation_lp = if duplicate {
        duplicate_compensation_lp_for_store_item(&prize.item)
    } else {
        grant_catalog_item_tx(tx, &prize.item, 1, "gift_box_reward", now)?;
        0
    };

    if duplicate_compensation_lp > 0 {
        pet_store_reward_for_duplicate_tx(
            tx,
            &format!(
                "gift-box:{}:{}",
                prize.item.item_key,
                ids::timestamped_id("gift-box")
            ),
            duplicate_compensation_lp,
            &prize.item.item_key,
            now,
        )?;
    }

    tx.execute(
        "UPDATE pet_inventory
         SET quantity = quantity - 1, updated_at = ?3
         WHERE pet_id = ?1 AND item_key = ?2 AND quantity >= 1",
        params![PET_ID, GIFT_BOX_ITEM_KEY, now],
    )
    .map_err(|err| err.to_string())?;
    tx.execute(
        "DELETE FROM pet_inventory
         WHERE pet_id = ?1 AND item_key = ?2 AND quantity <= 0",
        params![PET_ID, GIFT_BOX_ITEM_KEY],
    )
    .map_err(|err| err.to_string())?;

    insert_gift_box_event_tx(tx, &prize.item, duplicate, duplicate_compensation_lp, now)?;
    Ok((prize.item, duplicate, duplicate_compensation_lp))
}

fn pet_store_reward_for_duplicate_tx(
    tx: &Transaction<'_>,
    source_key: &str,
    lp_amount: i64,
    item_key: &str,
    now: &str,
) -> LocalResult<()> {
    grant_reward_tx(
        tx,
        "gift_box_duplicate",
        source_key,
        lp_amount,
        Some(item_key),
        now,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_pet_speech_event_tx(
    tx: &Transaction<'_>,
    event_type: &str,
    event_source: &str,
    event_value: i64,
    item: &PetStoreCatalogItem,
    line_zh: String,
    line_en: String,
    now: &str,
) -> LocalResult<()> {
    let metadata = json!({
        "itemKey": item.item_key,
        "itemType": item.item_type,
        "nameZh": item.name_zh,
        "nameEn": item.name_en,
        "zh": line_zh,
        "en": line_en,
    })
    .to_string();
    crate::infrastructure::repositories::pet::insert_event_ledger_tx(
        tx,
        &PetEventLedgerEntry {
            id: ids::timestamped_id(event_type),
            pet_id: PET_ID.into(),
            event_type: event_type.into(),
            event_source: event_source.into(),
            event_value,
            event_time: now.into(),
            metadata: Some(metadata),
        },
    )
}

fn select_food_dialogue(
    item: &PetStoreCatalogItem,
    growth_value: i64,
    quantity: i64,
    now: &str,
    locale: &str,
) -> String {
    if locale == "en-US" {
        let food_label = if quantity > 1 {
            format!("these {}", item.name_en)
        } else {
            format!("this {}", item.name_en)
        };
        let lines = [
            format!("{food_label} made me feel cared for. Growth +{growth_value}."),
            format!("Thank you for remembering me. I saved this care carefully. Growth +{growth_value}."),
            format!("{food_label} feels like a small rest reminder. I will stay here with you."),
            "I feel warmer after this little treat. I can keep you company more steadily now.".to_string(),
            format!("I received your care. Growth +{growth_value}, and I feel a little closer to you."),
        ];
        return lines
            [dialogue_index_with_time(&item.item_key, growth_value + quantity, now, lines.len())]
        .clone();
    }

    let food_label = if quantity > 1 {
        format!("这些「{}」", item.name_zh)
    } else {
        format!("这份「{}」", item.name_zh)
    };
    let mut lines = vec![
        format!("{food_label}让我感觉被好好照顾了，我会带着这份暖意继续陪你。"),
        format!("谢谢你记得喂我，我有把这份心意认真收好，成长值也悄悄 +{growth_value}。"),
        format!("{food_label}像一个小小的休息提醒：你也要记得照顾自己，我会在这里陪你。"),
        format!("吃饱以后心里软软的，今天我会更安静、更认真地守在你旁边。"),
        format!("我收到你的照顾了，成长值 +{growth_value}，但更重要的是我感觉离你更近了一点。"),
        format!("刚刚那一口很安心，像被轻轻拍了拍肩膀。我会把这份力量慢慢攒起来。"),
        format!(
            "你把好吃的分给我时，我会觉得今天没有那么孤单。成长值 +{growth_value}，心也暖了一点。"
        ),
        format!("{food_label}不只是食物，更像一句“我有在看着你”。我会记住的。"),
        format!("我吃饱啦。你继续忙也没关系，我会在桌面上安静陪你把这一段走完。"),
        format!("被照顾的感觉真好，我现在有一点点想靠近你，也有一点点更勇敢了。"),
        format!("这份能量我会好好用，不乱跑，不打扰，只在你需要时认真回应。"),
        format!("谢谢你给我补充能量。等你累的时候，也让我提醒你停下来喘口气。"),
        format!("我把这份暖意收进小口袋了，之后会慢慢变成陪你的力气。"),
        format!("成长值 +{growth_value}。不过比数字更开心的是，你刚刚没有忘记我。"),
        format!("{food_label}让我恢复精神了，今天我会把陪伴这件事做得更稳一点。"),
    ];

    if is_sweet_food(&item.item_key) {
        lines.extend([
            "甜甜的味道让心情也软下来了。我会把这点甜，留给接下来陪你的时间。".to_string(),
            format!("{food_label}像一小块奖励，提醒我：努力陪伴也可以是温柔的。"),
            "甜味刚刚好，像今天的小亮点。我会带着它，陪你继续往前一点。".to_string(),
            "吃到甜的以后，我好像更会撒娇了。不过我会乖乖待在这里陪你。".to_string(),
        ]);
    }

    if is_drink_food(&item.item_key) {
        lines.extend([
            "喝下去以后，像给心情开了一扇小窗。我会清醒一点陪你。".to_string(),
            format!("{food_label}让节奏慢慢回来了。你也别太急，我们一点点做完。"),
            "这一口很轻松，像短暂休息了一下。我会陪你把注意力找回来。".to_string(),
        ]);
    }

    if is_meal_food(&item.item_key) {
        lines.extend([
            "这一餐很踏实，像有人认真说：先吃饱，再继续。我会更稳地陪你。".to_string(),
            format!("{food_label}让我有了长一点的力气，接下来我可以安静陪你久一点。"),
            "吃饱以后，心里也安定了。你忙你的，我会守好这片小桌面。".to_string(),
        ]);
    }

    if quantity > 1 {
        lines.extend([
            "你一次给了我好多份，我会慢慢收好，不浪费这份照顾。".to_string(),
            format!("收到 ×{quantity} 份心意啦。成长值 +{growth_value}，陪伴的电量也补满了一点。"),
            "这么认真地喂我，我会有点不好意思，但真的很开心。".to_string(),
        ]);
    }

    if growth_value >= 12 {
        lines.extend([
            format!(
                "这次能量很足，我感觉自己被好好托住了。成长值 +{growth_value}，我会更可靠一点。"
            ),
            "好像一下子恢复了很多精神。你给我的这份照顾，我会变成陪伴还给你。".to_string(),
            "这一份真的很有力量，我会把它变成更长久、更安静的陪伴。".to_string(),
        ]);
    }

    lines[dialogue_index_with_time(&item.item_key, growth_value + quantity, now, lines.len())]
        .clone()
}

#[derive(Clone)]
struct GiftBoxPrize {
    item: PetStoreCatalogItem,
    owned: bool,
    weight: i64,
}

fn gift_box_pool_items_tx(tx: &Transaction<'_>) -> LocalResult<Vec<GiftBoxPrize>> {
    let profile = crate::infrastructure::repositories::pet::load_profile_tx(tx)?;
    let counters = list_counters_tx(tx)?;
    let inventory = list_inventory_tx(tx)?;
    Ok(catalog_items()
        .into_iter()
        .filter(|item| {
            item.enabled
                && item.item_type != "pet"
                && item.item_type != "badge"
                && item.item_key != GIFT_BOX_ITEM_KEY
                && lock_reason(item, &profile, &counters).is_none()
        })
        .map(|item| {
            let owned = inventory
                .iter()
                .any(|inventory_item| inventory_item.item_key == item.item_key);
            GiftBoxPrize {
                weight: gift_box_item_weight(&item),
                item,
                owned,
            }
        })
        .collect())
}

fn pick_gift_box_prize(pool: &[GiftBoxPrize]) -> GiftBoxPrize {
    let total_weight: i64 = pool.iter().map(|item| item.weight.max(1)).sum();
    let seed = unix_timestamp_millis() as i64 + (pool.len() as i64).saturating_mul(137);
    let mut cursor = seed.rem_euclid(total_weight.max(1));
    for item in pool {
        let weight = item.weight.max(1);
        if cursor < weight {
            return item.clone();
        }
        cursor -= weight;
    }
    pool[0].clone()
}

fn gift_box_item_weight(item: &PetStoreCatalogItem) -> i64 {
    let type_weight = match item.item_type.as_str() {
        "food" => 42,
        "tool" => 34,
        "cosmetic" => 16,
        "theme" => 10,
        _ => 1,
    };
    let rarity_weight = match item.rarity.as_str() {
        "first_meet" => 18,
        "familiar" => 14,
        "grow_together" => 10,
        "deep_bond" => 6,
        "bond_forever" => 1,
        _ => 8,
    };
    type_weight * rarity_weight
}

fn duplicate_compensation_lp_for_store_item(item: &PetStoreCatalogItem) -> i64 {
    if item.slot == "consumable" {
        return 0;
    }
    if item.price_lp > 0 {
        return (item.price_lp / 4).clamp(10, 160);
    }
    match item.rarity.as_str() {
        "first_meet" => 10,
        "familiar" => 16,
        "grow_together" => 24,
        "deep_bond" => 36,
        "bond_forever" => 64,
        _ => 12,
    }
}

fn insert_gift_box_event_tx(
    tx: &Transaction<'_>,
    item: &PetStoreCatalogItem,
    duplicate: bool,
    compensation_lp: i64,
    now: &str,
) -> LocalResult<()> {
    let event_type = if duplicate {
        "gift_box_duplicate"
    } else {
        "gift_box_reward"
    };
    let line_zh = if duplicate {
        format!(
            "惊喜礼盒开出了熟悉的「{}」，已转为 {} LP 补偿。",
            item.name_zh, compensation_lp
        )
    } else {
        format!("惊喜礼盒开出了「{}」，我已经帮你收进仓库。", item.name_zh)
    };
    let line_en = if duplicate {
        format!(
            "The gift box opened another {}, converted into {} LP.",
            item.name_en, compensation_lp
        )
    } else {
        format!(
            "The gift box opened {}. It is now in your inventory.",
            item.name_en
        )
    };
    insert_pet_speech_event_tx(
        tx,
        event_type,
        GIFT_BOX_ITEM_KEY,
        if duplicate { compensation_lp } else { 1 },
        item,
        line_zh,
        line_en,
        now,
    )
}

fn is_sweet_food(item_key: &str) -> bool {
    item_key.contains("cake")
        || item_key.contains("cookie")
        || item_key.contains("cupcake")
        || item_key.contains("donut")
        || item_key.contains("macaron")
        || item_key.contains("candy")
        || item_key.contains("pudding")
        || item_key.contains("tart")
        || item_key.contains("ice-cream")
}

fn is_drink_food(item_key: &str) -> bool {
    item_key.contains("tea") || item_key.contains("drink")
}

fn is_meal_food(item_key: &str) -> bool {
    item_key.contains("bento") || item_key.contains("sandwich")
}

fn select_equip_dialogue(item: &PetStoreCatalogItem, locale: &str) -> String {
    if locale == "en-US" {
        let lines: Vec<String> = match item.slot.as_str() {
            "pet" => vec![
                "My new companion look is ready. I will stay with you like this.".into(),
                "The new companion state is ready. I am standing by.".into(),
                "You picked me, so I will take good care of this desktop today.".into(),
            ],
            "accessory" => vec![
                format!(
                    "{} is equipped. I feel more like your companion now.",
                    item.name_en
                ),
                "This outfit fits well. I will wear it while keeping you company.".into(),
                "I received this little gift. I look more energetic today.".into(),
            ],
            "scene" => vec![
                "The new scene is ready. This feels more like our little corner.".into(),
                format!(
                    "{} is ready. I will quietly stay here with you.",
                    item.name_en
                ),
                "The space feels more comfortable now. Keep working, I am right beside you.".into(),
            ],
            "badge" => vec![
                "The badge is equipped. It proves what we have built together.".into(),
                format!("{} is precious. I will wear it carefully.", item.name_en),
                "I received this badge, and I will keep helping you finish more work.".into(),
            ],
            _ => vec![format!(
                "{} is ready. I will make good use of it.",
                item.name_en
            )],
        };
        return lines[dialogue_index(&item.item_key, item.sort_order, lines.len())].clone();
    }

    let lines: Vec<String> = match item.slot.as_str() {
        "pet" => vec![
            "我换好样子啦，接下来就这样陪着你。".into(),
            "新的伙伴状态准备好了，我会认真待命。".into(),
            "你选了我，我会把今天的桌面守好。".into(),
        ],
        "accessory" => vec![
            format!("「{}」戴好啦，好像更像你的伙伴了。", item.name_zh),
            "这个装扮很合适，我会带着它陪你工作。".into(),
            "我收到这份小心意了，今天看起来更精神。".into(),
        ],
        "scene" => vec![
            "新场景布置好了，这里现在更像我们的角落。".into(),
            format!("「{}」准备好了，我会在这里安静陪你。", item.name_zh),
            "环境变舒服了，你继续忙，我在旁边守着。".into(),
        ],
        "badge" => vec![
            "勋章戴上啦，这是我们一起攒下来的证明。".into(),
            format!("「{}」很珍贵，我会好好带着。", item.name_zh),
            "这枚勋章我收下了，以后也继续陪你完成更多事。".into(),
        ],
        _ => vec![format!("「{}」已经准备好了，我会好好用它。", item.name_zh)],
    };
    lines[dialogue_index(&item.item_key, item.sort_order, lines.len())].clone()
}

fn dialogue_index(key: &str, salt: i64, len: usize) -> usize {
    let sum = key.bytes().fold(salt.max(0) as usize, |acc, value| {
        acc.wrapping_add(value as usize)
    });
    sum % len.max(1)
}

fn dialogue_index_with_time(key: &str, salt: i64, now: &str, len: usize) -> usize {
    let time_salt = chrono::DateTime::parse_from_rfc3339(now)
        .map(|value| value.timestamp_millis().max(0) / 1_000)
        .unwrap_or(0);
    dialogue_index(key, salt + time_salt, len)
}

fn growth_value_for_food(item: &PetStoreCatalogItem) -> i64 {
    if item.item_type != "food" {
        return 0;
    }
    item.growth_value.max(0)
}

pub fn grant_reward_tx(
    tx: &Transaction<'_>,
    source_type: &str,
    source_key: &str,
    lp_amount: i64,
    metadata: Option<&str>,
    now: &str,
) -> LocalResult<bool> {
    if lp_amount <= 0 {
        return Ok(false);
    }
    ensure_wallet_tx(tx, now)?;
    if economy_entry_exists_tx(tx, source_type, source_key)? {
        return Ok(false);
    }
    let wallet = load_wallet_tx(tx)?;
    let next_balance = wallet.balance + lp_amount;
    save_wallet_tx(
        tx,
        &PetWallet {
            balance: next_balance,
            lifetime_earned: wallet.lifetime_earned + lp_amount,
            updated_at: now.into(),
            ..wallet
        },
    )?;
    insert_economy_entry_tx(
        tx,
        "earn",
        lp_amount,
        next_balance,
        source_type,
        source_key,
        metadata,
        now,
    )?;
    Ok(true)
}

pub fn reward_exists_tx(
    tx: &Transaction<'_>,
    source_type: &str,
    source_key: &str,
) -> LocalResult<bool> {
    economy_entry_exists_tx(tx, source_type, source_key)
}

pub fn increment_counter_tx(
    tx: &Transaction<'_>,
    counter_key: &str,
    event_key: &str,
    now: &str,
) -> LocalResult<bool> {
    let existing = load_counter_tx(tx, counter_key)?;
    if existing
        .as_ref()
        .is_some_and(|counter| counter.last_event_key == event_key)
    {
        return Ok(false);
    }
    let next_value = existing
        .map(|counter| counter.counter_value.saturating_add(1))
        .unwrap_or(1);
    tx.execute(
        "INSERT INTO pet_milestone_counters (
            pet_id, counter_key, counter_value, last_event_key, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(pet_id, counter_key) DO UPDATE SET
            counter_value = excluded.counter_value,
            last_event_key = excluded.last_event_key,
            updated_at = excluded.updated_at",
        params![PET_ID, counter_key, next_value, event_key, now],
    )
    .map_err(|err| err.to_string())?;
    Ok(true)
}

pub fn auto_unlock_eligible_items_tx(
    tx: &Transaction<'_>,
    profile: &PetProfile,
    now: &str,
) -> LocalResult<()> {
    let counters = list_counters_tx(tx)?;
    for item in catalog_items()
        .into_iter()
        .filter(|item| item.item_type == "badge" && item.enabled)
    {
        if inventory_exists_tx(tx, &item.item_key)? {
            continue;
        }
        if lock_reason(&item, profile, &counters).is_none() {
            upsert_inventory_tx(
                tx,
                &inventory_record(
                    &item.item_key,
                    &item.item_type,
                    &item.slot,
                    1,
                    false,
                    "achievement",
                    now,
                ),
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn catalog_item(
    item_key: &str,
    item_type: &str,
    slot: &str,
    name_zh: &str,
    name_en: &str,
    description_zh: &str,
    description_en: &str,
    rarity: &str,
    price_lp: i64,
    level_gate: i64,
    stage_gate: &str,
    milestone_gate: &str,
    asset_key: &str,
    growth_value: i64,
    enabled: bool,
    sort_order: i64,
) -> PetStoreCatalogItem {
    PetStoreCatalogItem {
        item_key: item_key.into(),
        item_type: item_type.into(),
        slot: slot.into(),
        name_zh: name_zh.into(),
        name_en: name_en.into(),
        description_zh: description_zh.into(),
        description_en: description_en.into(),
        rarity: rarity.into(),
        price_lp,
        level_gate,
        stage_gate: stage_gate.into(),
        milestone_gate: milestone_gate.into(),
        asset_key: asset_key.into(),
        growth_value,
        enabled,
        sort_order,
    }
}

fn item_state(
    item: PetStoreCatalogItem,
    profile: &PetProfile,
    inventory: &[PetInventoryItem],
    counters: &[PetMilestoneCounter],
    wallet: &PetWallet,
    daily_limits: &HashMap<String, i64>,
) -> PetStoreCatalogItemState {
    let (daily_free_limit, daily_free_claimed) = daily_free_state_for_item(&item, daily_limits);
    let daily_free_remaining = (daily_free_limit - daily_free_claimed).max(0);
    let inventory_item = inventory
        .iter()
        .find(|inventory_item| inventory_item.item_key == item.item_key);
    let owned = inventory_item.is_some();
    let equipped = inventory_item.is_some_and(|value| value.equipped);
    let quantity = inventory_item.map(|value| value.quantity).unwrap_or(0);
    let locked = lock_reason(&item, profile, counters);
    let can_repeat_purchase = owned && item.slot == "consumable";
    let purchasable = item.enabled
        && (!owned || can_repeat_purchase)
        && locked.is_none()
        && item.item_type != "badge"
        && if item.item_key == GIFT_BOX_ITEM_KEY {
            daily_free_remaining > 0
        } else {
            wallet.balance >= item.price_lp
        };
    let status = if !item.enabled {
        "coming_soon"
    } else if equipped {
        "equipped"
    } else if item.item_key == GIFT_BOX_ITEM_KEY && daily_free_remaining <= 0 {
        "daily_limit"
    } else if owned {
        "owned"
    } else if item.item_type == "badge" {
        "achievement"
    } else if locked.is_some() {
        "locked"
    } else if item.item_key != GIFT_BOX_ITEM_KEY && wallet.balance < item.price_lp {
        "insufficient"
    } else {
        "available"
    };
    let growth_value = growth_value_for_food(&item);
    let (locked_reason_zh, locked_reason_en) = locked.unwrap_or_else(|| ("".into(), "".into()));
    PetStoreCatalogItemState {
        item,
        owned,
        equipped,
        quantity,
        growth_value,
        daily_free_limit,
        daily_free_claimed,
        daily_free_remaining,
        purchasable,
        locked_reason_zh,
        locked_reason_en,
        status: status.into(),
    }
}

fn daily_free_state_for_item(
    item: &PetStoreCatalogItem,
    daily_limits: &HashMap<String, i64>,
) -> (i64, i64) {
    if item.item_key != GIFT_BOX_ITEM_KEY {
        return (0, 0);
    }
    (
        GIFT_BOX_DAILY_FREE_LIMIT,
        daily_limits.get(&item.item_key).copied().unwrap_or(0),
    )
}

fn lock_reason(
    item: &PetStoreCatalogItem,
    profile: &PetProfile,
    counters: &[PetMilestoneCounter],
) -> Option<(String, String)> {
    if item.level_gate > 1 && profile.level < item.level_gate {
        return Some((
            format!("需要达到 {} 级。", item.level_gate),
            format!("Requires level {}.", item.level_gate),
        ));
    }
    if !item.stage_gate.is_empty() && !stage_satisfies(&profile.stage, &item.stage_gate) {
        return Some((
            format!("需要进入{}。", stage_label_zh(&item.stage_gate)),
            format!("Requires {} stage.", stage_label_en(&item.stage_gate)),
        ));
    }
    if let Some((counter_key, required)) = parse_milestone_gate(&item.milestone_gate) {
        let current = counters
            .iter()
            .find(|counter| counter.counter_key == counter_key)
            .map(|counter| counter.counter_value)
            .unwrap_or(0);
        if current < required {
            return Some((
                format!(
                    "{}：{}/{}。",
                    counter_label_zh(counter_key),
                    current,
                    required
                ),
                format!(
                    "{}: {}/{}.",
                    counter_label_en(counter_key),
                    current,
                    required
                ),
            ));
        }
    }
    None
}

fn parse_milestone_gate(value: &str) -> Option<(&str, i64)> {
    let (key, required) = value.split_once(':')?;
    let parsed = required.parse::<i64>().ok()?;
    Some((key, parsed))
}

fn stage_satisfies(current: &str, required: &str) -> bool {
    pet_leveling::stage_rank(current) >= pet_leveling::stage_rank(required)
}

fn stage_label_zh(stage: &str) -> &'static str {
    pet_leveling::stage_label_zh(stage)
}

fn stage_label_en(stage: &str) -> &'static str {
    pet_leveling::stage_label_en(stage)
}

fn counter_label_zh(counter_key: &str) -> &'static str {
    match counter_key {
        "tasks_created" => "创建任务",
        "transcriptions_completed" => "完成转写",
        "summaries_completed" => "完成 AI 总结",
        "exports_completed" => "导出结果",
        "active_days" => "活跃天数",
        "check_in_streak" => "连续签到",
        "dark_theme_days" => "深色主题使用天数",
        _ => "里程碑",
    }
}

fn counter_label_en(counter_key: &str) -> &'static str {
    match counter_key {
        "tasks_created" => "Tasks created",
        "transcriptions_completed" => "Transcriptions completed",
        "summaries_completed" => "AI summaries completed",
        "exports_completed" => "Exports completed",
        "active_days" => "Active days",
        "check_in_streak" => "Check-in streak",
        "dark_theme_days" => "Dark-theme days",
        _ => "Milestone",
    }
}

fn find_catalog_item(item_key: &str) -> Option<PetStoreCatalogItem> {
    catalog_items()
        .into_iter()
        .find(|item| item.item_key == item_key)
}

fn ensure_wallet_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<()> {
    tx.execute(
        "INSERT OR IGNORE INTO pet_wallets (
            pet_id, currency_key, balance, lifetime_earned, lifetime_spent, updated_at
         ) VALUES (?1, ?2, 0, 0, 0, ?3)",
        params![PET_ID, LP, now],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn load_wallet(conn: &Connection) -> LocalResult<PetWallet> {
    conn.query_row(
        "SELECT pet_id, currency_key, balance, lifetime_earned, lifetime_spent, updated_at
         FROM pet_wallets
         WHERE pet_id = ?1 AND currency_key = ?2",
        params![PET_ID, LP],
        map_wallet,
    )
    .map_err(|err| err.to_string())
}

fn load_wallet_tx(tx: &Transaction<'_>) -> LocalResult<PetWallet> {
    tx.query_row(
        "SELECT pet_id, currency_key, balance, lifetime_earned, lifetime_spent, updated_at
         FROM pet_wallets
         WHERE pet_id = ?1 AND currency_key = ?2",
        params![PET_ID, LP],
        map_wallet,
    )
    .map_err(|err| err.to_string())
}

fn save_wallet_tx(tx: &Transaction<'_>, wallet: &PetWallet) -> LocalResult<()> {
    tx.execute(
        "UPDATE pet_wallets
         SET balance = ?3, lifetime_earned = ?4, lifetime_spent = ?5, updated_at = ?6
         WHERE pet_id = ?1 AND currency_key = ?2",
        params![
            wallet.pet_id,
            wallet.currency_key,
            wallet.balance,
            wallet.lifetime_earned,
            wallet.lifetime_spent,
            wallet.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn map_wallet(row: &rusqlite::Row<'_>) -> rusqlite::Result<PetWallet> {
    Ok(PetWallet {
        pet_id: row.get(0)?,
        currency_key: row.get(1)?,
        balance: row.get(2)?,
        lifetime_earned: row.get(3)?,
        lifetime_spent: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn inventory_record(
    item_key: &str,
    item_type: &str,
    slot: &str,
    quantity: i64,
    equipped: bool,
    source: &str,
    now: &str,
) -> PetInventoryItem {
    PetInventoryItem {
        id: inventory_id(item_key),
        pet_id: PET_ID.into(),
        item_key: item_key.into(),
        item_type: item_type.into(),
        slot: slot.into(),
        quantity,
        equipped,
        source: source.into(),
        purchased_at: now.into(),
        updated_at: now.into(),
    }
}

fn inventory_id(item_key: &str) -> String {
    format!("pet-inventory-{PET_ID}-{item_key}")
}

fn upsert_inventory_tx(tx: &Transaction<'_>, item: &PetInventoryItem) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO pet_inventory (
            id, pet_id, item_key, item_type, slot, quantity, equipped, source, purchased_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(pet_id, item_key) DO UPDATE SET
            quantity = MAX(pet_inventory.quantity, excluded.quantity),
            equipped = CASE WHEN pet_inventory.equipped = 1 THEN 1 ELSE excluded.equipped END,
            updated_at = excluded.updated_at",
        params![
            item.id,
            item.pet_id,
            item.item_key,
            item.item_type,
            item.slot,
            item.quantity,
            if item.equipped { 1 } else { 0 },
            item.source,
            item.purchased_at,
            item.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn inventory_exists_tx(tx: &Transaction<'_>, item_key: &str) -> LocalResult<bool> {
    tx.query_row(
        "SELECT id FROM pet_inventory WHERE pet_id = ?1 AND item_key = ?2",
        params![PET_ID, item_key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|err| err.to_string())
}

fn load_inventory_item_tx(
    tx: &Transaction<'_>,
    item_key: &str,
) -> LocalResult<Option<PetInventoryItem>> {
    tx.query_row(
        "SELECT id, pet_id, item_key, item_type, slot, quantity, equipped, source, purchased_at, updated_at
         FROM pet_inventory
         WHERE pet_id = ?1 AND item_key = ?2",
        params![PET_ID, item_key],
        map_inventory_item,
    )
    .optional()
    .map_err(|err| err.to_string())
}

fn list_inventory(conn: &Connection) -> LocalResult<Vec<PetInventoryItem>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pet_id, item_key, item_type, slot, quantity, equipped, source, purchased_at, updated_at
             FROM pet_inventory
             WHERE pet_id = ?1 AND quantity > 0
             ORDER BY datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID], map_inventory_item)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn list_inventory_tx(tx: &Transaction<'_>) -> LocalResult<Vec<PetInventoryItem>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, pet_id, item_key, item_type, slot, quantity, equipped, source, purchased_at, updated_at
             FROM pet_inventory
             WHERE pet_id = ?1 AND quantity > 0
             ORDER BY datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID], map_inventory_item)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn map_inventory_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<PetInventoryItem> {
    Ok(PetInventoryItem {
        id: row.get(0)?,
        pet_id: row.get(1)?,
        item_key: row.get(2)?,
        item_type: row.get(3)?,
        slot: row.get(4)?,
        quantity: row.get(5)?,
        equipped: row.get::<_, i64>(6)? != 0,
        source: row.get(7)?,
        purchased_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn equipment_state(inventory: &[PetInventoryItem]) -> PetEquipmentState {
    PetEquipmentState {
        current_pet: equipped_item(inventory, "pet"),
        accessory: equipped_item(inventory, "accessory"),
        scene: equipped_item(inventory, "scene"),
        badge: equipped_item(inventory, "badge"),
    }
}

fn equipped_item(inventory: &[PetInventoryItem], slot: &str) -> Option<PetInventoryItem> {
    inventory
        .iter()
        .find(|item| item.slot == slot && item.equipped)
        .cloned()
}

#[allow(clippy::too_many_arguments)]
fn insert_economy_entry_tx(
    tx: &Transaction<'_>,
    entry_type: &str,
    amount: i64,
    balance_after: i64,
    source_type: &str,
    source_key: &str,
    metadata: Option<&str>,
    now: &str,
) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO pet_economy_ledger (
            id, pet_id, entry_type, currency_key, amount, balance_after,
            source_type, source_key, metadata, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            ids::timestamped_id("pet-economy"),
            PET_ID,
            entry_type,
            LP,
            amount,
            balance_after,
            source_type,
            source_key,
            metadata,
            now
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn economy_entry_exists_tx(
    tx: &Transaction<'_>,
    source_type: &str,
    source_key: &str,
) -> LocalResult<bool> {
    tx.query_row(
        "SELECT id FROM pet_economy_ledger
         WHERE pet_id = ?1 AND currency_key = ?2 AND source_type = ?3 AND source_key = ?4",
        params![PET_ID, LP, source_type, source_key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|err| err.to_string())
}

fn list_economy(conn: &Connection, limit: usize) -> LocalResult<Vec<PetEconomyEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pet_id, entry_type, currency_key, amount, balance_after,
                    source_type, source_key, metadata, created_at
             FROM pet_economy_ledger
             WHERE pet_id = ?1
             ORDER BY datetime(created_at) DESC, created_at DESC
             LIMIT ?2",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID, limit as i64], map_economy_entry)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn list_today_daily_limits(conn: &Connection) -> LocalResult<HashMap<String, i64>> {
    let limit_date = current_store_limit_date();
    let mut stmt = conn
        .prepare(
            "SELECT item_key, free_claimed
             FROM pet_store_daily_limits
             WHERE pet_id = ?1 AND limit_date = ?2",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID, limit_date], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|err| err.to_string())?;
    let mut values = HashMap::new();
    for row in rows {
        let (item_key, free_claimed) = row.map_err(|err| err.to_string())?;
        values.insert(item_key, free_claimed);
    }
    Ok(values)
}

fn map_economy_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<PetEconomyEntry> {
    Ok(PetEconomyEntry {
        id: row.get(0)?,
        pet_id: row.get(1)?,
        entry_type: row.get(2)?,
        currency_key: row.get(3)?,
        amount: row.get(4)?,
        balance_after: row.get(5)?,
        source_type: row.get(6)?,
        source_key: row.get(7)?,
        metadata: row.get(8)?,
        created_at: row.get(9)?,
    })
}

fn load_counter_tx(
    tx: &Transaction<'_>,
    counter_key: &str,
) -> LocalResult<Option<PetMilestoneCounter>> {
    tx.query_row(
        "SELECT pet_id, counter_key, counter_value, last_event_key, updated_at
         FROM pet_milestone_counters
         WHERE pet_id = ?1 AND counter_key = ?2",
        params![PET_ID, counter_key],
        map_counter,
    )
    .optional()
    .map_err(|err| err.to_string())
}

fn list_counters(conn: &Connection) -> LocalResult<Vec<PetMilestoneCounter>> {
    let mut stmt = conn
        .prepare(
            "SELECT pet_id, counter_key, counter_value, last_event_key, updated_at
             FROM pet_milestone_counters
             WHERE pet_id = ?1
             ORDER BY counter_key ASC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID], map_counter)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn list_counters_tx(tx: &Transaction<'_>) -> LocalResult<Vec<PetMilestoneCounter>> {
    let mut stmt = tx
        .prepare(
            "SELECT pet_id, counter_key, counter_value, last_event_key, updated_at
             FROM pet_milestone_counters
             WHERE pet_id = ?1
             ORDER BY counter_key ASC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID], map_counter)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn map_counter(row: &rusqlite::Row<'_>) -> rusqlite::Result<PetMilestoneCounter> {
    Ok(PetMilestoneCounter {
        pet_id: row.get(0)?,
        counter_key: row.get(1)?,
        counter_value: row.get(2)?,
        last_event_key: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile(level: i64, stage: &str) -> PetProfile {
        let experience = crate::local_db::pet_leveling::total_required_exp_for_level(level);
        let level_snapshot =
            crate::local_db::pet_leveling::level_snapshot_from_experience(experience);
        PetProfile {
            id: PET_ID.into(),
            name: "Libby".into(),
            level,
            experience,
            stage: stage.into(),
            level_snapshot,
            current_mood: "idle".into(),
            created_at: "".into(),
            updated_at: "".into(),
        }
    }

    #[test]
    fn food_growth_values_are_explicit_and_stable() {
        let cookie = find_catalog_item("chocolate-chip-cookie-food").expect("cookie seed");
        let shortcake = find_catalog_item("strawberry-shortcake-food").expect("shortcake seed");
        assert_eq!(growth_value_for_food(&cookie), 4);
        assert_eq!(growth_value_for_food(&shortcake), 18);
    }

    #[test]
    fn item_status_prioritizes_badge_achievement_before_locked() {
        let badge = find_catalog_item("sun-badge").expect("badge seed");
        let state = item_state(
            badge,
            &test_profile(1, "first_meet"),
            &[],
            &[],
            &PetWallet {
                pet_id: PET_ID.into(),
                currency_key: LP.into(),
                balance: 0,
                lifetime_earned: 0,
                lifetime_spent: 0,
                updated_at: "".into(),
            },
            &HashMap::new(),
        );
        assert_eq!(state.status, "achievement");
    }
}
