use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::infrastructure::credentials::{default_credential_store, CredentialStore};
use crate::local_db::{AiModelConfig, AiModelMetadata, AiModelSaveInput, LocalResult};

pub fn list_ai_models(conn: &Connection) -> LocalResult<Vec<AiModelMetadata>> {
    let mut statement = conn
        .prepare(
            "SELECT id, name, base_url,
                    CASE WHEN TRIM(api_key) <> '' THEN 1 ELSE 0 END,
                    COALESCE(api_key_ref, ''), model,
                    enabled, is_default, created_at, updated_at
             FROM ai_model_configs
             ORDER BY datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|error| error.to_string())?;

    let stored_models = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)? != 0,
                row.get::<_, i64>(7)? != 0,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let credential_store = default_credential_store();

    stored_models
        .into_iter()
        .map(
            |(
                id,
                name,
                base_url,
                legacy_credential_present,
                api_key_ref,
                model,
                enabled,
                is_default,
                created_at,
                updated_at,
            )| {
                let credential_present = if legacy_credential_present {
                    true
                } else if api_key_ref.trim().is_empty() {
                    false
                } else {
                    credential_store
                        .get_secret(&api_key_ref)
                        .map_err(String::from)?
                        .is_some_and(|value| !value.trim().is_empty())
                };
                Ok(AiModelMetadata {
                    id,
                    name,
                    base_url,
                    model,
                    enabled,
                    is_default,
                    credential_present,
                    created_at,
                    updated_at,
                })
            },
        )
        .collect()
}

