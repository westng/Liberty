use crate::{
    infrastructure::{
        ids,
        repositories::{pet, pet_store},
    },
    local_db::{pet_leveling, LocalResult, MAX_DAILY_INTERACTION_PER_SOURCE},
};
use rusqlite::Connection;
use serde_json::json;

use super::model::{PetEventLedgerEntry, PetProfile};

pub(crate) fn apply_pet_growth_event(
    conn: &mut Connection,
    event_type: &str,
    event_source: &str,
    event_value: i64,
    mood: &str,
    metadata: Option<&str>,
) -> LocalResult<PetProfile> {
    let tx = conn.transaction().map_err(|err| err.to_string())?;
    pet::ensure_default_exists_tx(&tx)?;
    pet_store::ensure_store_defaults_tx(&tx, &chrono::Utc::now().to_rfc3339())?;
    let mut profile = pet::load_profile_tx(&tx)?;

    if event_type == "interaction" {
        let todays_count = pet::count_events_for_today_tx(&tx, event_type, event_source)?;
        if todays_count >= MAX_DAILY_INTERACTION_PER_SOURCE {
            return Err(format!(
                "今天的{}互动已经达到上限（{}次）。",
                pet_interaction_label(event_source),
                MAX_DAILY_INTERACTION_PER_SOURCE
            ));
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    profile = migrate_profile_level_floor(profile);
    let previous_stage = profile.stage.clone();
    let reward_source_key = reward_source_key(event_type, event_source, metadata, &now);
    let reward_already_granted =
        pet_store::reward_exists_tx(&tx, "workflow_reward", &reward_source_key)?;
    let reward = reward_rule(event_type, event_source);
    let base_growth = if reward_already_granted && event_type != "interaction" {
        0
    } else {
        event_value
    };
    let growth_multiplier = if event_type == "interaction" {
        1.0
    } else {
        pet_leveling::growth_reward_multiplier(profile.level)
    };
    let lp_multiplier = if event_type == "interaction" {
        1.0
    } else {
        pet_leveling::lp_reward_multiplier(profile.level)
    };
    let effective_event_value = scaled_amount(base_growth, growth_multiplier);
    let effective_lp = scaled_amount(reward.lp, lp_multiplier);
    let next_experience = (profile.experience + effective_event_value).max(0);
    let level_snapshot = pet_leveling::level_snapshot_from_experience(next_experience);
    profile.experience = next_experience;
    profile.level = level_snapshot.level;
    profile.stage = level_snapshot.current_stage.clone();
    profile.level_snapshot = level_snapshot;
    profile.current_mood = mood.to_string();
    profile.updated_at = now.clone();
    pet::save_profile_tx(&tx, &profile)?;
    pet::ensure_stage_cosmetic_unlocks_tx(&tx, &profile, &previous_stage, &now)?;
    if let Some(counter_key) = counter_key_for_event(event_source) {
        let _ = pet_store::increment_counter_tx(&tx, counter_key, &reward_source_key, &now)?;
    }
    pet_store::grant_reward_tx(
        &tx,
        "workflow_reward",
        &reward_source_key,
        effective_lp,
        metadata,
        &now,
    )?;
    pet_store::auto_unlock_eligible_items_tx(&tx, &profile, &now)?;
    pet::insert_event_ledger_tx(
        &tx,
        &PetEventLedgerEntry {
            id: ids::timestamped_id("pet-event"),
            pet_id: profile.id.clone(),
            event_type: event_type.to_string(),
            event_source: event_source.to_string(),
            event_value: effective_event_value,
            event_time: now,
            metadata: Some(reward_metadata(
                metadata,
                base_growth,
                growth_multiplier,
                effective_event_value,
                reward.lp,
                lp_multiplier,
                effective_lp,
            )),
        },
    )?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(profile)
}

struct PetRewardRule {
    lp: i64,
}

fn reward_rule(event_type: &str, event_source: &str) -> PetRewardRule {
    let lp = if event_type == "interaction" {
        1
    } else {
        match event_source {
            "daily_open" => 5,
            "job_created" => 8,
            "transcription_started" => 3,
            "transcription_completed" => 18,
            "ai_summary_completed" => 15,
            "export_completed" => 10,
            "dark_theme_used" => 0,
            _ => 0,
        }
    };
    PetRewardRule { lp }
}

fn reward_source_key(
    event_type: &str,
    event_source: &str,
    metadata: Option<&str>,
    now: &str,
) -> String {
    if event_source == "daily_open" {
        let date = now.split('T').next().unwrap_or(now);
        return format!("{event_type}:{event_source}:{date}");
    }
    if event_type == "interaction" {
        return format!(
            "{event_type}:{event_source}:{}",
            ids::timestamped_id("interaction")
        );
    }
    let source = metadata
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("global");
    format!("{event_type}:{event_source}:{source}")
}

fn counter_key_for_event(event_source: &str) -> Option<&'static str> {
    match event_source {
        "daily_open" => Some("active_days"),
        "job_created" => Some("tasks_created"),
        "transcription_completed" => Some("transcriptions_completed"),
        "ai_summary_completed" => Some("summaries_completed"),
        "export_completed" => Some("exports_completed"),
        "dark_theme_used" => Some("dark_theme_days"),
        _ => None,
    }
}

fn pet_interaction_label(event_source: &str) -> &'static str {
    match event_source {
        "tap" => "点击",
        "pet" => "抚摸",
        "feed" => "投喂",
        "encourage" => "鼓励",
        _ => "互动",
    }
}

