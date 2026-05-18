use rusqlite::{params, Connection, Transaction};

use crate::local_db::{LocalResult, MeetingMember};

pub fn list_meeting_members(conn: &Connection) -> LocalResult<Vec<MeetingMember>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, department, sort_order, is_recorder, created_at, updated_at
             FROM meeting_members
             ORDER BY sort_order ASC, datetime(updated_at) DESC, updated_at DESC, name COLLATE NOCASE ASC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(MeetingMember {
                id: row.get(0)?,
                name: row.get(1)?,
                department: row.get(2)?,
                sort_order: row.get(3)?,
                is_recorder: row.get::<_, i64>(4)? != 0,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

pub fn save_meeting_member_tx(tx: &Transaction<'_>, member: &MeetingMember) -> LocalResult<()> {
    if member.is_recorder {
        tx.execute(
            "UPDATE meeting_members SET is_recorder = 0, updated_at = ?1 WHERE id <> ?2 AND is_recorder <> 0",
            params![member.updated_at, member.id],
        )
        .map_err(|err| err.to_string())?;
    }

    tx.execute(
        "INSERT INTO meeting_members (
            id, name, department, sort_order, is_recorder, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            department = excluded.department,
            sort_order = excluded.sort_order,
            is_recorder = excluded.is_recorder,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            member.id,
            member.name.trim(),
            member.department.trim(),
            member.sort_order,
            if member.is_recorder { 1 } else { 0 },
            member.created_at,
            member.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

pub fn delete_meeting_member(conn: &Connection, member_id: &str) -> LocalResult<()> {
    conn.execute(
        "DELETE FROM meeting_members WHERE id = ?1",
        params![member_id],
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}
