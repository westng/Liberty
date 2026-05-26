use rusqlite::{params, Connection, Transaction};
use serde_json::json;

use crate::{
    infrastructure::{ids, repositories::pet_store, time::unix_timestamp_millis},
    local_db::{
        LocalResult, PetBlindBoxDrawEntry, PetBlindBoxDrawResult, PetBlindBoxPoolItem,
        PetBlindBoxState, PetEventLedgerEntry, PetInventoryItem, PetProfile, PetStoreCatalogItem,
    },
};

const PET_ID: &str = "default-pet";
const DAILY_LIMIT: i64 = 10;
const SOURCE_TYPE: &str = "daily_blind_box";
const EMPTY_PRIZE_KEY: &str = "nothing";

pub fn blind_box_state(conn: &Connection, profile: PetProfile) -> LocalResult<PetBlindBoxState> {
    let draw_date = current_draw_date();
    let pool = pool_items(conn)?;
    let used_today = count_draws_for_date(conn, &draw_date)?;
    let history = list_history(conn, 20)?;
    let store_state = pet_store::store_state(conn, profile)?;
    Ok(PetBlindBoxState {
        draw_date,
        daily_limit: DAILY_LIMIT,
        used_today,
        remaining_today: (DAILY_LIMIT - used_today).max(0),
        pool,
        history,
        store_state,
    })
}

pub fn draw_blind_box_tx(
    tx: &Transaction<'_>,
    profile: &PetProfile,
    now: &str,
) -> LocalResult<PetBlindBoxDrawEntry> {
    let draw_date = current_draw_date();
    let used_today = count_draws_for_date_tx(tx, &draw_date)?;
    if used_today >= DAILY_LIMIT {
        return Err("今日每日盲盒次数已用完。".into());
    }

    let pool = pool_items_tx(tx)?;
    if pool.is_empty() {
        return Err("每日盲盒奖池暂时为空。".into());
    }

    let prize = pick_prize(&pool, profile, used_today);
    let is_empty_prize = prize.item.item_key == EMPTY_PRIZE_KEY;
    let duplicate = !is_empty_prize && prize.owned && prize.item.slot != "consumable";
    let compensation_lp = if is_empty_prize {
        0
    } else if duplicate {
        prize.duplicate_compensation_lp
    } else {
        pet_store::grant_catalog_item_tx(tx, &prize.item, 1, SOURCE_TYPE, now)?;
        0
    };

    if compensation_lp > 0 {
        let source_key = format!("daily-blind-box:{}:{}", draw_date, used_today + 1);
        pet_store::grant_reward_tx(
            tx,
            SOURCE_TYPE,
            &source_key,
            compensation_lp,
            Some(&prize.item.item_key),
            now,
        )?;
    }

    insert_blind_box_speech_event_tx(
        tx,
        &profile.id,
        &prize.item,
        is_empty_prize,
        duplicate,
        compensation_lp,
        used_today + 1,
        now,
    )?;

    let draw = PetBlindBoxDrawEntry {
        id: ids::timestamped_id("pet-blind-box"),
        pet_id: PET_ID.into(),
        draw_date,
        item_key: prize.item.item_key,
        item_type: prize.item.item_type,
        quantity: 1,
        duplicate_compensation_lp: compensation_lp,
        created_at: now.into(),
    };
    insert_draw_tx(tx, &draw)?;
    Ok(draw)
}

