use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, OptionalExtension, Transaction};

use crate::{
    infrastructure::repositories::{pet, pet_store},
    local_db::{
        LocalResult, WorkGameClaimResult, WorkGameJobConfig, WorkGameRewardItem,
        WorkGameRewardLedgerEntry, WorkGameState, WorkGameTask, WorkMap, WorkMapSummary,
    },
};

const STATUS_IDLE: &str = "idle";
const STATUS_RUNNING: &str = "running";
const STATUS_NEEDS_CARE: &str = "needsCare";
const STATUS_CLAIMABLE: &str = "claimable";

struct WorkGameSeed {
    game_key: &'static str,
    name_zh: &'static str,
    name_en: &'static str,
    description_zh: &'static str,
    description_en: &'static str,
    outputs: &'static [&'static str],
    jobs: &'static [WorkGameJobSeed],
}

struct WorkGameJobSeed {
    job_key: &'static str,
    slot_index: i64,
    name_zh: &'static str,
    name_en: &'static str,
    description_zh: &'static str,
    description_en: &'static str,
    duration_seconds: i64,
    care_required: i64,
    primary_reward_item_key: &'static str,
    primary_reward_quantity: i64,
    bonus_reward_item_key: Option<&'static str>,
    bonus_chance_percent: i64,
    lp_min: i64,
    lp_max: i64,
    care_actions_zh: &'static [&'static str],
    care_actions_en: &'static [&'static str],
}

const MINE_JOBS: &[WorkGameJobSeed] = &[
    WorkGameJobSeed {
        job_key: "shallow-vein",
        slot_index: 1,
        name_zh: "浅层矿脉",
        name_en: "Shallow Vein",
        description_zh: "短周期矿点，适合快速收获工具和少量 LP。",
        description_en: "A short mining loop for tools and a little LP.",
        duration_seconds: 10 * 60,
        care_required: 1,
        primary_reward_item_key: "energy-capsule-tool",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("rune-stone-tool"),
        bonus_chance_percent: 12,
        lp_min: 3,
        lp_max: 6,
        care_actions_zh: &["敲击矿点", "清理碎石"],
        care_actions_en: &["Tap ore", "Clear rocks"],
    },
    WorkGameJobSeed {
        job_key: "deep-vein",
        slot_index: 2,
        name_zh: "深层矿脉",
        name_en: "Deep Vein",
        description_zh: "中周期矿点，需要加固支架，收益更稳定。",
        description_en: "A deeper mining loop with steadier rewards.",
        duration_seconds: 25 * 60,
        care_required: 2,
        primary_reward_item_key: "rune-stone-tool",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("rainbow-crystal-tool"),
        bonus_chance_percent: 16,
        lp_min: 8,
        lp_max: 14,
        care_actions_zh: &["清理碎石", "加固支架", "装载矿车"],
        care_actions_en: &["Clear rocks", "Brace beams", "Load cart"],
    },
    WorkGameJobSeed {
        job_key: "glowing-vein",
        slot_index: 3,
        name_zh: "闪光富矿",
        name_en: "Glowing Vein",
        description_zh: "长周期富矿，低概率带出惊喜礼盒。",
        description_en: "A long rich-vein loop with a small gift-box chance.",
        duration_seconds: 45 * 60,
        care_required: 3,
        primary_reward_item_key: "rainbow-crystal-tool",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some(pet_store::GIFT_BOX_ITEM_KEY),
        bonus_chance_percent: 14,
        lp_min: 15,
        lp_max: 24,
        care_actions_zh: &["定位富矿", "加固支架", "装载矿车"],
        care_actions_en: &["Mark vein", "Brace beams", "Load cart"],
    },
];

