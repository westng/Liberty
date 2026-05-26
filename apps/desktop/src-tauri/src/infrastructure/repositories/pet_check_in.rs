use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::json;

use crate::{
    infrastructure::{
        ids,
        repositories::{pet, pet_store},
    },
    local_db::{
        pet_leveling, LocalResult, PetDailyCheckInClaimResult, PetDailyCheckInEntry,
        PetDailyCheckInRewardPreview, PetDailyCheckInState, PetEventLedgerEntry, PetProfile,
        PetRewardItem, PetStoreCatalogItem,
    },
};

const PET_ID: &str = "default-pet";
const SOURCE_TYPE: &str = "daily_check_in";
const MILESTONE_TRACK_LENGTH: i64 = 14;
const BASE_REWARD_LP: i64 = 10;
const BASE_GROWTH_VALUE: i64 = 5;

#[derive(Clone)]
struct RewardRule {
    cycle_day: i64,
    reward_lp: i64,
    growth_value: i64,
    items: &'static [(&'static str, i64)],
}

const REWARD_RULES: &[RewardRule] = &[
    reward_rule(1, 10, 5, &[]),
    reward_rule(2, 10, 5, &[]),
    reward_rule(3, 15, 5, &[("gift-box-tool", 1)]),
    reward_rule(4, 10, 5, &[]),
    reward_rule(5, 15, 5, &[("cupcake-food", 1)]),
    reward_rule(6, 10, 5, &[]),
    reward_rule(7, 20, 5, &[("clover-badge", 1)]),
    reward_rule(8, 10, 5, &[]),
    reward_rule(9, 10, 5, &[]),
    reward_rule(10, 20, 5, &[("gift-box-tool", 1)]),
    reward_rule(11, 10, 5, &[]),
    reward_rule(12, 15, 5, &[("ice-cream-cone-food", 1)]),
    reward_rule(13, 10, 5, &[]),
    reward_rule(14, 30, 5, &[("sprout-badge", 1)]),
];

const fn reward_rule(
    cycle_day: i64,
    reward_lp: i64,
    growth_value: i64,
    items: &'static [(&'static str, i64)],
) -> RewardRule {
    RewardRule {
        cycle_day,
        reward_lp,
        growth_value,
        items,
    }
}

pub fn daily_check_in_state(
    conn: &Connection,
    profile: PetProfile,
) -> LocalResult<PetDailyCheckInState> {
    let check_in_date = current_check_in_date();
    let history = list_history(conn, 20)?;
    let checked_in_today = history
        .iter()
        .any(|entry| entry.check_in_date == check_in_date);
    let current_streak = if checked_in_today {
        history
            .iter()
            .find(|entry| entry.check_in_date == check_in_date)
            .map(|entry| entry.streak_count)
            .unwrap_or(0)
    } else {
        next_streak_from_history(&history, &check_in_date)
    };
    let next_cycle_day = current_streak.max(1);
    let today_reward = preview_for_streak_day(next_cycle_day);
    let rewards = REWARD_RULES.iter().map(preview_from_rule).collect();
    let store_state = pet_store::store_state(conn, profile)?;
    Ok(PetDailyCheckInState {
        check_in_date,
        checked_in_today,
        current_streak,
        next_cycle_day,
        cycle_length: MILESTONE_TRACK_LENGTH,
        today_reward,
        rewards,
        history,
        store_state,
    })
}

pub fn claim_daily_check_in_tx(
    tx: &Transaction<'_>,
    profile: &PetProfile,
    now: &str,
) -> LocalResult<(PetDailyCheckInEntry, bool)> {
    let check_in_date = current_check_in_date();
    if let Some(existing) = load_entry_for_date_tx(tx, &check_in_date)? {
        return Ok((existing, true));
    }

    let recent_history = list_history_tx(tx, 30)?;
    let streak_count = next_streak_from_history(&recent_history, &check_in_date).max(1);
    let cycle_day = streak_count.max(1);
    let rule = reward_for_streak_day(cycle_day);
    let reward_items = grant_rule_items_tx(tx, &rule, now)?;
    let source_key = format!("daily-check-in:{check_in_date}");
    let metadata = json!({
        "date": check_in_date,
        "streakCount": streak_count,
        "cycleDay": cycle_day,
        "rewardLp": rule.reward_lp,
        "growthValue": rule.growth_value,
        "items": reward_items,
    })
    .to_string();
    pet_store::grant_reward_tx(
        tx,
        SOURCE_TYPE,
        &source_key,
        rule.reward_lp,
        Some(&metadata),
        now,
    )?;
    grant_growth_tx(tx, profile, rule.growth_value, now)?;

    let entry = PetDailyCheckInEntry {
        id: ids::timestamped_id("pet-check-in"),
        pet_id: PET_ID.into(),
        check_in_date: check_in_date.clone(),
        streak_count,
        cycle_day,
        reward_lp: rule.reward_lp,
        growth_value: rule.growth_value,
        reward_items,
        created_at: now.into(),
    };
    insert_entry_tx(tx, &entry)?;
    insert_check_in_event_tx(tx, &entry, now)?;
    Ok((entry, false))
}

