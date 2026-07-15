use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, OptionalExtension, Transaction};

use crate::{
    infrastructure::repositories::{pet, pet_store, work_game},
    local_db::{
        FarmCropConfig, FarmHarvestLedgerEntry, FarmPlot, FarmRewardItem, FarmState, LocalResult,
        WorkMap, WorkMapSummary, WorkMarketState,
    },
};

const PLOT_COUNT: i64 = 3;
const STATUS_EMPTY: &str = "empty";
const STATUS_PLANTED: &str = "planted";
const STATUS_NEEDS_WATER: &str = "needs_water";
const STATUS_MATURE: &str = "mature";

struct FarmCropSeed {
    crop_key: &'static str,
    seed_item_key: &'static str,
    name_zh: &'static str,
    name_en: &'static str,
    description_zh: &'static str,
    description_en: &'static str,
    duration_seconds: i64,
    water_required: i64,
    primary_reward_item_key: &'static str,
    primary_reward_quantity: i64,
    bonus_reward_item_key: Option<&'static str>,
    bonus_chance_percent: i64,
    lp_min: i64,
    lp_max: i64,
}

const CROP_SEEDS: &[FarmCropSeed] = &[
    FarmCropSeed {
        crop_key: "wheat",
        seed_item_key: "wheat-seed",
        name_zh: "小麦",
        name_en: "Wheat",
        description_zh: "新手作物，成熟很快，适合熟悉播种和收获。",
        description_en: "A starter crop with a short loop for learning the farm.",
        duration_seconds: 5 * 60,
        water_required: 1,
        primary_reward_item_key: "wheat-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: None,
        bonus_chance_percent: 0,
        lp_min: 1,
        lp_max: 3,
    },
    FarmCropSeed {
        crop_key: "carrot",
        seed_item_key: "carrot-seed",
        name_zh: "胡萝卜",
        name_en: "Carrot",
        description_zh: "稳定作物，主要产出日常投喂食物。",
        description_en: "A stable crop that mostly yields everyday feeding items.",
        duration_seconds: 15 * 60,
        water_required: 2,
        primary_reward_item_key: "carrot-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("cupcake-food"),
        bonus_chance_percent: 25,
        lp_min: 3,
        lp_max: 6,
    },
    FarmCropSeed {
        crop_key: "tomato",
        seed_item_key: "tomato-seed",
        name_zh: "番茄",
        name_en: "Tomato",
        description_zh: "中周期作物，有机会额外收获互动道具。",
        description_en: "A mid-cycle crop with a chance to grant an interaction tool.",
        duration_seconds: 30 * 60,
        water_required: 2,
        primary_reward_item_key: "tomato-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("energy-drink-tool"),
        bonus_chance_percent: 20,
        lp_min: 6,
        lp_max: 10,
    },
    FarmCropSeed {
        crop_key: "pumpkin",
        seed_item_key: "pumpkin-seed",
        name_zh: "南瓜",
        name_en: "Pumpkin",
        description_zh: "高收益作物，成熟较慢，有机会产出惊喜礼盒。",
        description_en: "A slower high-yield crop with a chance to grant a gift box.",
        duration_seconds: 60 * 60,
        water_required: 3,
        primary_reward_item_key: "pumpkin-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some(pet_store::GIFT_BOX_ITEM_KEY),
        bonus_chance_percent: 15,
        lp_min: 10,
        lp_max: 18,
    },
    FarmCropSeed {
        crop_key: "corn",
        seed_item_key: "corn-seed",
        name_zh: "玉米",
        name_en: "Corn",
        description_zh: "中短周期作物，稳定产出能量补给。",
        description_en: "A mid-short cycle crop that yields reliable energy supplies.",
        duration_seconds: 20 * 60,
        water_required: 2,
        primary_reward_item_key: "corn-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("energy-drink-tool"),
        bonus_chance_percent: 18,
        lp_min: 4,
        lp_max: 7,
    },
    FarmCropSeed {
        crop_key: "strawberry",
        seed_item_key: "strawberry-seed",
        name_zh: "草莓",
        name_en: "Strawberry",
        description_zh: "甜点作物，成熟后产出高级投喂食物。",
        description_en: "A dessert crop that yields premium feeding treats.",
        duration_seconds: 25 * 60,
        water_required: 2,
        primary_reward_item_key: "strawberry-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("pink-donut-food"),
        bonus_chance_percent: 18,
        lp_min: 5,
        lp_max: 8,
    },
    FarmCropSeed {
        crop_key: "blueberry",
        seed_item_key: "blueberry-seed",
        name_zh: "蓝莓",
        name_en: "Blueberry",
        description_zh: "较长周期浆果作物，有更高概率额外收获甜点。",
        description_en: "A longer berry crop with a better chance of bonus sweets.",
        duration_seconds: 35 * 60,
        water_required: 3,
        primary_reward_item_key: "blueberry-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("rainbow-crystal-tool"),
        bonus_chance_percent: 24,
        lp_min: 7,
        lp_max: 11,
    },
    FarmCropSeed {
        crop_key: "potato",
        seed_item_key: "potato-seed",
        name_zh: "土豆",
        name_en: "Potato",
        description_zh: "便宜稳定的基础作物，适合持续种植。",
        description_en: "A cheap and steady base crop for continuous planting.",
        duration_seconds: 12 * 60,
        water_required: 1,
        primary_reward_item_key: "potato-harvest-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("sandwich-food"),
        bonus_chance_percent: 12,
        lp_min: 2,
        lp_max: 4,
    },
];

