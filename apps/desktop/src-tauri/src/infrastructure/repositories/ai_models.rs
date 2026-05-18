use rusqlite::{params, Connection, Transaction};

use crate::infrastructure::credentials::{
    credential_key_for_ai_model, default_credential_store, CredentialStore,
};
use crate::local_db::{AiModelConfig, LocalResult};

pub fn list_ai_models(conn: &Connection) -> LocalResult<Vec<AiModelConfig>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, base_url, api_key, COALESCE(api_key_ref, ''), model, enabled, is_default, created_at, updated_at
             FROM ai_model_configs
             ORDER BY datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AiModelConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                base_url: row.get(2)?,
                api_key: row.get(3)?,
                api_key_ref: row.get(4)?,
                model: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                is_default: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|err| err.to_string())?;

    let mut models = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    hydrate_model_api_keys(&mut models)?;
    Ok(models)
}

pub fn save_ai_model_tx(tx: &Transaction<'_>, model: &AiModelConfig) -> LocalResult<()> {
    if model.is_default {
        tx.execute("UPDATE ai_model_configs SET is_default = 0", [])
            .map_err(|err| err.to_string())?;
    }

    let api_key_ref = if model.api_key_ref.trim().is_empty() {
        credential_key_for_ai_model(&model.id)
    } else {
        model.api_key_ref.clone()
    };
    let api_key_for_database = if model.api_key.trim().is_empty() {
        String::new()
    } else {
        default_credential_store()
            .set_secret(&api_key_ref, &model.api_key)
            .map_err(String::from)?;
        String::new()
    };

    tx.execute(
        "INSERT INTO ai_model_configs (
            id, name, base_url, api_key, api_key_ref, model, enabled, is_default, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            base_url = excluded.base_url,
            api_key = excluded.api_key,
            api_key_ref = excluded.api_key_ref,
            model = excluded.model,
            enabled = excluded.enabled,
            is_default = excluded.is_default,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            model.id,
            model.name,
            model.base_url,
            api_key_for_database,
            api_key_ref,
            model.model,
            if model.enabled { 1 } else { 0 },
            if model.is_default { 1 } else { 0 },
            model.created_at,
            model.updated_at
        ],
    )
    .map_err(|err| err.to_string())?;

    Ok(())
}

pub fn delete_ai_model(conn: &Connection, model_id: &str) -> LocalResult<()> {
    let credential_key = conn
        .query_row(
            "SELECT COALESCE(api_key_ref, '') FROM ai_model_configs WHERE id = ?1",
            params![model_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_default();

    conn.execute(
        "DELETE FROM ai_model_configs WHERE id = ?1",
        params![model_id],
    )
    .map_err(|err| err.to_string())?;

    if !credential_key.trim().is_empty() {
        default_credential_store()
            .delete_secret(&credential_key)
            .map_err(String::from)?;
    }

    Ok(())
}

fn hydrate_model_api_keys(models: &mut [AiModelConfig]) -> LocalResult<()> {
    let store = default_credential_store();
    for model in models {
        if !model.api_key_ref.trim().is_empty() {
            if let Some(secret) = store.get_secret(&model.api_key_ref).map_err(String::from)? {
                model.api_key = secret;
            }
        }
    }
    Ok(())
}
