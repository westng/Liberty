use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::local_db::{
    pet_leveling, LocalResult, PetCosmeticUnlock, PetEventLedgerEntry, PetProfile, PetSettings,
};

pub fn load_profile(conn: &Connection) -> LocalResult<PetProfile> {
    conn.query_row(
        "SELECT id, name, level, experience, stage, current_mood, created_at, updated_at
         FROM pet_profile
         WHERE id = 'default-pet'",
        [],
        map_pet_profile,
    )
    .map_err(|err| err.to_string())
}

pub fn reconcile_profile_leveling(conn: &Connection) -> LocalResult<()> {
    let (stored_level, stored_experience, stored_stage) = conn
        .query_row(
            "SELECT level, experience, stage
             FROM pet_profile
             WHERE id = 'default-pet'",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|err| err.to_string())?;

    let calculated_snapshot = pet_leveling::level_snapshot_from_experience(stored_experience);
    let effective_level = stored_level
        .max(calculated_snapshot.level)
        .clamp(1, pet_leveling::MAX_PET_LEVEL);
    let effective_experience = if effective_level > calculated_snapshot.level {
        pet_leveling::total_required_exp_for_level(effective_level)
    } else {
        stored_experience
    };
    let level_snapshot = pet_leveling::level_snapshot_from_experience(effective_experience);
    let normalized_stage = pet_leveling::normalize_stage(&stored_stage, level_snapshot.level);

    if stored_level == level_snapshot.level
        && stored_experience == effective_experience
        && stored_stage == normalized_stage
    {
        return Ok(());
    }

    conn.execute(
        "UPDATE pet_profile
         SET level = ?2, experience = ?3, stage = ?4, updated_at = ?5
         WHERE id = ?1",
        params![
            "default-pet",
            level_snapshot.level,
            effective_experience,
            normalized_stage,
            chrono::Utc::now().to_rfc3339()
        ],
    )
    .map_err(|err| err.to_string())?;

    Ok(())
}

pub fn load_profile_tx(tx: &Transaction<'_>) -> LocalResult<PetProfile> {
    tx.query_row(
        "SELECT id, name, level, experience, stage, current_mood, created_at, updated_at
         FROM pet_profile
         WHERE id = 'default-pet'",
        [],
        map_pet_profile,
    )
    .map_err(|err| err.to_string())
}

pub fn save_profile_tx(tx: &Transaction<'_>, profile: &PetProfile) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO pet_profile (
            id, name, level, experience, stage, current_mood, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            level = excluded.level,
            experience = excluded.experience,
            stage = excluded.stage,
            current_mood = excluded.current_mood,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            profile.id,
            profile.name,
            profile.level,
            profile.experience,
            profile.stage,
            profile.current_mood,
            profile.created_at,
            profile.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn load_settings(conn: &Connection) -> LocalResult<PetSettings> {
    conn.query_row(
        "SELECT pet_id, desktop_enabled, always_on_top, muted, focus_mode_enabled,
                proactive_level, last_window_x, last_window_y, updated_at
         FROM pet_settings
         WHERE pet_id = 'default-pet'",
        [],
        |row| {
            Ok(PetSettings {
                pet_id: row.get(0)?,
                desktop_enabled: row.get::<_, i64>(1)? != 0,
                always_on_top: row.get::<_, i64>(2)? != 0,
                muted: row.get::<_, i64>(3)? != 0,
                focus_mode_enabled: row.get::<_, i64>(4)? != 0,
                proactive_level: row.get(5)?,
                last_window_x: row.get(6)?,
                last_window_y: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .map_err(|err| err.to_string())
}

pub fn save_settings(conn: &Connection, settings: &PetSettings) -> LocalResult<()> {
    conn.execute(
        "INSERT INTO pet_settings (
            pet_id, desktop_enabled, always_on_top, muted, focus_mode_enabled,
            proactive_level, last_window_x, last_window_y, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(pet_id) DO UPDATE SET
            desktop_enabled = excluded.desktop_enabled,
            always_on_top = excluded.always_on_top,
            muted = excluded.muted,
            focus_mode_enabled = excluded.focus_mode_enabled,
            proactive_level = excluded.proactive_level,
            last_window_x = excluded.last_window_x,
            last_window_y = excluded.last_window_y,
            updated_at = excluded.updated_at",
        params![
            settings.pet_id,
            if settings.desktop_enabled { 1 } else { 0 },
            if settings.always_on_top { 1 } else { 0 },
            if settings.muted { 1 } else { 0 },
            if settings.focus_mode_enabled { 1 } else { 0 },
            settings.proactive_level,
            settings.last_window_x,
            settings.last_window_y,
            settings.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn list_event_ledger(conn: &Connection, limit: usize) -> LocalResult<Vec<PetEventLedgerEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pet_id, event_type, event_source, event_value, event_time, metadata
             FROM pet_event_ledger
             WHERE pet_id = 'default-pet'
             ORDER BY datetime(event_time) DESC, event_time DESC
             LIMIT ?1",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(PetEventLedgerEntry {
                id: row.get(0)?,
                pet_id: row.get(1)?,
                event_type: row.get(2)?,
                event_source: row.get(3)?,
                event_value: row.get(4)?,
                event_time: row.get(5)?,
                metadata: row.get(6)?,
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

pub fn list_cosmetic_unlocks(conn: &Connection) -> LocalResult<Vec<PetCosmeticUnlock>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, pet_id, cosmetic_type, cosmetic_key, unlocked_at, equipped
             FROM pet_cosmetic_unlocks
             WHERE pet_id = 'default-pet'
             ORDER BY datetime(unlocked_at) DESC, unlocked_at DESC",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PetCosmeticUnlock {
                id: row.get(0)?,
                pet_id: row.get(1)?,
                cosmetic_type: row.get(2)?,
                cosmetic_key: row.get(3)?,
                unlocked_at: row.get(4)?,
                equipped: row.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

pub fn insert_event_ledger_tx(
    tx: &Transaction<'_>,
    entry: &PetEventLedgerEntry,
) -> LocalResult<()> {
    tx.execute(
        "INSERT INTO pet_event_ledger (
            id, pet_id, event_type, event_source, event_value, event_time, metadata
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.id,
            entry.pet_id,
            entry.event_type,
            entry.event_source,
            entry.event_value,
            entry.event_time,
            entry.metadata
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn count_events_for_today_tx(
    tx: &Transaction<'_>,
    event_type: &str,
    event_source: &str,
) -> LocalResult<i64> {
    tx.query_row(
        "SELECT COUNT(1)
         FROM pet_event_ledger
         WHERE event_type = ?1
           AND event_source = ?2
           AND date(event_time, 'localtime') = date('now', 'localtime')",
        params![event_type, event_source],
        |row| row.get(0),
    )
    .map_err(|err| err.to_string())
}

pub fn ensure_stage_cosmetic_unlocks_tx(
    tx: &Transaction<'_>,
    profile: &PetProfile,
    previous_stage: &str,
    unlocked_at: &str,
) -> LocalResult<()> {
    if previous_stage == profile.stage {
        return Ok(());
    }

    let (cosmetic_type, cosmetic_key) = match profile.stage.as_str() {
        "familiar" => ("accessory", "clover-bow"),
        "grow_together" => ("accessory", "strawberry-candy"),
        "deep_bond" => ("accessory", "bell-accessory"),
        _ => return Ok(()),
    };

    tx.execute(
        "INSERT OR IGNORE INTO pet_cosmetic_unlocks (
            id, pet_id, cosmetic_type, cosmetic_key, unlocked_at, equipped
         ) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        params![
            format!("pet-cosmetic-{}-{}", profile.id, cosmetic_key),
            profile.id,
            cosmetic_type,
            cosmetic_key,
            unlocked_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn ensure_default_exists(conn: &Connection) -> LocalResult<()> {
    let existing = conn
        .query_row(
            "SELECT id FROM pet_profile WHERE id = 'default-pet'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?;
    if existing.is_some() {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO pet_profile (
            id, name, level, experience, stage, current_mood, created_at, updated_at
         ) VALUES ('default-pet', 'Libby', 1, 0, 'first_meet', 'idle', ?1, ?1)",
        params![now],
    )
    .map_err(|err| err.to_string())?;
    conn.execute(
        "INSERT INTO pet_settings (
            pet_id, desktop_enabled, always_on_top, muted, focus_mode_enabled,
            proactive_level, last_window_x, last_window_y, updated_at
         ) VALUES ('default-pet', 1, 1, 0, 0, 2, NULL, NULL, ?1)",
        params![chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn ensure_default_exists_tx(tx: &Transaction<'_>) -> LocalResult<()> {
    let existing = tx
        .query_row(
            "SELECT id FROM pet_profile WHERE id = 'default-pet'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?;
    if existing.is_some() {
        return Ok(());
    }

    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO pet_profile (
            id, name, level, experience, stage, current_mood, created_at, updated_at
         ) VALUES ('default-pet', 'Libby', 1, 0, 'first_meet', 'idle', ?1, ?1)",
        params![now],
    )
    .map_err(|err| err.to_string())?;
    tx.execute(
        "INSERT INTO pet_settings (
            pet_id, desktop_enabled, always_on_top, muted, focus_mode_enabled,
            proactive_level, last_window_x, last_window_y, updated_at
         ) VALUES ('default-pet', 1, 1, 0, 0, 2, NULL, NULL, ?1)",
        params![chrono::Utc::now().to_rfc3339()],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn map_pet_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<PetProfile> {
    let stored_level = row.get::<_, i64>(2)?;
    let stored_experience = row.get::<_, i64>(3)?;
    let stored_stage = row.get::<_, String>(4)?;
    let calculated_snapshot = pet_leveling::level_snapshot_from_experience(stored_experience);
    let effective_level = stored_level
        .max(calculated_snapshot.level)
        .clamp(1, pet_leveling::MAX_PET_LEVEL);
    let effective_experience = if effective_level > calculated_snapshot.level {
        pet_leveling::total_required_exp_for_level(effective_level)
    } else {
        stored_experience
    };
    let mut level_snapshot = pet_leveling::level_snapshot_from_experience(effective_experience);
    let normalized_stage = pet_leveling::normalize_stage(&stored_stage, level_snapshot.level);
    if normalized_stage != level_snapshot.current_stage {
        level_snapshot.current_stage = normalized_stage.clone();
        level_snapshot.current_stage_label_zh =
            pet_leveling::stage_label_zh(&normalized_stage).into();
        level_snapshot.current_stage_label_en =
            pet_leveling::stage_label_en(&normalized_stage).into();
    }

    Ok(PetProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        level: level_snapshot.level,
        experience: effective_experience,
        stage: normalized_stage,
        level_snapshot,
        current_mood: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}