pub fn claim_result(
    conn: &Connection,
    profile: PetProfile,
    entry: PetDailyCheckInEntry,
    duplicate: bool,
) -> LocalResult<PetDailyCheckInClaimResult> {
    Ok(PetDailyCheckInClaimResult {
        state: daily_check_in_state(conn, profile)?,
        entry,
        duplicate,
    })
}

fn grant_rule_items_tx(
    tx: &Transaction<'_>,
    rule: &RewardRule,
    now: &str,
) -> LocalResult<Vec<PetRewardItem>> {
    let mut granted = Vec::new();
    for (item_key, quantity) in rule.items {
        let Some(item) = pet_store::find_catalog_item_by_key(item_key) else {
            continue;
        };
        let duplicate_compensation_lp =
            if item.slot != "consumable" && inventory_exists_tx(tx, &item.item_key)? {
                pet_store::duplicate_compensation_lp_for_item(&item)
            } else {
                pet_store::grant_catalog_item_tx(tx, &item, *quantity, SOURCE_TYPE, now)?;
                0
            };
        if duplicate_compensation_lp > 0 {
            pet_store::grant_reward_tx(
                tx,
                "daily_check_in_duplicate",
                &format!(
                    "daily-check-in-duplicate:{}:{}",
                    current_check_in_date(),
                    item.item_key
                ),
                duplicate_compensation_lp,
                Some(&item.item_key),
                now,
            )?;
        }
        granted.push(PetRewardItem {
            item_key: item.item_key,
            item_type: item.item_type,
            quantity: *quantity,
            duplicate_compensation_lp,
        });
    }
    Ok(granted)
}

fn grant_growth_tx(
    tx: &Transaction<'_>,
    profile: &PetProfile,
    growth_value: i64,
    now: &str,
) -> LocalResult<()> {
    if growth_value <= 0 {
        return Ok(());
    }
    let mut next_profile = profile.clone();
    let previous_stage = next_profile.stage.clone();
    next_profile.experience = (next_profile.experience + growth_value).max(0);
    let snapshot = pet_leveling::level_snapshot_from_experience(next_profile.experience);
    next_profile.level = snapshot.level;
    next_profile.stage = snapshot.current_stage.clone();
    next_profile.level_snapshot = snapshot;
    next_profile.current_mood = "cheerful".into();
    next_profile.updated_at = now.into();
    pet::save_profile_tx(tx, &next_profile)?;
    pet::ensure_stage_cosmetic_unlocks_tx(tx, &next_profile, &previous_stage, now)
}

fn insert_check_in_event_tx(
    tx: &Transaction<'_>,
    entry: &PetDailyCheckInEntry,
    now: &str,
) -> LocalResult<()> {
    let line_zh = if entry.reward_items.is_empty() {
        format!(
            "今日签到完成，连续 {} 天。LP +{}，成长值 +{}。",
            entry.streak_count, entry.reward_lp, entry.growth_value
        )
    } else {
        format!(
            "今日签到完成，连续 {} 天。奖励和商店物品都已收好。",
            entry.streak_count
        )
    };
    let line_en = if entry.reward_items.is_empty() {
        format!(
            "Daily check-in complete. Streak {} days. LP +{}, growth +{}.",
            entry.streak_count, entry.reward_lp, entry.growth_value
        )
    } else {
        format!(
            "Daily check-in complete. Streak {} days. Rewards and store items are saved.",
            entry.streak_count
        )
    };
    let metadata = json!({
        "source": SOURCE_TYPE,
        "date": entry.check_in_date,
        "streakCount": entry.streak_count,
        "cycleDay": entry.cycle_day,
        "finalLp": entry.reward_lp,
        "growthValue": entry.growth_value,
        "items": entry.reward_items,
        "zh": line_zh,
        "en": line_en,
    })
    .to_string();
    pet::insert_event_ledger_tx(
        tx,
        &PetEventLedgerEntry {
            id: ids::timestamped_id("pet-check-in-event"),
            pet_id: PET_ID.into(),
            event_type: "daily_check_in".into(),
            event_source: SOURCE_TYPE.into(),
            event_value: entry.growth_value,
            event_time: now.into(),
            metadata: Some(metadata),
        },
    )
}

fn preview_for_streak_day(streak_day: i64) -> PetDailyCheckInRewardPreview {
    let rule = reward_for_streak_day(streak_day);
    preview_from_rule(&rule)
}