const FACTORY_JOBS: &[WorkGameJobSeed] = &[
    WorkGameJobSeed {
        job_key: "basic-assembly",
        slot_index: 1,
        name_zh: "基础装配",
        name_en: "Basic Assembly",
        description_zh: "短周期工位，稳定产出 LP 和基础工具。",
        description_en: "A short station with stable LP and tool rewards.",
        duration_seconds: 8 * 60,
        care_required: 1,
        primary_reward_item_key: "energy-capsule-tool",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("star-coin-tool"),
        bonus_chance_percent: 10,
        lp_min: 5,
        lp_max: 9,
        care_actions_zh: &["按序拧螺丝", "打包出货"],
        care_actions_en: &["Tighten screws", "Pack order"],
    },
    WorkGameJobSeed {
        job_key: "rush-order",
        slot_index: 2,
        name_zh: "加急订单",
        name_en: "Rush Order",
        description_zh: "中周期订单，照看传送带后获得更高 LP。",
        description_en: "A mid-cycle order with higher LP after line care.",
        duration_seconds: 18 * 60,
        care_required: 2,
        primary_reward_item_key: "energy-drink-tool",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("stopwatch-tool"),
        bonus_chance_percent: 18,
        lp_min: 10,
        lp_max: 16,
        care_actions_zh: &["清理卡料", "质检盖章", "打包出货"],
        care_actions_en: &["Clear jam", "Inspect stamp", "Pack order"],
    },
    WorkGameJobSeed {
        job_key: "precision-check",
        slot_index: 3,
        name_zh: "精密质检",
        name_en: "Precision Check",
        description_zh: "长周期质检岗位，适合收获秒表和稀有工具。",
        description_en: "A longer inspection station for stopwatch and rare tools.",
        duration_seconds: 35 * 60,
        care_required: 3,
        primary_reward_item_key: "stopwatch-tool",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("golden-bell-tool"),
        bonus_chance_percent: 14,
        lp_min: 16,
        lp_max: 25,
        care_actions_zh: &["按序拧螺丝", "质检盖章", "整理工位"],
        care_actions_en: &["Tighten screws", "Inspect stamp", "Reset station"],
    },
];

const STORE_JOBS: &[WorkGameJobSeed] = &[
    WorkGameJobSeed {
        job_key: "day-shift",
        slot_index: 1,
        name_zh: "白班",
        name_en: "Day Shift",
        description_zh: "短周期值班，主要获得食物和少量 LP。",
        description_en: "A short store shift for food and a little LP.",
        duration_seconds: 12 * 60,
        care_required: 1,
        primary_reward_item_key: "sandwich-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("bubble-tea-food"),
        bonus_chance_percent: 18,
        lp_min: 4,
        lp_max: 8,
        care_actions_zh: &["收银结账", "补齐货架"],
        care_actions_en: &["Checkout", "Restock shelf"],
    },
    WorkGameJobSeed {
        job_key: "evening-shift",
        slot_index: 2,
        name_zh: "晚班",
        name_en: "Evening Shift",
        description_zh: "中周期值班，兼顾补货、加热便当和顾客需求。",
        description_en: "A mid-cycle shift with restocking and customer care.",
        duration_seconds: 24 * 60,
        care_required: 2,
        primary_reward_item_key: "bento-box-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some(pet_store::GIFT_BOX_ITEM_KEY),
        bonus_chance_percent: 10,
        lp_min: 9,
        lp_max: 15,
        care_actions_zh: &["加热便当", "补齐货架", "收银结账"],
        care_actions_en: &["Heat meal", "Restock shelf", "Checkout"],
    },
    WorkGameJobSeed {
        job_key: "night-shift",
        slot_index: 3,
        name_zh: "夜班",
        name_en: "Night Shift",
        description_zh: "长周期夜班，低压力但有更好的礼盒和道具概率。",
        description_en: "A longer night shift with better gift and tool chances.",
        duration_seconds: 40 * 60,
        care_required: 3,
        primary_reward_item_key: "fruit-tart-food",
        primary_reward_quantity: 1,
        bonus_reward_item_key: Some("magic-potion-tool"),
        bonus_chance_percent: 16,
        lp_min: 14,
        lp_max: 22,
        care_actions_zh: &["清洁门口", "处理顾客需求", "整理货架"],
        care_actions_en: &["Clean entrance", "Help customer", "Tidy shelf"],
    },
];

