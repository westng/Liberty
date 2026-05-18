use crate::{
    infrastructure::{ids, repositories::pet},
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
    let next_experience = (profile.experience + event_value).max(0);
    profile.experience = next_experience;
    profile.level = pet_level_from_experience(next_experience);
    profile.stage = pet_stage_from_level(profile.level).to_string();
    profile.current_mood = mood.to_string();
    profile.updated_at = now.clone();
    pet::save_profile_tx(&tx, &profile)?;
    pet::ensure_stage_cosmetic_unlocks_tx(&tx, &profile, &previous_stage, &now)?;
    pet::insert_event_ledger_tx(
        &tx,
        &PetEventLedgerEntry {
            id: ids::timestamped_id("pet-event"),
            pet_id: profile.id.clone(),
            event_type: event_type.to_string(),
            event_source: event_source.to_string(),
            event_value,
            event_time: now,
            metadata: metadata.map(|value| value.to_string()),
        },
    )?;
    tx.commit().map_err(|err| err.to_string())?;
    Ok(profile)
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