fn preview_from_rule(rule: &RewardRule) -> PetDailyCheckInRewardPreview {
    PetDailyCheckInRewardPreview {
        cycle_day: rule.cycle_day,
        reward_lp: rule.reward_lp,
        growth_value: rule.growth_value,
        items: rule
            .items
            .iter()
            .filter_map(|(item_key, quantity)| preview_item(item_key, *quantity))
            .collect(),
    }
}

fn preview_item(item_key: &str, quantity: i64) -> Option<PetRewardItem> {
    let item: PetStoreCatalogItem = pet_store::find_catalog_item_by_key(item_key)?;
    Some(PetRewardItem {
        item_key: item.item_key,
        item_type: item.item_type,
        quantity,
        duplicate_compensation_lp: 0,
    })
}

fn reward_for_streak_day(streak_day: i64) -> RewardRule {
    REWARD_RULES
        .iter()
        .find(|rule| rule.cycle_day == streak_day)
        .cloned()
        .unwrap_or_else(|| reward_rule(streak_day, BASE_REWARD_LP, BASE_GROWTH_VALUE, &[]))
}

fn next_streak_from_history(history: &[PetDailyCheckInEntry], check_in_date: &str) -> i64 {
    let Some(previous) = history.first() else {
        return 1;
    };
    if previous.check_in_date == check_in_date {
        return previous.streak_count;
    }
    if is_previous_local_date(&previous.check_in_date, check_in_date) {
        return previous.streak_count.saturating_add(1);
    }
    1
}

fn is_previous_local_date(previous: &str, current: &str) -> bool {
    let Ok(previous_date) = chrono::NaiveDate::parse_from_str(previous, "%Y-%m-%d") else {
        return false;
    };
    let Ok(current_date) = chrono::NaiveDate::parse_from_str(current, "%Y-%m-%d") else {
        return false;
    };
    previous_date
        .succ_opt()
        .is_some_and(|date| date == current_date)
}

fn current_check_in_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn insert_entry_tx(tx: &Transaction<'_>, entry: &PetDailyCheckInEntry) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO pet_daily_check_ins (
            id, pet_id, check_in_date, streak_count, cycle_day, reward_lp,
            growth_value, reward_items_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            entry.id,
            entry.pet_id,
            entry.check_in_date,
            entry.streak_count,
            entry.cycle_day,
            entry.reward_lp,
            entry.growth_value,
            serde_json::to_string(&entry.reward_items).map_err(|err| err.to_string())?,
            entry.created_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn load_entry_for_date_tx(
    tx: &Transaction<'_>,
    check_in_date: &str,
) -> LocalResult<Option<PetDailyCheckInEntry>> {
    tx.query_row(
        "SELECT id, pet_id, check_in_date, streak_count, cycle_day, reward_lp,
                growth_value, reward_items_json, created_at
         FROM pet_daily_check_ins
         WHERE pet_id = ?1 AND check_in_date = ?2",
        params![PET_ID, check_in_date],
        map_entry,
    )
    .optional()
    .map_err(|err| err.to_string())
}

fn list_history(conn: &Connection, limit: usize) -> LocalResult<Vec<PetDailyCheckInEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pet_id, check_in_date, streak_count, cycle_day, reward_lp,
                    growth_value, reward_items_json, created_at
             FROM pet_daily_check_ins
             WHERE pet_id = ?1
             ORDER BY check_in_date DESC, datetime(created_at) DESC
             LIMIT ?2",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID, limit as i64], map_entry)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn list_history_tx(tx: &Transaction<'_>, limit: usize) -> LocalResult<Vec<PetDailyCheckInEntry>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, pet_id, check_in_date, streak_count, cycle_day, reward_lp,
                    growth_value, reward_items_json, created_at
             FROM pet_daily_check_ins
             WHERE pet_id = ?1
             ORDER BY check_in_date DESC, datetime(created_at) DESC
             LIMIT ?2",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![PET_ID, limit as i64], map_entry)
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn inventory_exists_tx(tx: &Transaction<'_>, item_key: &str) -> LocalResult<bool> {
    tx.query_row(
        "SELECT id
         FROM pet_inventory
         WHERE pet_id = ?1 AND item_key = ?2 AND quantity > 0",
        params![PET_ID, item_key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(|err| err.to_string())
}

fn map_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<PetDailyCheckInEntry> {
    let reward_items_json: String = row.get(7)?;
    let reward_items = serde_json::from_str(&reward_items_json).unwrap_or_default();
    Ok(PetDailyCheckInEntry {
        id: row.get(0)?,
        pet_id: row.get(1)?,
        check_in_date: row.get(2)?,
        streak_count: row.get(3)?,
        cycle_day: row.get(4)?,
        reward_lp: row.get(5)?,
        growth_value: row.get(6)?,
        reward_items,
        created_at: row.get(8)?,
    })
}