const WORK_GAMES: &[WorkGameSeed] = &[
    WorkGameSeed {
        game_key: "mine",
        name_zh: "矿场挖矿",
        name_en: "Mine",
        description_zh: "选择矿脉、照看矿点，收获工具、稀有道具和 LP。",
        description_en: "Pick a vein, care for the mine, and earn tools, rare items, and LP.",
        outputs: &["tool", "rare_tool", "lp", "gift_box"],
        jobs: MINE_JOBS,
    },
    WorkGameSeed {
        game_key: "factory",
        name_zh: "工厂打螺丝",
        name_en: "Factory",
        description_zh: "接装配订单、处理生产线，稳定获得 LP 和工具。",
        description_en: "Run assembly orders for steady LP and tool rewards.",
        outputs: &["lp", "tool", "energy", "badge"],
        jobs: FACTORY_JOBS,
    },
    WorkGameSeed {
        game_key: "convenience-store",
        name_zh: "便利店值班",
        name_en: "Convenience Store",
        description_zh: "完成收银、补货和清洁，获得食物、礼盒和日常 LP。",
        description_en: "Handle checkout, restocking, and cleaning for food, gifts, and LP.",
        outputs: &["food", "gift_box", "lp", "tool"],
        jobs: STORE_JOBS,
    },
];

pub fn work_maps_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<Vec<WorkMap>> {
    ensure_default_tasks_tx(tx, now)?;
    refresh_all_progress_tx(tx, now)?;
    WORK_GAMES
        .iter()
        .map(|game| {
            let tasks = list_tasks_tx(tx, game.game_key, now)?;
            let summary = summary_from_tasks(&tasks);
            Ok(WorkMap {
                id: game.game_key.into(),
                name_zh: game.name_zh.into(),
                name_en: game.name_en.into(),
                description_zh: game.description_zh.into(),
                description_en: game.description_en.into(),
                category: game.game_key.into(),
                status: summary.status.clone(),
                route: format!("/work-game/{}", game.game_key),
                outputs: game.outputs.iter().map(|value| (*value).into()).collect(),
                enabled: true,
                summary,
            })
        })
        .collect()
}

pub fn state_tx(tx: &Transaction<'_>, game_key: &str, now: &str) -> LocalResult<WorkGameState> {
    let game = find_game(game_key).ok_or_else(|| "打工地图不存在。".to_string())?;
    ensure_default_tasks_tx(tx, now)?;
    refresh_all_progress_tx(tx, now)?;
    let tasks = list_tasks_tx(tx, game.game_key, now)?;
    let rewards = list_reward_ledger_tx(tx, game.game_key, 12)?;
    Ok(WorkGameState {
        game_key: game.game_key.into(),
        name_zh: game.name_zh.into(),
        name_en: game.name_en.into(),
        description_zh: game.description_zh.into(),
        description_en: game.description_en.into(),
        map_status: map_status_from_tasks(&tasks),
        tasks,
        jobs: game
            .jobs
            .iter()
            .map(|job| job_config_from_seed(game, job))
            .collect(),
        rewards,
        updated_at: now.into(),
    })
}

pub fn start_task_tx(
    tx: &Transaction<'_>,
    game_key: &str,
    task_id: &str,
    job_key: &str,
    now: &str,
) -> LocalResult<WorkGameState> {
    ensure_default_tasks_tx(tx, now)?;
    refresh_all_progress_tx(tx, now)?;
    let task = load_task_tx(tx, task_id)?;
    if task.game_key != game_key {
        return Err("岗位不属于当前地图。".into());
    }
    if task.status != STATUS_IDLE {
        return Err("这个岗位还没有空闲。".into());
    }
    let game = find_game(game_key).ok_or_else(|| "打工地图不存在。".to_string())?;
    let job = find_job_config(game_key, job_key).ok_or_else(|| "岗位配置不存在。".to_string())?;
    if job.slot_index != task.slot_index {
        return Err("岗位配置和当前热区不匹配。".into());
    }
    let next_care_at = next_stage_at(now, &job)?;
    tx.execute(
        "UPDATE work_game_tasks
         SET job_key = ?2,
             status = ?3,
             stage_index = 0,
             started_at = ?4,
             last_cared_at = NULL,
             next_care_at = ?5,
             claimable_at = NULL,
             updated_at = ?4
         WHERE id = ?1",
        params![task.id, job.job_key, STATUS_RUNNING, now, next_care_at],
    )
    .map_err(|err| err.to_string())?;
    state_tx(tx, game.game_key, now)
}