#[allow(clippy::too_many_arguments)]
fn insert_blind_box_speech_event_tx(
    tx: &Transaction<'_>,
    pet_id: &str,
    item: &PetStoreCatalogItem,
    is_empty_prize: bool,
    duplicate: bool,
    compensation_lp: i64,
    draw_number: i64,
    now: &str,
) -> LocalResult<()> {
    let event_type = if is_empty_prize {
        "blind_box_empty"
    } else if duplicate {
        "blind_box_duplicate"
    } else {
        "blind_box_reward"
    };
    let mood = if is_empty_prize {
        "needy"
    } else if duplicate {
        "cheerful"
    } else {
        "excited"
    };
    let event_value = if compensation_lp > 0 {
        compensation_lp
    } else {
        1
    };
    let line_zh = select_blind_box_dialogue(
        item,
        is_empty_prize,
        duplicate,
        compensation_lp,
        draw_number,
        now,
        "zh-CN",
    );
    let line_en = select_blind_box_dialogue(
        item,
        is_empty_prize,
        duplicate,
        compensation_lp,
        draw_number,
        now,
        "en-US",
    );
    let metadata = json!({
        "itemKey": item.item_key,
        "itemType": item.item_type,
        "nameZh": item.name_zh,
        "nameEn": item.name_en,
        "drawNumber": draw_number,
        "duplicate": duplicate,
        "compensationLp": compensation_lp,
        "mood": mood,
        "zh": line_zh,
        "en": line_en,
    })
    .to_string();

    crate::infrastructure::repositories::pet::insert_event_ledger_tx(
        tx,
        &PetEventLedgerEntry {
            id: ids::timestamped_id(event_type),
            pet_id: pet_id.into(),
            event_type: event_type.into(),
            event_source: SOURCE_TYPE.into(),
            event_value,
            event_time: now.into(),
            metadata: Some(metadata),
        },
    )?;

    let mut profile = crate::infrastructure::repositories::pet::load_profile_tx(tx)?;
    profile.current_mood = mood.into();
    profile.updated_at = now.into();
    crate::infrastructure::repositories::pet::save_profile_tx(tx, &profile)?;
    Ok(())
}

fn select_blind_box_dialogue(
    item: &PetStoreCatalogItem,
    is_empty_prize: bool,
    duplicate: bool,
    compensation_lp: i64,
    draw_number: i64,
    now: &str,
    locale: &str,
) -> String {
    if locale == "en-US" {
        if is_empty_prize {
            let lines = [
                "Nothing came out this time... I am a little disappointed, but I still want to try again with you.",
                "The box was empty. I will stay beside you and save the next bit of luck.",
                "No reward this time. I am pouting a little, but I have not given up.",
                "It missed this time. Let me hold the luck for the next box.",
            ];
            return lines[dialogue_index_with_time(&item.item_key, draw_number, now, lines.len())]
                .to_string();
        }
        if duplicate {
            let lines = [
                format!(
                    "We already have {}, so I tucked away {} LP for us instead.",
                    item.name_en, compensation_lp
                ),
                format!(
                    "{} came back again. I am still happy; it turned into {} LP.",
                    item.name_en, compensation_lp
                ),
                format!(
                    "A familiar gift appeared. I saved the extra care as {} LP.",
                    compensation_lp
                ),
            ];
            return lines
                [dialogue_index_with_time(&item.item_key, compensation_lp, now, lines.len())]
            .clone();
        }
        let lines = [
            format!(
                "I opened {}! My heart jumped a little. This feels like luck arriving.",
                item.name_en
            ),
            format!(
                "Look, {} came out! I want to show it to you right away.",
                item.name_en
            ),
            format!(
                "This box hid {} inside. I feel bright and excited now.",
                item.name_en
            ),
            format!(
                "We got {}. I will keep this surprise carefully with you.",
                item.name_en
            ),
        ];
        return lines[dialogue_index_with_time(&item.item_key, draw_number, now, lines.len())]
            .clone();
    }

    if is_empty_prize {
        let lines = [
            "这次盒子空空的……我有一点点失落，但还是想和你再试一次。",
            "什么都没开出来。我先把下一次的好运抱紧一点，别让它跑掉。",
            "这次没有奖励，我有点想鼓鼓脸，不过我还没有放弃。",
            "它这次和我们擦肩而过了。下一次，让我来帮你守住好运。",
            "空盒子也算一次回应吧……只是它回答得有点小声，我会继续陪你。",
        ];
        return lines[dialogue_index_with_time(&item.item_key, draw_number, now, lines.len())]
            .to_string();
    }

    if duplicate {
        let lines = [
            format!("「{}」又回来啦，虽然已经拥有了，但我把它认真换成了 {compensation_lp} LP。", item.name_zh),
            format!("是熟悉的「{}」。我没有难过哦，它变成 {compensation_lp} LP 留给我们了。", item.name_zh),
            format!("重复奖励也不是白来一趟，我把多出来的心意收成了 {compensation_lp} LP。"),
            format!("这个我们已经有啦。不过能再次遇见它，我还是有点开心，补偿 {compensation_lp} LP 已收好。"),
        ];
        return lines[dialogue_index_with_time(&item.item_key, compensation_lp, now, lines.len())]
            .clone();
    }

    let lines = [
        format!(
            "开到「{}」啦！我刚刚心里亮了一下，像好运真的落到我们手里了。",
            item.name_zh
        ),
        format!(
            "你看你看，是「{}」！我有点激动，想马上把它举给你看。",
            item.name_zh
        ),
        format!(
            "这只盲盒里藏着「{}」。我现在开心得有点坐不住了。",
            item.name_zh
        ),
        format!(
            "我们抽到「{}」啦。我会把这份惊喜认真收好，陪你一起用。",
            item.name_zh
        ),
        format!(
            "「{}」出现的那一刻，我感觉今天的运气被点亮了一点。",
            item.name_zh
        ),
        format!(
            "这份奖励我很喜欢，因为它是我们一起开出来的「{}」。",
            item.name_zh
        ),
    ];
    lines[dialogue_index_with_time(&item.item_key, draw_number, now, lines.len())].clone()
}