pub fn crop_configs() -> Vec<FarmCropConfig> {
    CROP_SEEDS.iter().map(crop_config_from_seed).collect()
}

pub fn state_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<FarmState> {
    ensure_default_plots_tx(tx, now)?;
    refresh_plot_progress_tx(tx, now)?;
    let plots = list_plots_tx(tx, now)?;
    let harvests = list_harvest_ledger_tx(tx, 12)?;
    Ok(FarmState {
        map_status: map_status_from_plots(&plots),
        plots,
        crops: crop_configs(),
        harvests,
        updated_at: now.into(),
    })
}

pub fn work_market_state_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<WorkMarketState> {
    let farm_state = state_tx(tx, now)?;
    let summary = farm_summary(&farm_state);
    let mut maps = vec![WorkMap {
        id: "farm".into(),
        name_zh: "农场种菜".into(),
        name_en: "Farm".into(),
        description_zh: "播种、浇水、等待成熟，收获宠物商品。".into(),
        description_en: "Plant, water, wait, and harvest pet items.".into(),
        category: "farm".into(),
        status: farm_state.map_status,
        route: "/farm".into(),
        outputs: vec!["food".into(), "lp".into(), "tool".into(), "gift_box".into()],
        enabled: true,
        summary,
    }];
    maps.extend(work_game::work_maps_tx(tx, now)?);
    Ok(WorkMarketState {
        maps,
        updated_at: now.into(),
    })
}

pub fn plant_crop_tx(
    tx: &Transaction<'_>,
    plot_id: &str,
    crop_key: &str,
    now: &str,
) -> LocalResult<FarmState> {
    ensure_default_plots_tx(tx, now)?;
    refresh_plot_progress_tx(tx, now)?;
    let plot = load_plot_tx(tx, plot_id)?;
    if plot.status != STATUS_EMPTY {
        return Err("这块地还不能播种。".into());
    }
    let crop = find_crop_config(crop_key).ok_or_else(|| "作物不存在。".to_string())?;
    pet_store::consume_inventory_item_tx(tx, &crop.seed_item_key, 1, now)
        .map_err(|_| format!("缺少{}，请先去宠物商店购买。", seed_name_zh(&crop)))?;
    let next_care_at = next_stage_at(now, &crop)?;
    tx.execute(
        "UPDATE farm_plots
         SET crop_key = ?2,
             status = ?3,
             stage_index = 0,
             planted_at = ?4,
             last_watered_at = NULL,
             next_care_at = ?5,
             mature_at = NULL,
             updated_at = ?4
         WHERE id = ?1",
        params![plot.id, crop.crop_key, STATUS_PLANTED, now, next_care_at],
    )
    .map_err(|err| err.to_string())?;
    state_tx(tx, now)
}