pub fn care_task_tx(
    tx: &Transaction<'_>,
    game_key: &str,
    task_id: &str,
    now: &str,
) -> LocalResult<WorkGameState> {
    ensure_default_tasks_tx(tx, now)?;
    refresh_all_progress_tx(tx, now)?;
    let task = load_task_tx(tx, task_id)?;
    if task.game_key != game_key {
        return Err("岗位不属于当前地图。".into());
    }
    if task.status != STATUS_NEEDS_CARE {
        return Err("这个岗位暂时不需要照看。".into());
    }
    let job =
        find_job_config(game_key, &task.job_key).ok_or_else(|| "岗位配置不存在。".to_string())?;
    let next_stage_index = task.stage_index + 1;
    if next_stage_index >= job.care_required {
        let claimable_at = next_stage_at(now, &job)?;
        tx.execute(
            "UPDATE work_game_tasks
             SET status = ?2,
                 stage_index = ?3,
                 last_cared_at = ?4,
                 next_care_at = NULL,
                 claimable_at = ?5,
                 updated_at = ?4
             WHERE id = ?1",
            params![task.id, STATUS_RUNNING, next_stage_index, now, claimable_at],
        )
        .map_err(|err| err.to_string())?;
    } else {
        let next_care_at = next_stage_at(now, &job)?;
        tx.execute(
            "UPDATE work_game_tasks
             SET status = ?2,
                 stage_index = ?3,
                 last_cared_at = ?4,
                 next_care_at = ?5,
                 claimable_at = NULL,
                 updated_at = ?4
             WHERE id = ?1",
            params![task.id, STATUS_RUNNING, next_stage_index, now, next_care_at],
        )
        .map_err(|err| err.to_string())?;
    }
    state_tx(tx, game_key, now)
}

pub fn claim_task_tx(
    tx: &Transaction<'_>,
    game_key: &str,
    task_id: &str,
    now: &str,
) -> LocalResult<WorkGameClaimResult> {
    ensure_default_tasks_tx(tx, now)?;
    refresh_all_progress_tx(tx, now)?;
    pet::ensure_default_exists_tx(tx)?;
    pet_store::ensure_store_defaults_tx(tx, now)?;
    let task = load_task_tx(tx, task_id)?;
    if task.game_key != game_key {
        return Err("岗位不属于当前地图。".into());
    }
    if task.status != STATUS_CLAIMABLE {
        return Err("这个岗位还不能领奖。".into());
    }
    let job =
        find_job_config(game_key, &task.job_key).ok_or_else(|| "岗位配置不存在。".to_string())?;
    let event_key = reward_event_key(&task, &job)?;
    let reward_id = format!("work-game-reward:{event_key}");
    if !pet_store::claim_event_key_tx(tx, "work_game_reward", &event_key, Some(game_key), now)? {
        let reward = load_reward_entry_tx(tx, &reward_id)?
            .ok_or_else(|| "这轮工作已经领奖，无法恢复原奖励记录。".to_string())?;
        clear_task_tx(tx, &task.id, now)?;
        return Ok(WorkGameClaimResult {
            state: state_tx(tx, game_key, now)?,
            reward,
        });
    }
    let rewards = reward_items_for_job(&job, &event_key);
    let lp_reward = lp_reward_for_job(&job, &event_key);
    for reward in &rewards {
        let item = pet_store::find_catalog_item_by_key(&reward.item_key)
            .ok_or_else(|| format!("宠物商品不存在: {}", reward.item_key))?;
        pet_store::grant_catalog_item_tx(tx, &item, reward.quantity, "work_game_reward", now)?;
    }
    if lp_reward > 0 {
        pet_store::grant_reward_tx(
            tx,
            "work_game_reward",
            &event_key,
            lp_reward,
            Some(game_key),
            now,
        )?;
    }
    let reward = WorkGameRewardLedgerEntry {
        id: reward_id,
        game_key: game_key.into(),
        task_id: task.id.clone(),
        job_key: job.job_key.clone(),
        rewards: rewards.clone(),
        lp_reward,
        created_at: now.into(),
    };
    insert_reward_ledger_tx(tx, &reward)?;
    clear_task_tx(tx, &task.id, now)?;
    Ok(WorkGameClaimResult {
        state: state_tx(tx, game_key, now)?,
        reward,
    })
}

