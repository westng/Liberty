use rusqlite::{params, Connection, OptionalExtension};

use crate::local_db::{AiSummaryTemplate, LocalResult};

pub fn list_ai_templates(conn: &Connection) -> LocalResult<Vec<AiSummaryTemplate>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, prompt, include_speaker_by_default,
                    include_timestamp_by_default, builtin, created_at, updated_at
             FROM ai_summary_templates
             ORDER BY builtin DESC, datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AiSummaryTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                prompt: row.get(3)?,
                include_speaker_by_default: row.get::<_, i64>(4)? != 0,
                include_timestamp_by_default: row.get::<_, i64>(5)? != 0,
                builtin: row.get::<_, i64>(6)? != 0,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

pub fn save_ai_template(conn: &Connection, template: &AiSummaryTemplate) -> LocalResult<()> {
    conn.execute(
        "INSERT INTO ai_summary_templates (
            id, name, description, prompt, include_speaker_by_default,
            include_timestamp_by_default, builtin, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            prompt = excluded.prompt,
            include_speaker_by_default = excluded.include_speaker_by_default,
            include_timestamp_by_default = excluded.include_timestamp_by_default,
            builtin = excluded.builtin,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            template.id,
            template.name,
            template.description,
            template.prompt,
            if template.include_speaker_by_default {
                1
            } else {
                0
            },
            if template.include_timestamp_by_default {
                1
            } else {
                0
            },
            if template.builtin { 1 } else { 0 },
            template.created_at,
            template.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn delete_ai_template(conn: &Connection, template_id: &str) -> LocalResult<()> {
    let builtin = conn
        .query_row(
            "SELECT builtin FROM ai_summary_templates WHERE id = ?1",
            params![template_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|err| err.to_string())?;

    if builtin.unwrap_or_default() != 0 {
        return Err("内置模板不可删除。".into());
    }

    conn.execute(
        "DELETE FROM ai_summary_templates WHERE id = ?1",
        params![template_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}
