use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::infrastructure::credentials::{
    credential_key_for_ai_model, default_credential_store, CredentialStore,
};
use crate::local_db::{
    AiModelConfig, AiModelCredentialUpdate, AiModelMetadata, AiModelSaveInput, LocalResult,
};

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

pub fn save_ai_model_tx(tx: &Transaction<'_>, model: &AiModelSaveInput) -> LocalResult<()> {
    save_ai_model_tx_with_store(tx, model, &default_credential_store())
}

fn save_ai_model_tx_with_store(
    tx: &Transaction<'_>,
    model: &AiModelSaveInput,
    credential_store: &dyn CredentialStore,
) -> LocalResult<()> {
    let existing_credential = tx
        .query_row(
            "SELECT api_key, COALESCE(api_key_ref, '')
             FROM ai_model_configs WHERE id = ?1",
            params![model.id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let (api_key_for_database, api_key_ref) = match (&model.credential, existing_credential) {
        (AiModelCredentialUpdate::Keep, Some(existing)) => existing,
        (AiModelCredentialUpdate::Set { value }, existing) => {
            let value = value.trim();
            if value.is_empty() {
                return Err("设置 AI 模型凭据时 API Key 不能为空。".into());
            }
            let api_key_ref = existing
                .as_ref()
                .map(|(_, reference)| reference.trim())
                .filter(|reference| !reference.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| credential_key_for_ai_model(&model.id));
            credential_store
                .set_secret(&api_key_ref, value)
                .map_err(String::from)?;
            (String::new(), api_key_ref)
        }
        (AiModelCredentialUpdate::Clear, Some((_, existing_ref))) => {
            let reference = if existing_ref.trim().is_empty() {
                credential_key_for_ai_model(&model.id)
            } else {
                existing_ref
            };
            credential_store
                .delete_secret(&reference)
                .map_err(String::from)?;
            (String::new(), String::new())
        }
        (AiModelCredentialUpdate::Keep | AiModelCredentialUpdate::Clear, None) => {
            return Err("新建 AI 模型必须设置 API Key。".into());
        }
    };

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
            api_key_for_database,
            api_key_ref,
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
    .map_err(|error| error.to_string())?;

    if !credential_key.trim().is_empty() {
        default_credential_store()
            .delete_secret(&credential_key)
            .map_err(String::from)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap};

    use super::{get_ai_model_with_store, save_ai_model_tx_with_store};
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
    fn new_model_requires_set_credential_action() {
        let mut connection = database();
        let store = TestCredentialStore::default();
        let transaction = connection.transaction().expect("transaction");

        let error = save_ai_model_tx_with_store(
            &transaction,
            &model(AiModelCredentialUpdate::Keep),
            &store,
        )
        .expect_err("new model must set credential");

        assert!(error.contains("必须设置 API Key"));
    }

    #[test]
    fn set_stores_new_credential_outside_database() {
        let mut connection = database();
        let store = TestCredentialStore::default();
        let transaction = connection.transaction().expect("transaction");

        save_ai_model_tx_with_store(
            &transaction,
            &model(AiModelCredentialUpdate::Set {
                value: "new-secret".into(),
            }),
            &store,
        )
        .expect("save model");
        transaction.commit().expect("commit model");

        assert_eq!(
            store
                .get_secret("ai-model:model-a:api-key")
                .expect("read credential"),
            Some("new-secret".into())
        );
        let stored = connection
            .query_row(
                "SELECT api_key, api_key_ref FROM ai_model_configs WHERE id = 'model-a'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("stored model");
        assert_eq!(stored, (String::new(), "ai-model:model-a:api-key".into()));
    }

    #[test]
    fn keep_preserves_existing_credential_reference() {
        let mut connection = database();
        connection
            .execute(
                "INSERT INTO ai_model_configs VALUES (?1, ?2, ?3, '', ?4, ?5, 1, 1, ?6, ?6)",
                params![
                    "model-a",
                    "Old Name",
                    "https://example.com",
                    "ai-model:model-a:api-key",
                    "example",
                    "2026-07-15T00:00:00Z"
                ],
            )
            .expect("seed model");
        let store = TestCredentialStore::default();
        store
            .set_secret("ai-model:model-a:api-key", "existing-secret")
            .expect("seed credential");
        let transaction = connection.transaction().expect("transaction");

        save_ai_model_tx_with_store(&transaction, &model(AiModelCredentialUpdate::Keep), &store)
            .expect("save model");
        transaction.commit().expect("commit model");

        assert_eq!(
            store
                .get_secret("ai-model:model-a:api-key")
                .expect("read credential"),
            Some("existing-secret".into())
        );
    }

    #[test]
    fn clear_deletes_existing_credential_and_reference() {
        let mut connection = database();
        connection
            .execute(
                "INSERT INTO ai_model_configs VALUES (?1, ?2, ?3, '', ?4, ?5, 1, 1, ?6, ?6)",
                params![
                    "model-a",
                    "Old Name",
                    "https://example.com",
                    "ai-model:model-a:api-key",
                    "example",
                    "2026-07-15T00:00:00Z"
                ],
            )
            .expect("seed model");
        let store = TestCredentialStore::default();
        store
            .set_secret("ai-model:model-a:api-key", "existing-secret")
            .expect("seed credential");
        let transaction = connection.transaction().expect("transaction");

        save_ai_model_tx_with_store(&transaction, &model(AiModelCredentialUpdate::Clear), &store)
            .expect("save model");
        transaction.commit().expect("commit model");

        assert_eq!(
            store
                .get_secret("ai-model:model-a:api-key")
                .expect("read credential"),
            None
        );
        let stored = connection
            .query_row(
                "SELECT api_key, api_key_ref FROM ai_model_configs WHERE id = 'model-a'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("stored model");
        assert_eq!(stored, (String::new(), String::new()));
    }
}
