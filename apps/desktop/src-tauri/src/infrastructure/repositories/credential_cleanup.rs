use rusqlite::{params, Connection, Transaction};

use crate::local_db::LocalResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialCleanupIntent {
    pub id: String,
    pub model_id: String,
    pub credential_reference: String,
    pub operation: String,
    pub attempt: u32,
}

pub fn insert_tx(
    transaction: &Transaction<'_>,
    id: &str,
    model_id: &str,
    credential_reference: &str,
    operation: &str,
    now: &str,
) -> LocalResult<()> {
    if credential_reference.trim().is_empty() {
        return Ok(());
    }
    transaction
        .execute(
            "INSERT INTO credential_cleanup_intents (
               id, model_id, credential_reference, operation, attempt,
               last_error, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 0, NULL, ?5, ?5)
             ON CONFLICT(credential_reference) DO UPDATE SET
               model_id = excluded.model_id,
               operation = excluded.operation,
               updated_at = excluded.updated_at",
            params![id, model_id, credential_reference, operation, now],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn insert(
    connection: &mut Connection,
    id: &str,
    model_id: &str,
    credential_reference: &str,
    operation: &str,
    now: &str,
) -> LocalResult<()> {
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    insert_tx(
        &transaction,
        id,
        model_id,
        credential_reference,
        operation,
        now,
    )?;
    transaction.commit().map_err(|error| error.to_string())
}

pub fn list(connection: &Connection) -> LocalResult<Vec<CredentialCleanupIntent>> {
    let mut statement = connection
        .prepare(
            "SELECT id, model_id, credential_reference, operation, attempt
             FROM credential_cleanup_intents
             ORDER BY created_at, id",
        )
        .map_err(|error| error.to_string())?;
    let intents = statement
        .query_map([], |row| {
            let attempt = row.get::<_, i64>(4)?;
            Ok(CredentialCleanupIntent {
                id: row.get(0)?,
                model_id: row.get(1)?,
                credential_reference: row.get(2)?,
                operation: row.get(3)?,
                attempt: u32::try_from(attempt).unwrap_or(u32::MAX),
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(intents)
}

pub fn is_referenced(connection: &Connection, credential_reference: &str) -> LocalResult<bool> {
    connection
        .query_row(
            "SELECT EXISTS (
               SELECT 1 FROM ai_model_configs WHERE api_key_ref = ?1
             )",
            params![credential_reference],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

pub fn finish(connection: &Connection, id: &str) -> LocalResult<()> {
    connection
        .execute(
            "DELETE FROM credential_cleanup_intents WHERE id = ?1",
            params![id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn record_failure(
    connection: &Connection,
    id: &str,
    error: &str,
    now: &str,
) -> LocalResult<()> {
    connection
        .execute(
            "UPDATE credential_cleanup_intents
             SET attempt = attempt + 1, last_error = ?2, updated_at = ?3
             WHERE id = ?1",
            params![id, error, now],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}