pub fn draw_result(
    conn: &Connection,
    profile: PetProfile,
    draw: PetBlindBoxDrawEntry,
) -> LocalResult<PetBlindBoxDrawResult> {
    let prize =
        find_pool_catalog_item(&draw.item_key).ok_or_else(|| "每日盲盒奖励不存在。".to_string())?;
    let duplicate = draw.duplicate_compensation_lp > 0;
    let state = blind_box_state(conn, profile)?;
    Ok(PetBlindBoxDrawResult {
        state,
        draw,
        prize,
        duplicate,
    })
}

fn pool_items(conn: &Connection) -> LocalResult<Vec<PetBlindBoxPoolItem>> {
    let inventory = list_inventory(conn)?;
    let mut pool = pet_store::catalog_items()
        .into_iter()
        .filter(|item| item.enabled && item.item_type != "pet")
        .map(|item| pool_item(item, &inventory))
        .collect::<Vec<_>>();
    pool.push(pool_item(empty_prize_item(), &inventory));
    Ok(pool)
}

fn pool_items_tx(tx: &Transaction<'_>) -> LocalResult<Vec<PetBlindBoxPoolItem>> {
    let inventory = list_inventory_tx(tx)?;
    let mut pool = pet_store::catalog_items()
        .into_iter()
        .filter(|item| item.enabled && item.item_type != "pet")
        .map(|item| pool_item(item, &inventory))
        .collect::<Vec<_>>();
    pool.push(pool_item(empty_prize_item(), &inventory));
    Ok(pool)
}

fn pool_item(item: PetStoreCatalogItem, inventory: &[PetInventoryItem]) -> PetBlindBoxPoolItem {
    let owned = inventory
        .iter()
        .any(|value| value.item_key == item.item_key);
    let weight = item_weight(&item);
    let duplicate_compensation_lp = duplicate_compensation_lp(&item);
    PetBlindBoxPoolItem {
        item,
        owned,
        weight,
        duplicate_compensation_lp,
    }
}