pub fn list_reward_ledger_tx(
    tx: &Transaction<'_>,
    game_key: &str,
    limit: usize,
) -> LocalResult<Vec<WorkGameRewardLedgerEntry>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, game_key, task_id, job_key, rewards_json, lp_reward, created_at
             FROM work_game_reward_ledger
             WHERE game_key = ?1
             ORDER BY datetime(created_at) DESC, created_at DESC
             LIMIT ?2",
        )
        .map_err(|err| err.to_string())?;
    let entries = stmt
        .query_map(params![game_key, limit as i64], map_reward_entry)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(entries)
}

fn ensure_default_tasks_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<()> {
    for game in WORK_GAMES {
        for job in game.jobs {
            let id = task_id_for(game.game_key, job.slot_index);
            tx.execute(
                "INSERT OR IGNORE INTO work_game_tasks (
                    id, game_key, slot_index, job_key, status, stage_index, updated_at
                 ) VALUES (?1, ?2, ?3, '', ?4, 0, ?5)",
                params![id, game.game_key, job.slot_index, STATUS_IDLE, now],
            )
            .map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

fn refresh_all_progress_tx(tx: &Transaction<'_>, now: &str) -> LocalResult<()> {
    let tasks = list_raw_tasks_tx(tx)?;
    for task in tasks {
        if task.status != STATUS_RUNNING {
            continue;
        }
        if let Some(claimable_at) = task.claimable_at.as_deref() {
            if compare_iso(claimable_at, now)? <= 0 {
                tx.execute(
                    "UPDATE work_game_tasks
                     SET status = ?2, updated_at = ?3
                     WHERE id = ?1",
                    params![task.id, STATUS_CLAIMABLE, now],
                )
                .map_err(|err| err.to_string())?;
            }
            continue;
        }
        if let Some(next_care_at) = task.next_care_at.as_deref() {
            if compare_iso(next_care_at, now)? <= 0 {
                tx.execute(
                    "UPDATE work_game_tasks
                     SET status = ?2, updated_at = ?3
                     WHERE id = ?1",
                    params![task.id, STATUS_NEEDS_CARE, now],
                )
                .map_err(|err| err.to_string())?;
            }
        }
    }
    Ok(())
}

fn list_tasks_tx(
    tx: &Transaction<'_>,
    game_key: &str,
    now: &str,
) -> LocalResult<Vec<WorkGameTask>> {
    list_raw_tasks_for_game_tx(tx, game_key)?
        .into_iter()
        .map(|task| enrich_task(task, now))
        .collect()
}

fn list_raw_tasks_for_game_tx(
    tx: &Transaction<'_>,
    game_key: &str,
) -> LocalResult<Vec<WorkGameTask>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, game_key, slot_index, job_key, status, stage_index, started_at,
                    last_cared_at, next_care_at, claimable_at, updated_at
             FROM work_game_tasks
             WHERE game_key = ?1
             ORDER BY slot_index ASC",
        )
        .map_err(|err| err.to_string())?;
    let tasks = stmt
        .query_map(params![game_key], map_task)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(tasks)
}

fn list_raw_tasks_tx(tx: &Transaction<'_>) -> LocalResult<Vec<WorkGameTask>> {
    let mut stmt = tx
        .prepare(
            "SELECT id, game_key, slot_index, job_key, status, stage_index, started_at,
                    last_cared_at, next_care_at, claimable_at, updated_at
             FROM work_game_tasks
             ORDER BY game_key ASC, slot_index ASC",
        )
        .map_err(|err| err.to_string())?;
    let tasks = stmt
        .query_map([], map_task)
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(tasks)
}

fn load_task_tx(tx: &Transaction<'_>, task_id: &str) -> LocalResult<WorkGameTask> {
    tx.query_row(
        "SELECT id, game_key, slot_index, job_key, status, stage_index, started_at,
                last_cared_at, next_care_at, claimable_at, updated_at
         FROM work_game_tasks
         WHERE id = ?1",
        params![task_id],
        map_task,
    )
    .optional()
    .map_err(|err| err.to_string())?
    .ok_or_else(|| "岗位不存在。".into())
}