fn scaled_amount(base: i64, multiplier: f64) -> i64 {
    if base <= 0 {
        0
    } else {
        ((base as f64) * multiplier).round().max(0.0) as i64
    }
}

fn migrate_profile_level_floor(mut profile: PetProfile) -> PetProfile {
    let snapshot = pet_leveling::level_snapshot_from_experience(profile.experience);
    let floor_level = profile
        .level
        .max(snapshot.level)
        .clamp(1, pet_leveling::MAX_PET_LEVEL);
    let effective_experience = if floor_level > snapshot.level {
        pet_leveling::total_required_exp_for_level(floor_level)
    } else {
        profile.experience
    };
    let next_snapshot = pet_leveling::level_snapshot_from_experience(effective_experience);
    profile.experience = effective_experience;
    profile.level = next_snapshot.level;
    profile.stage = next_snapshot.current_stage.clone();
    profile.level_snapshot = next_snapshot;
    profile
}

fn reward_metadata(
    original_metadata: Option<&str>,
    base_growth: i64,
    growth_multiplier: f64,
    final_growth: i64,
    base_lp: i64,
    lp_multiplier: f64,
    final_lp: i64,
) -> String {
    json!({
        "source": original_metadata.unwrap_or(""),
        "baseGrowth": base_growth,
        "growthMultiplier": growth_multiplier,
        "finalGrowth": final_growth,
        "baseLp": base_lp,
        "lpMultiplier": lp_multiplier,
        "finalLp": final_lp,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use crate::infrastructure::repositories::{pet, pet_store};
    use rusqlite::Connection;

    use super::*;

    #[test]
    fn food_style_zero_base_does_not_scale_to_positive() {
        assert_eq!(scaled_amount(0, 2.0), 0);
    }

    #[test]
    fn rounds_scaled_rewards() {
        assert_eq!(scaled_amount(12, 1.6), 19);
        assert_eq!(scaled_amount(18, 1.3), 23);
    }

    #[test]
    fn first_created_job_unlocks_first_task_badge() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
        crate::local_db::schema::apply_test_schema(&conn).expect("schema");

        let profile = apply_pet_growth_event(
            &mut conn,
            "workflow",
            "job_created",
            5,
            "cheerful",
            Some("job-1"),
        )
        .expect("job-created growth event");
        let state = pet_store::store_state(&conn, profile).expect("store state");

        assert!(state
            .inventory
            .iter()
            .any(|item| item.item_key == "baby-bottle-badge" && item.source == "achievement"));
        assert_eq!(
            state
                .counters
                .iter()
                .find(|counter| counter.counter_key == "tasks_created")
                .map(|counter| counter.counter_value),
            Some(1)
        );
        assert!(pet::list_event_ledger(&conn, 10)
            .expect("event ledger")
            .iter()
            .any(|entry| entry.event_source == "job_created"));
    }

    #[test]
    fn dark_theme_usage_counts_once_per_day_without_lp_reward() {
        let mut conn = Connection::open_in_memory().expect("in-memory sqlite");
        crate::local_db::schema::apply_test_schema(&conn).expect("schema");

        for _ in 0..2 {
            apply_pet_growth_event(
                &mut conn,
                "workflow",
                "dark_theme_used",
                0,
                "idle",
                Some("2026-05-22"),
            )
            .expect("dark-theme usage event");
        }

        let profile = pet::load_profile(&conn).expect("profile");
        let state = pet_store::store_state(&conn, profile).expect("store state");

        assert_eq!(
            state
                .counters
                .iter()
                .find(|counter| counter.counter_key == "dark_theme_days")
                .map(|counter| counter.counter_value),
            Some(1)
        );
        assert_eq!(state.wallet.lifetime_earned, 0);
    }
}