pub fn water_plot_tx(tx: &Transaction<'_>, plot_id: &str, now: &str) -> LocalResult<FarmState> {
    ensure_default_plots_tx(tx, now)?;
    refresh_plot_progress_tx(tx, now)?;
    let plot = load_plot_tx(tx, plot_id)?;
    if plot.status != STATUS_NEEDS_WATER {
        return Err("这块地暂时不需要浇水。".into());
    }
    let crop = find_crop_config(&plot.crop_key).ok_or_else(|| "作物配置不存在。".to_string())?;
    let next_stage_index = plot.stage_index + 1;
    if next_stage_index >= crop.water_required {
        let mature_at = next_stage_at(now, &crop)?;
        tx.execute(
            "UPDATE farm_plots
             SET status = ?2,
                 stage_index = ?3,
                 last_watered_at = ?4,
                 next_care_at = ?5,
                 mature_at = ?5,
                 updated_at = ?4
             WHERE id = ?1",
            params![plot.id, STATUS_PLANTED, next_stage_index, now, mature_at],
        )
        .map_err(|err| err.to_string())?;
    } else {
        let next_care_at = next_stage_at(now, &crop)?;
        tx.execute(
            "UPDATE farm_plots
             SET status = ?2,
                 stage_index = ?3,
                 last_watered_at = ?4,
                 next_care_at = ?5,
                 mature_at = NULL,
                 updated_at = ?4
             WHERE id = ?1",
            params![plot.id, STATUS_PLANTED, next_stage_index, now, next_care_at],
        )
        .map_err(|err| err.to_string())?;
    }
    state_tx(tx, now)
}

pub fn harvest_plot_tx(
    tx: &Transaction<'_>,
    plot_id: &str,
    now: &str,
) -> LocalResult<(FarmState, FarmHarvestLedgerEntry)> {
    ensure_default_plots_tx(tx, now)?;
    refresh_plot_progress_tx(tx, now)?;
    pet::ensure_default_exists_tx(tx)?;
    pet_store::ensure_store_defaults_tx(tx, now)?;
    let plot = load_plot_tx(tx, plot_id)?;
    if plot.status != STATUS_MATURE {
        return Err("作物还没有成熟。".into());
    }
    let crop = find_crop_config(&plot.crop_key).ok_or_else(|| "作物配置不存在。".to_string())?;
    let event_key = harvest_event_key(&plot, &crop)?;
    let entry_id = format!("farm-harvest:{event_key}");
    if !pet_store::claim_event_key_tx(tx, "farm_harvest", &event_key, Some(&crop.crop_key), now)? {
        let entry = load_harvest_entry_tx(tx, &entry_id)?
            .ok_or_else(|| "这轮收获已经领取，无法恢复原收获记录。".to_string())?;
        clear_plot_tx(tx, &plot.id, now)?;
        return Ok((state_tx(tx, now)?, entry));
    }
    let rewards = reward_items_for_crop(&crop, &event_key);
    let lp_reward = lp_reward_for_crop(&crop, &event_key);
    for reward in &rewards {
        let item = pet_store::find_catalog_item_by_key(&reward.item_key)
            .ok_or_else(|| format!("宠物商品不存在: {}", reward.item_key))?;
        pet_store::grant_catalog_item_tx(tx, &item, reward.quantity, "farm_harvest", now)?;
    }
    if lp_reward > 0 {
        pet_store::grant_reward_tx(
            tx,
            "farm_harvest",
            &event_key,
            lp_reward,
            Some(&crop.crop_key),
            now,
        )?;
    }
    let entry = FarmHarvestLedgerEntry {
        id: entry_id,
        plot_id: plot.id.clone(),
        crop_key: crop.crop_key.clone(),
        rewards: rewards.clone(),
        lp_reward,
        created_at: now.into(),
    };
    insert_harvest_ledger_tx(tx, &entry)?;
    clear_plot_tx(tx, &plot.id, now)?;
    let state = state_tx(tx, now)?;
    Ok((state, entry))
}

pub fn list_harvest_ledger_tx(
    tx: &Transaction<'_>,
    limit: usize,
) -> LocalResult<Vec<FarmHarvestLedgerEntry>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, plot_id, crop_key, rewards_json, lp_reward, created_at
             FROM farm_harvest_ledger
             ORDER BY datetime(created_at) DESC, created_at DESC
             LIMIT ?1",
        )
        .map_err(|err| err.to_string())?;
    let entries = stmt
        .query_map(params![limit as i64], map_harvest_entry)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(entries)
}