fn pick_prize(
    pool: &[PetBlindBoxPoolItem],
    profile: &PetProfile,
    used_today: i64,
) -> PetBlindBoxPoolItem {
    let total_weight: i64 = pool.iter().map(|item| item.weight.max(1)).sum();
    let seed = unix_timestamp_millis() as i64
        + profile.experience.saturating_mul(31)
        + used_today.saturating_mul(97);
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

fn item_weight(item: &PetStoreCatalogItem) -> i64 {
    let type_weight = match item.item_type.as_str() {
        "none" => 18,
        "food" => 48,
        "tool" => 30,
        "cosmetic" => 12,
        "theme" => 8,
        "badge" => 4,
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

fn duplicate_compensation_lp(item: &PetStoreCatalogItem) -> i64 {
    if item.item_type == "none" {
        return 0;
    }
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

fn find_pool_catalog_item(item_key: &str) -> Option<PetStoreCatalogItem> {
    if item_key == EMPTY_PRIZE_KEY {
        return Some(empty_prize_item());
    }
    pet_store::catalog_items()
        .into_iter()
        .find(|item| item.item_key == item_key)
}

fn empty_prize_item() -> PetStoreCatalogItem {
    PetStoreCatalogItem {
        item_key: EMPTY_PRIZE_KEY.into(),
        item_type: "none".into(),
        slot: "none".into(),
        name_zh: "什么都没抽中".into(),
        name_en: "Nothing This Time".into(),
        description_zh: "这次盲盒没有获得物品，今日次数仍会正常消耗。".into(),
        description_en:
            "No item was gained from this opening. The daily attempt is still consumed.".into(),
        rarity: "first_meet".into(),
        price_lp: 0,
        level_gate: 1,
        stage_gate: "".into(),
        milestone_gate: "".into(),
        asset_key: "gift_box".into(),
        growth_value: 0,
        enabled: true,
        sort_order: 9999,
    }
}

fn current_draw_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn count_draws_for_date(conn: &Connection, draw_date: &str) -> LocalResult<i64> {
    conn.query_row(
        "SELECT COUNT(*)
         FROM pet_blind_box_draws
         WHERE pet_id = ?1 AND draw_date = ?2",
        params![PET_ID, draw_date],
        |row| row.get(0),
    )
    .map_err(|err| err.to_string())
}

fn count_draws_for_date_tx(tx: &Transaction<'_>, draw_date: &str) -> LocalResult<i64> {
    tx.query_row(
        "SELECT COUNT(*)
         FROM pet_blind_box_draws
         WHERE pet_id = ?1 AND draw_date = ?2",
        params![PET_ID, draw_date],
        |row| row.get(0),
    )
    .map_err(|err| err.to_string())
}

fn insert_draw_tx(tx: &Transaction<'_>, draw: &PetBlindBoxDrawEntry) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO pet_blind_box_draws (
            id, pet_id, draw_date, item_key, item_type, quantity,
            duplicate_compensation_lp, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            draw.id,
            draw.pet_id,
            draw.draw_date,
            draw.item_key,
            draw.item_type,
            draw.quantity,
            draw.duplicate_compensation_lp,
            draw.created_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn list_history(conn: &Connection, limit: usize) -> LocalResult<Vec<PetBlindBoxDrawEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pet_id, draw_date, item_key, item_type, quantity,
                    duplicate_compensation_lp, created_at
             FROM pet_blind_box_draws
             WHERE pet_id = ?1
             ORDER BY datetime(created_at) DESC, created_at DESC
             LIMIT ?2",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID, limit as i64], map_draw_entry)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn list_inventory(conn: &Connection) -> LocalResult<Vec<PetInventoryItem>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pet_id, item_key, item_type, slot, quantity, equipped, source, purchased_at, updated_at
             FROM pet_inventory
             WHERE pet_id = ?1 AND quantity > 0",
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
             WHERE pet_id = ?1 AND quantity > 0",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID], map_inventory_item)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn map_draw_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<PetBlindBoxDrawEntry> {
    Ok(PetBlindBoxDrawEntry {
        id: row.get(0)?,
        pet_id: row.get(1)?,
        draw_date: row.get(2)?,
        item_key: row.get(3)?,
        item_type: row.get(4)?,
        quantity: row.get(5)?,
        duplicate_compensation_lp: row.get(6)?,
        created_at: row.get(7)?,
    })
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