pub fn list_ai_model_options(conn: &Connection) -> LocalResult<Vec<AiModelConfig>> {
    let mut statement = conn
        .prepare(
            "SELECT id, name, model, enabled, is_default
             FROM ai_model_configs
             ORDER BY is_default DESC, datetime(updated_at) DESC, updated_at DESC",
        )
        .map_err(|error| error.to_string())?;
    let models = statement
        .query_map([], |row| {
            Ok(AiModelConfig {
                id: row.get(0)?,
                name: row.get(1)?,
                model: row.get(2)?,
                enabled: row.get::<_, i64>(3)? != 0,
                is_default: row.get::<_, i64>(4)? != 0,
                base_url: String::new(),
                api_key: String::new(),
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(models)
}

pub fn get_ai_model(conn: &Connection, model_id: &str) -> LocalResult<Option<AiModelConfig>> {
    get_ai_model_with_store(conn, model_id, &default_credential_store())
}

pub fn find_credential_reference(conn: &Connection, model_id: &str) -> LocalResult<Option<String>> {
    conn.query_row(
        "SELECT NULLIF(TRIM(api_key_ref), '') FROM ai_model_configs WHERE id = ?1",
        params![model_id],
        |row| row.get(0),
    )
    .optional()
    .map(|value| value.flatten())
    .map_err(|error| error.to_string())
}

pub fn credential_reference_matches(
    conn: &Connection,
    model_id: &str,
    expected_reference: Option<&str>,
) -> LocalResult<bool> {
    let stored = conn
        .query_row(
            "SELECT COALESCE(api_key_ref, '') FROM ai_model_configs WHERE id = ?1",
            params![model_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(stored.as_deref() == Some(expected_reference.unwrap_or_default()))
}

pub fn model_exists(conn: &Connection, model_id: &str) -> LocalResult<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM ai_model_configs WHERE id = ?1)",
        params![model_id],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

fn get_ai_model_with_store(
    conn: &Connection,
    model_id: &str,
    credential_store: &dyn CredentialStore,
) -> LocalResult<Option<AiModelConfig>> {
    let stored = conn
        .query_row(
            "SELECT id, name, base_url, api_key, COALESCE(api_key_ref, ''), model,
                    enabled, is_default
             FROM ai_model_configs WHERE id = ?1",
            params![model_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)? != 0,
                    row.get::<_, i64>(7)? != 0,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((id, name, base_url, legacy_api_key, api_key_ref, model, enabled, is_default)) =
        stored
    else {
        return Ok(None);
    };

    let api_key = if api_key_ref.trim().is_empty() {
        legacy_api_key
    } else {
        credential_store
            .get_secret(&api_key_ref)
            .map_err(String::from)?
            .filter(|secret| !secret.trim().is_empty())
            .unwrap_or(legacy_api_key)
    };

    Ok(Some(AiModelConfig {
        id,
        name,
        base_url,
        api_key,
        model,
        enabled,
        is_default,
    }))
}

pub fn save_ai_model_metadata_tx(
    tx: &Transaction<'_>,
    model: &AiModelSaveInput,
    credential_reference: Option<&str>,
) -> LocalResult<()> {
    if model.is_default {
        tx.execute("UPDATE ai_model_configs SET is_default = 0", [])
            .map_err(|error| error.to_string())?;
    }

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
            "",
            credential_reference.unwrap_or_default(),
            model.model,
            if model.enabled { 1 } else { 0 },
            if model.is_default { 1 } else { 0 },
            model.created_at,
            model.updated_at
        ],
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

pub fn delete_ai_model_tx(tx: &Transaction<'_>, model_id: &str) -> LocalResult<usize> {
    tx.execute(
        "DELETE FROM ai_model_configs WHERE id = ?1",
        params![model_id],
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap};

    use super::{get_ai_model_with_store, save_ai_model_metadata_tx};
    use crate::{
        infrastructure::credentials::{CredentialResult, CredentialStore},
        local_db::{AiModelCredentialUpdate, AiModelMetadata, AiModelSaveInput},
    };
    use rusqlite::{params, Connection};

    #[derive(Default)]
    struct TestCredentialStore {
        secrets: RefCell<HashMap<String, String>>,
    }

    impl CredentialStore for TestCredentialStore {
        fn get_secret(&self, key: &str) -> CredentialResult<Option<String>> {
            Ok(self.secrets.borrow().get(key).cloned())
        }

        fn set_secret(&self, key: &str, value: &str) -> CredentialResult<()> {
            self.secrets
                .borrow_mut()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn delete_secret(&self, key: &str) -> CredentialResult<()> {
            self.secrets.borrow_mut().remove(key);
            Ok(())
        }
    }

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE ai_model_configs (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    base_url TEXT NOT NULL,
                    api_key TEXT NOT NULL,
                    api_key_ref TEXT NOT NULL DEFAULT '',
                    model TEXT NOT NULL,
                    enabled INTEGER NOT NULL,
                    is_default INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );",
            )
            .expect("model schema");
        connection
    }

    fn model(credential: AiModelCredentialUpdate) -> AiModelSaveInput {
        AiModelSaveInput {
            id: "model-a".into(),
            name: "Model A".into(),
            base_url: "https://example.com".into(),
            model: "example".into(),
            enabled: true,
            is_default: true,
            created_at: "2026-07-15T00:00:00Z".into(),
            updated_at: "2026-07-15T00:00:00Z".into(),
            credential,
        }
    }

    #[test]
    fn model_metadata_serialization_excludes_credentials() {
        let value = serde_json::to_value(AiModelMetadata {
            id: "model-a".into(),
            name: "Model A".into(),
            base_url: "https://example.com".into(),
            model: "example".into(),
            enabled: true,
            is_default: true,
            credential_present: true,
            created_at: "2026-07-15T00:00:00Z".into(),
            updated_at: "2026-07-15T00:00:00Z".into(),
        })
        .expect("serialize model metadata");

        assert_eq!(value.as_object().map(serde_json::Map::len), Some(9));
        assert_eq!(
            value.get("credentialPresent"),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(value.get("apiKey").is_none());
        assert!(value.get("apiKeyRef").is_none());
    }

    #[test]
    fn runtime_model_hydrates_credential_inside_rust() {
        let connection = database();
        connection
            .execute(
                "INSERT INTO ai_model_configs VALUES (?1, ?2, ?3, '', ?4, ?5, 1, 1, ?6, ?6)",
                params![
                    "model-a",
                    "Model A",
                    "https://example.com",
                    "ai-model:model-a:api-key",
                    "example",
                    "2026-07-15T00:00:00Z"
                ],
            )
            .expect("seed model");
        let store = TestCredentialStore::default();
        store
            .set_secret("ai-model:model-a:api-key", "runtime-secret")
            .expect("seed credential");

        let model = get_ai_model_with_store(&connection, "model-a", &store)
            .expect("load model")
            .expect("model exists");

        assert_eq!(model.api_key, "runtime-secret");
    }

    #[test]
    fn metadata_save_only_publishes_the_provided_reference() {
        let mut connection = database();
        let transaction = connection.transaction().expect("transaction");

        save_ai_model_metadata_tx(
            &transaction,
            &model(AiModelCredentialUpdate::Set {
                value: "new-secret".into(),
            }),
            Some("ai-model:model-a:api-key:staged:write-1"),
        )
        .expect("save model");
        transaction.commit().expect("commit model");

        let stored = connection
            .query_row(
                "SELECT api_key, api_key_ref FROM ai_model_configs WHERE id = 'model-a'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("stored model");
        assert_eq!(
            stored,
            (
                String::new(),
                "ai-model:model-a:api-key:staged:write-1".into()
            )
        );
    }
}