fn clear_task_tx(tx: &Transaction<'_>, task_id: &str, now: &str) -> LocalResult<()> {
    tx.execute(
        "UPDATE work_game_tasks
         SET job_key = '',
             status = ?2,
             stage_index = 0,
             started_at = NULL,
             last_cared_at = NULL,
             next_care_at = NULL,
             claimable_at = NULL,
             updated_at = ?3
         WHERE id = ?1",
        params![task_id, STATUS_IDLE, now],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn insert_reward_ledger_tx(
    tx: &Transaction<'_>,
    entry: &WorkGameRewardLedgerEntry,
) -> LocalResult<()> {
    let rewards_json = serde_json::to_string(&entry.rewards).map_err(|err| err.to_string())?;
    tx.execute(
        "INSERT INTO work_game_reward_ledger (
            id, game_key, task_id, job_key, rewards_json, lp_reward, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.id,
            entry.game_key,
            entry.task_id,
            entry.job_key,
            rewards_json,
            entry.lp_reward,
            entry.created_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn load_reward_entry_tx(
    tx: &Transaction<'_>,
    reward_id: &str,
) -> LocalResult<Option<WorkGameRewardLedgerEntry>> {
    tx.query_row(
        "SELECT id, game_key, task_id, job_key, rewards_json, lp_reward, created_at
         FROM work_game_reward_ledger
         WHERE id = ?1",
        params![reward_id],
        map_reward_entry,
    )
    .optional()
    .map_err(|err| err.to_string())
}

fn enrich_task(mut task: WorkGameTask, now: &str) -> LocalResult<WorkGameTask> {
    let job = if task.job_key.is_empty() {
        default_job_for_slot(&task.game_key, task.slot_index)
    } else {
        find_job_config(&task.game_key, &task.job_key)
    };
    task.job = job.clone();
    if task.status == STATUS_IDLE || job.is_none() {
        task.progress_ratio = 0.0;
        task.remaining_seconds = 0;
        return Ok(task);
    }
    let started_at = task
        .started_at
        .as_deref()
        .map(parse_iso)
        .transpose()?
        .unwrap_or_else(Utc::now);
    let now_dt = parse_iso(now)?;
    let elapsed = (now_dt - started_at).num_seconds().max(0);
    let duration = job.map(|value| value.duration_seconds).unwrap_or(1).max(1);
    task.progress_ratio = ((elapsed as f64) / (duration as f64)).clamp(0.0, 1.0);
    task.remaining_seconds = task
        .claimable_at
        .as_deref()
        .or(task.next_care_at.as_deref())
        .map(|value| seconds_until(value, now).unwrap_or(0))
        .unwrap_or(0);
    Ok(task)
}

fn next_stage_at(now: &str, job: &WorkGameJobConfig) -> LocalResult<String> {
    let now_dt = parse_iso(now)?;
    let checkpoints = job.care_required + 1;
    let step_seconds = (job.duration_seconds / checkpoints.max(1)).max(60);
    Ok((now_dt + Duration::seconds(step_seconds)).to_rfc3339())
}

fn reward_items_for_job(job: &WorkGameJobConfig, event_key: &str) -> Vec<WorkGameRewardItem> {
    let mut rewards = vec![WorkGameRewardItem {
        item_key: job.primary_reward_item_key.clone(),
        quantity: job.primary_reward_quantity.max(1),
        reward_type: "primary".into(),
    }];
    if let Some(bonus_key) = &job.bonus_reward_item_key {
        let roll = deterministic_percent(&format!("{event_key}:bonus"));
        if roll < job.bonus_chance_percent {
            rewards.push(WorkGameRewardItem {
                item_key: bonus_key.clone(),
                quantity: 1,
                reward_type: "bonus".into(),
            });
        }
    }
    rewards
}

fn lp_reward_for_job(job: &WorkGameJobConfig, event_key: &str) -> i64 {
    let min = job.lp_min.min(job.lp_max);
    let max = job.lp_min.max(job.lp_max);
    if max <= 0 {
        return 0;
    }
    let span = max - min + 1;
    min + deterministic_number(&format!("{event_key}:lp"), span)
}

fn reward_event_key(task: &WorkGameTask, job: &WorkGameJobConfig) -> LocalResult<String> {
    let started_at = task
        .started_at
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "这轮工作缺少开始时间，不能领取奖励。".to_string())?;
    Ok(format!("{}:{}:{started_at}", task.id, job.job_key))
}

fn summary_from_tasks(tasks: &[WorkGameTask]) -> WorkMapSummary {
    WorkMapSummary {
        status: map_status_from_tasks(tasks),
        active_plots: tasks
            .iter()
            .filter(|task| task.status != STATUS_IDLE)
            .count() as i64,
        needs_care_plots: tasks
            .iter()
            .filter(|task| task.status == STATUS_NEEDS_CARE)
            .count() as i64,
        mature_plots: tasks
            .iter()
            .filter(|task| task.status == STATUS_CLAIMABLE)
            .count() as i64,
    }
}

fn map_status_from_tasks(tasks: &[WorkGameTask]) -> String {
    if tasks.iter().any(|task| task.status == STATUS_CLAIMABLE) {
        STATUS_CLAIMABLE
    } else if tasks.iter().any(|task| task.status == STATUS_NEEDS_CARE) {
        STATUS_NEEDS_CARE
    } else if tasks.iter().any(|task| task.status == STATUS_RUNNING) {
        STATUS_RUNNING
    } else {
        STATUS_IDLE
    }
    .into()
}

fn find_game(game_key: &str) -> Option<&'static WorkGameSeed> {
    WORK_GAMES.iter().find(|game| game.game_key == game_key)
}

fn find_job_config(game_key: &str, job_key: &str) -> Option<WorkGameJobConfig> {
    let game = find_game(game_key)?;
    game.jobs
        .iter()
        .find(|job| job.job_key == job_key)
        .map(|job| job_config_from_seed(game, job))
}

fn default_job_for_slot(game_key: &str, slot_index: i64) -> Option<WorkGameJobConfig> {
    let game = find_game(game_key)?;
    game.jobs
        .iter()
        .find(|job| job.slot_index == slot_index)
        .map(|job| job_config_from_seed(game, job))
}

fn job_config_from_seed(game: &WorkGameSeed, seed: &WorkGameJobSeed) -> WorkGameJobConfig {
    WorkGameJobConfig {
        game_key: game.game_key.into(),
        job_key: seed.job_key.into(),
        slot_index: seed.slot_index,
        name_zh: seed.name_zh.into(),
        name_en: seed.name_en.into(),
        description_zh: seed.description_zh.into(),
        description_en: seed.description_en.into(),
        duration_seconds: seed.duration_seconds,
        care_required: seed.care_required,
        primary_reward_item_key: seed.primary_reward_item_key.into(),
        primary_reward_quantity: seed.primary_reward_quantity,
        bonus_reward_item_key: seed.bonus_reward_item_key.map(str::to_string),
        bonus_chance_percent: seed.bonus_chance_percent,
        lp_min: seed.lp_min,
        lp_max: seed.lp_max,
        care_actions_zh: seed
            .care_actions_zh
            .iter()
            .map(|value| (*value).into())
            .collect(),
        care_actions_en: seed
            .care_actions_en
            .iter()
            .map(|value| (*value).into())
            .collect(),
    }
}

fn task_id_for(game_key: &str, slot_index: i64) -> String {
    format!("{game_key}-slot-{slot_index}")
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

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkGameTask> {
    Ok(WorkGameTask {
        id: row.get(0)?,
        game_key: row.get(1)?,
        slot_index: row.get(2)?,
        job_key: row.get(3)?,
        status: row.get(4)?,
        stage_index: row.get(5)?,
        started_at: row.get(6)?,
        last_cared_at: row.get(7)?,
        next_care_at: row.get(8)?,
        claimable_at: row.get(9)?,
        updated_at: row.get(10)?,
        job: None,
        progress_ratio: 0.0,
        remaining_seconds: 0,
    })
}

fn map_reward_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkGameRewardLedgerEntry> {
    let rewards_json: String = row.get(4)?;
    let rewards =
        serde_json::from_str::<Vec<WorkGameRewardItem>>(&rewards_json).unwrap_or_default();
    Ok(WorkGameRewardLedgerEntry {
        id: row.get(0)?,
        game_key: row.get(1)?,
        task_id: row.get(2)?,
        job_key: row.get(3)?,
        rewards,
        lp_reward: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;

    const TEST_NOW: &str = "2026-07-15T12:00:00+00:00";

    fn prepare_claimable_factory_task(conn: &mut Connection, started_at: &str) {
        let tx = conn.transaction().expect("transaction");
        state_tx(&tx, "factory", TEST_NOW).expect("work game defaults");
        pet::ensure_default_exists_tx(&tx).expect("default pet");
        pet_store::ensure_store_defaults_tx(&tx, TEST_NOW).expect("store defaults");
        tx.execute(
            "UPDATE work_game_tasks
             SET job_key = 'basic-assembly', status = ?2, stage_index = 1,
                 started_at = ?3, next_care_at = NULL, claimable_at = ?4, updated_at = ?4
             WHERE id = ?1",
            params!["factory-slot-1", STATUS_CLAIMABLE, started_at, TEST_NOW],
        )
        .expect("claimable task");
        tx.commit().expect("commit claimable task");
    }

    #[test]
    fn next_stage_uses_job_checkpoint_interval() {
        let job = find_job_config("factory", "basic-assembly").expect("factory job");
        let started_at = "2026-06-08T00:00:00+00:00";
        let care_at = next_stage_at(started_at, &job).expect("care time");
        assert_eq!(care_at, "2026-06-08T00:04:00+00:00");
    }

    #[test]
    fn work_game_status_prioritizes_claimable() {
        let mut tasks = vec![WorkGameTask {
            status: STATUS_RUNNING.into(),
            ..WorkGameTask::default()
        }];
        assert_eq!(map_status_from_tasks(&tasks), STATUS_RUNNING);
        tasks.push(WorkGameTask {
            status: STATUS_CLAIMABLE.into(),
            ..WorkGameTask::default()
        });
        assert_eq!(map_status_from_tasks(&tasks), STATUS_CLAIMABLE);
    }

    #[test]
    fn work_reward_event_key_prevents_a_b_a_duplicate_rewards() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
        crate::local_db::schema::apply_test_schema(&conn).expect("schema");

        let mut first_reward = None;
        for started_at in [
            "2026-07-15T10:00:00+00:00",
            "2026-07-15T11:00:00+00:00",
            "2026-07-15T10:00:00+00:00",
        ] {
            prepare_claimable_factory_task(&mut conn, started_at);
            let tx = conn.transaction().expect("claim transaction");
            let result =
                claim_task_tx(&tx, "factory", "factory-slot-1", TEST_NOW).expect("claim or replay");
            tx.commit().expect("commit claim");
            if first_reward.is_none() {
                first_reward = Some(result.reward.id.clone());
            } else if started_at == "2026-07-15T10:00:00+00:00" {
                assert_eq!(Some(result.reward.id), first_reward);
            }
        }

        let reward_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM work_game_reward_ledger", [], |row| {
                row.get(0)
            })
            .expect("reward count");
        let item_quantity: i64 = conn
            .query_row(
                "SELECT quantity FROM pet_inventory WHERE item_key = 'energy-capsule-tool'",
                [],
                |row| row.get(0),
            )
            .expect("reward inventory");
        let recorded_lp: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(lp_reward), 0) FROM work_game_reward_ledger",
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
        assert_eq!(reward_count, 2);
        assert_eq!(item_quantity, 2);
        assert_eq!(wallet_balance, recorded_lp);
    }

    #[test]
    fn failed_work_reward_ledger_rolls_back_rewards_and_task_state() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
        crate::local_db::schema::apply_test_schema(&conn).expect("schema");
        prepare_claimable_factory_task(&mut conn, "2026-07-15T10:00:00+00:00");
        conn.execute_batch(
            "CREATE TRIGGER fail_work_reward_ledger
             BEFORE INSERT ON work_game_reward_ledger
             BEGIN SELECT RAISE(ABORT, 'forced work reward failure'); END;",
        )
        .expect("failure trigger");

        let tx = conn.transaction().expect("claim transaction");
        let error =
            claim_task_tx(&tx, "factory", "factory-slot-1", TEST_NOW).expect_err("forced failure");
        assert!(error.contains("forced work reward failure"));
        tx.rollback().expect("rollback");

        let task_status: String = conn
            .query_row(
                "SELECT status FROM work_game_tasks WHERE id = 'factory-slot-1'",
                [],
                |row| row.get(0),
            )
            .expect("task status");
        let reward_items: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pet_inventory WHERE item_key = 'energy-capsule-tool'",
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
        assert_eq!(task_status, STATUS_CLAIMABLE);
        assert_eq!(reward_items, 0);
        assert_eq!(wallet_balance, 0);
    }
}