fn ensure_default_plots_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<()> {
    for plot_index in 1..=PLOT_COUNT {
        let id = format!("farm-plot-{plot_index}");
        tx.execute(
            "INSERT OR IGNORE INTO farm_plots (
                id, plot_index, crop_key, status, stage_index, updated_at
             ) VALUES (?1, ?2, '', ?3, 0, ?4)",
            params![id, plot_index, STATUS_EMPTY, now],
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn refresh_plot_progress_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<()> {
    let plots = list_raw_plots_tx(tx)?;
    for plot in plots {
        if plot.status != STATUS_PLANTED {
            continue;
        }
        let Some(next_care_at) = plot.next_care_at.as_deref() else {
            continue;
        };
        if compare_iso(next_care_at, now)? > 0 {
            continue;
        }
        if plot.mature_at.is_some() {
            tx.execute(
                "UPDATE farm_plots
                 SET status = ?2, updated_at = ?3
                 WHERE id = ?1",
                params![plot.id, STATUS_MATURE, now],
            )
            .map_err(|err| err.to_string())?;
        } else {
            tx.execute(
                "UPDATE farm_plots
                 SET status = ?2, updated_at = ?3
                 WHERE id = ?1",
                params![plot.id, STATUS_NEEDS_WATER, now],
            )
            .map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

fn list_plots_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<Vec<FarmPlot>> {
    list_raw_plots_tx(tx)?
        .into_iter()
        .map(|plot| enrich_plot(plot, now))
        .collect()
}

fn list_raw_plots_tx(tx: &Transaction<'_>) -> LocalResult<Vec<FarmPlot>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, plot_index, crop_key, status, stage_index, planted_at,
                    last_watered_at, next_care_at, mature_at, updated_at
             FROM farm_plots
             ORDER BY plot_index ASC",
        )
        .map_err(|err| err.to_string())?;
    let plots = stmt
        .query_map([], map_plot)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(plots)
}

fn load_plot_tx(tx: &Transaction<'_>, plot_id: &str) -> LocalResult<FarmPlot> {
    tx.query_row(
        "SELECT id, plot_index, crop_key, status, stage_index, planted_at,
                last_watered_at, next_care_at, mature_at, updated_at
         FROM farm_plots
         WHERE id = ?1",
        params![plot_id],
        map_plot,
    )
    .optional()
    .map_err(|err| err.to_string())?
    .ok_or_else(|| "地块不存在。".into())
}

fn clear_plot_tx(tx: &Transaction<'_>, plot_id: &str, now: &str) -> LocalResult<()> {
    tx.execute(
        "UPDATE farm_plots
         SET crop_key = '',
             status = ?2,
             stage_index = 0,
             planted_at = NULL,
             last_watered_at = NULL,
             next_care_at = NULL,
             mature_at = NULL,
             updated_at = ?3
         WHERE id = ?1",
        params![plot_id, STATUS_EMPTY, now],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn insert_harvest_ledger_tx(
    tx: &Transaction<'_>,
    entry: &FarmHarvestLedgerEntry,
) -> LocalResult<()> {
    let rewards_json = serde_json::to_string(&entry.rewards).map_err(|err| err.to_string())?;
    tx.execute(
        "INSERT INTO farm_harvest_ledger (
            id, plot_id, crop_key, rewards_json, lp_reward, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            entry.id,
            entry.plot_id,
            entry.crop_key,
            rewards_json,
            entry.lp_reward,
            entry.created_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn load_harvest_entry_tx(
    tx: &Transaction<'_>,
    entry_id: &str,
) -> LocalResult<Option<FarmHarvestLedgerEntry>> {
    tx.query_row(
        "SELECT id, plot_id, crop_key, rewards_json, lp_reward, created_at
         FROM farm_harvest_ledger
         WHERE id = ?1",
        params![entry_id],
        map_harvest_entry,
    )
    .optional()
    .map_err(|err| err.to_string())
}

fn enrich_plot(mut plot: FarmPlot, now: &str) -> LocalResult<FarmPlot> {
    let crop = find_crop_config(&plot.crop_key);
    plot.crop = crop.clone();
    if plot.status == STATUS_EMPTY || crop.is_none() {
        plot.progress_ratio = 0.0;
        plot.remaining_seconds = 0;
        return Ok(plot);
    }
    let planted_at = plot
        .planted_at
        .as_deref()
        .map(parse_iso)
        .transpose()?
        .unwrap_or_else(Utc::now);
    let now_dt = parse_iso(now)?;
    let elapsed = (now_dt - planted_at).num_seconds().max(0);
    let duration = crop.map(|value| value.duration_seconds).unwrap_or(1).max(1);
    plot.progress_ratio = ((elapsed as f64) / (duration as f64)).clamp(0.0, 1.0);
    plot.remaining_seconds = plot
        .next_care_at
        .as_deref()
        .map(|value| seconds_until(value, now).unwrap_or(0))
        .unwrap_or(0);
    Ok(plot)
}

fn next_stage_at(now: &str, crop: &FarmCropConfig) -> LocalResult<String> {
    let now_dt = parse_iso(now)?;
    let checkpoints = crop.water_required + 1;
    let step_seconds = (crop.duration_seconds / checkpoints.max(1)).max(60);
    Ok((now_dt + Duration::seconds(step_seconds)).to_rfc3339())
}

fn reward_items_for_crop(crop: &FarmCropConfig, event_key: &str) -> Vec<FarmRewardItem> {
    let mut rewards = vec![FarmRewardItem {
        item_key: crop.primary_reward_item_key.clone(),
        quantity: crop.primary_reward_quantity.max(1),
        reward_type: "primary".into(),
    }];
    if let Some(bonus_key) = &crop.bonus_reward_item_key {
        let roll = deterministic_percent(&format!("{event_key}:bonus"));
        if roll < crop.bonus_chance_percent {
            rewards.push(FarmRewardItem {
                item_key: bonus_key.clone(),
                quantity: 1,
                reward_type: "bonus".into(),
            });
        }
    }
    rewards
}

fn lp_reward_for_crop(crop: &FarmCropConfig, event_key: &str) -> i64 {
    let min = crop.lp_min.min(crop.lp_max);
    let max = crop.lp_min.max(crop.lp_max);
    if max <= 0 {
        return 0;
    }
    let span = max - min + 1;
    min + deterministic_number(&format!("{event_key}:lp"), span)
}

fn harvest_event_key(plot: &FarmPlot, crop: &FarmCropConfig) -> LocalResult<String> {
    let planted_at = plot
        .planted_at
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "这轮种植缺少开始时间，不能领取奖励。".to_string())?;
    Ok(format!("{}:{}:{planted_at}", plot.id, crop.crop_key))
}

fn deterministic_percent(seed: &str) -> i64 {
    deterministic_number(seed, 100)
}

fn deterministic_number(seed: &str, modulo: i64) -> i64 {
    if modulo <= 1 {
        return 0;
    }
    let mut hash = 0_i64;
    for byte in seed.as_bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(*byte as i64);
    }
    hash.abs() % modulo
}

fn find_crop_config(crop_key: &str) -> Option<FarmCropConfig> {
    CROP_SEEDS
        .iter()
        .find(|seed| seed.crop_key == crop_key)
        .map(crop_config_from_seed)
}

fn crop_config_from_seed(seed: &FarmCropSeed) -> FarmCropConfig {
    FarmCropConfig {
        crop_key: seed.crop_key.into(),
        seed_item_key: seed.seed_item_key.into(),
        name_zh: seed.name_zh.into(),
        name_en: seed.name_en.into(),
        description_zh: seed.description_zh.into(),
        description_en: seed.description_en.into(),
        duration_seconds: seed.duration_seconds,
        water_required: seed.water_required,
        primary_reward_item_key: seed.primary_reward_item_key.into(),
        primary_reward_quantity: seed.primary_reward_quantity,
        bonus_reward_item_key: seed.bonus_reward_item_key.map(str::to_string),
        bonus_chance_percent: seed.bonus_chance_percent,
        lp_min: seed.lp_min,
        lp_max: seed.lp_max,
    }
}

fn seed_name_zh(crop: &FarmCropConfig) -> String {
    format!("{}种子", crop.name_zh)
}

fn map_status_from_plots(plots: &[FarmPlot]) -> String {
    if plots.iter().any(|plot| plot.status == STATUS_MATURE) {
        "claimable"
    } else if plots.iter().any(|plot| plot.status == STATUS_NEEDS_WATER) {
        "needsCare"
    } else if plots.iter().any(|plot| plot.status == STATUS_PLANTED) {
        "running"
    } else {
        "idle"
    }
    .into()
}

fn farm_summary(state: &FarmState) -> WorkMapSummary {
    WorkMapSummary {
        status: state.map_status.clone(),
        active_plots: state
            .plots
            .iter()
            .filter(|plot| plot.status != STATUS_EMPTY)
            .count() as i64,
        needs_care_plots: state
            .plots
            .iter()
            .filter(|plot| plot.status == STATUS_NEEDS_WATER)
            .count() as i64,
        mature_plots: state
            .plots
            .iter()
            .filter(|plot| plot.status == STATUS_MATURE)
            .count() as i64,
    }
}

fn seconds_until(target: &str, now: &str) -> LocalResult<i64> {
    Ok((parse_iso(target)? - parse_iso(now)?).num_seconds().max(0))
}

fn compare_iso(left: &str, right: &str) -> LocalResult<i64> {
    Ok((parse_iso(left)? - parse_iso(right)?).num_seconds())
}

fn parse_iso(value: &str) -> LocalResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|err| format!("时间格式无效: {err}"))
}

fn map_plot(row: &rusqlite::Row<'_>) -> rusqlite::Result<FarmPlot> {
    Ok(FarmPlot {
        id: row.get(0)?,
        plot_index: row.get(1)?,
        crop_key: row.get(2)?,
        status: row.get(3)?,
        stage_index: row.get(4)?,
        planted_at: row.get(5)?,
        last_watered_at: row.get(6)?,
        next_care_at: row.get(7)?,
        mature_at: row.get(8)?,
        updated_at: row.get(9)?,
        crop: None,
        progress_ratio: 0.0,
        remaining_seconds: 0,
    })
}

fn map_harvest_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<FarmHarvestLedgerEntry> {
    let rewards_json: String = row.get(3)?;
    let rewards = serde_json::from_str::<Vec<FarmRewardItem>>(&rewards_json).unwrap_or_default();
    Ok(FarmHarvestLedgerEntry {
        id: row.get(0)?,
        plot_id: row.get(1)?,
        crop_key: row.get(2)?,
        rewards,
        lp_reward: row.get(4)?,
        created_at: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    const TEST_NOW: &str = "2026-07-15T12:00:00+00:00";

    fn prepare_mature_wheat(conn: &mut Connection, planted_at: &str) {
        let tx = conn.transaction().expect("transaction");
        state_tx(&tx, TEST_NOW).expect("farm defaults");
        pet::ensure_default_exists_tx(&tx).expect("default pet");
        pet_store::ensure_store_defaults_tx(&tx, TEST_NOW).expect("store defaults");
        tx.execute(
            "UPDATE farm_plots
             SET crop_key = 'wheat', status = ?2, stage_index = 1,
                 planted_at = ?3, next_care_at = NULL, mature_at = ?4, updated_at = ?4
             WHERE id = ?1",
            params!["farm-plot-1", STATUS_MATURE, planted_at, TEST_NOW],
        )
        .expect("mature plot");
        tx.commit().expect("commit mature plot");
    }

    #[test]
    fn next_stage_uses_one_checkpoint_interval_from_current_action() {
        let wheat = find_crop_config("wheat").expect("wheat crop config");

        let planted_at = "2026-06-08T00:00:00+00:00";
        let first_care_at = next_stage_at(planted_at, &wheat).expect("first care time");
        assert_eq!(first_care_at, "2026-06-08T00:02:30+00:00");

        let watered_at = "2026-06-08T00:02:30+00:00";
        let mature_at = next_stage_at(watered_at, &wheat).expect("mature time");
        assert_eq!(mature_at, "2026-06-08T00:05:00+00:00");
    }

    #[test]
    fn next_stage_keeps_duration_even_for_multi_water_crops() {
        let carrot = find_crop_config("carrot").expect("carrot crop config");

        let planted_at = "2026-06-08T00:00:00+00:00";
        let first_care_at = next_stage_at(planted_at, &carrot).expect("first care time");
        assert_eq!(first_care_at, "2026-06-08T00:05:00+00:00");

        let second_care_at = next_stage_at(&first_care_at, &carrot).expect("second care time");
        assert_eq!(second_care_at, "2026-06-08T00:10:00+00:00");

        let mature_at = next_stage_at(&second_care_at, &carrot).expect("mature time");
        assert_eq!(mature_at, "2026-06-08T00:15:00+00:00");
    }

    #[test]
    fn harvest_event_key_prevents_a_b_a_duplicate_rewards() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
        crate::local_db::schema::apply_test_schema(&conn).expect("schema");

        let mut first_harvest = None;
        for planted_at in [
            "2026-07-15T10:00:00+00:00",
            "2026-07-15T11:00:00+00:00",
            "2026-07-15T10:00:00+00:00",
        ] {
            prepare_mature_wheat(&mut conn, planted_at);
            let tx = conn.transaction().expect("harvest transaction");
            let (_, harvest) =
                harvest_plot_tx(&tx, "farm-plot-1", TEST_NOW).expect("harvest or replay");
            tx.commit().expect("commit harvest");
            if first_harvest.is_none() {
                first_harvest = Some(harvest.id.clone());
            } else if planted_at == "2026-07-15T10:00:00+00:00" {
                assert_eq!(Some(harvest.id), first_harvest);
            }
        }

        let harvest_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM farm_harvest_ledger", [], |row| {
                row.get(0)
            })
            .expect("harvest count");
        let wheat_quantity: i64 = conn
            .query_row(
                "SELECT quantity FROM pet_inventory WHERE item_key = 'wheat-harvest-food'",
                [],
                |row| row.get(0),
            )
            .expect("wheat inventory");
        let recorded_lp: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(lp_reward), 0) FROM farm_harvest_ledger",
                [],
                |row| row.get(0),
            )
            .expect("recorded LP");
        let wallet_balance: i64 = conn
            .query_row(
                "SELECT balance FROM pet_wallets WHERE pet_id = 'default-pet' AND currency_key = 'lp'",
                [],
                |row| row.get(0),
            )
            .expect("wallet balance");
        assert_eq!(harvest_count, 2);
        assert_eq!(wheat_quantity, 2);
        assert_eq!(wallet_balance, recorded_lp);
    }

    #[test]
    fn failed_harvest_ledger_rolls_back_rewards_and_plot_state() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
        crate::local_db::schema::apply_test_schema(&conn).expect("schema");
        prepare_mature_wheat(&mut conn, "2026-07-15T10:00:00+00:00");
        conn.execute_batch(
            "CREATE TRIGGER fail_farm_harvest_ledger
             BEFORE INSERT ON farm_harvest_ledger
             BEGIN SELECT RAISE(ABORT, 'forced harvest failure'); END;",
        )
        .expect("failure trigger");

        let tx = conn.transaction().expect("harvest transaction");
        let error = harvest_plot_tx(&tx, "farm-plot-1", TEST_NOW).expect_err("forced failure");
        assert!(error.contains("forced harvest failure"));
        tx.rollback().expect("rollback");

        let plot_status: String = conn
            .query_row(
                "SELECT status FROM farm_plots WHERE id = 'farm-plot-1'",
                [],
                |row| row.get(0),
            )
            .expect("plot status");
        let reward_items: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pet_inventory WHERE item_key = 'wheat-harvest-food'",
                [],
                |row| row.get(0),
            )
            .expect("reward items");
        let wallet_balance: i64 = conn
            .query_row(
                "SELECT balance FROM pet_wallets WHERE pet_id = 'default-pet' AND currency_key = 'lp'",
                [],
                |row| row.get(0),
            )
            .expect("wallet balance");
        assert_eq!(plot_status, STATUS_MATURE);
        assert_eq!(reward_items, 0);
        assert_eq!(wallet_balance, 0);
    }
}
