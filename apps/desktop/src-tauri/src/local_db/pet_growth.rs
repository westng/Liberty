use crate::{
    infrastructure::{
        ids,
        repositories::{pet, pet_store},
    },
    local_db::{LocalResult, MAX_DAILY_INTERACTION_PER_SOURCE},
};
use rusqlite::Connection;

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

    let previous_stage = profile.stage.clone();
    let now = chrono::Utc::now().to_rfc3339();
    pet_store::ensure_store_defaults_tx(&tx, &now)?;
    let reward_source_key = reward_source_key(event_type, event_source, metadata, &now);
    let reward_already_granted =
        pet_store::reward_exists_tx(&tx, "workflow_reward", &reward_source_key)?;
    let reward = reward_rule(event_type, event_source);
    let effective_event_value = if reward_already_granted && event_type != "interaction" {
        0
    } else {
        event_value
    };
    let next_experience = (profile.experience + effective_event_value).max(0);
    profile.experience = next_experience;
    profile.level = pet_level_from_experience(next_experience);
    profile.stage = pet_stage_from_level(profile.level).to_string();
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
        reward.lp,
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
            metadata: metadata.map(|value| value.to_string()),
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

fn pet_level_from_experience(experience: i64) -> i64 {
    (experience.div_euclid(20) + 1).max(1)
}

fn pet_stage_from_level(level: i64) -> &'static str {
    if level >= 8 {
        "mature"
    } else if level >= 4 {
        "growing"
    } else {
        "baby"
    }
}
