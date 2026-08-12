use rusqlite::Connection;

use crate::{
    application::credential_cleanup,
    infrastructure::{
        credentials::CredentialStore,
        ids,
        repositories::{ai_models, credential_cleanup as cleanup_repository},
    },
    local_db::LocalResult,
};

pub fn delete_ai_model(
    connection: &mut Connection,
    credential_store: &dyn CredentialStore,
    model_id: &str,
) -> LocalResult<()> {
    let model_id = model_id.trim();
    if model_id.is_empty() {
        return Err("AI 模型 ID 不能为空。".into());
    }

    let credential_reference = ai_models::find_credential_reference(connection, model_id)?;
    let now = chrono::Utc::now().to_rfc3339();
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    ai_models::delete_ai_model_tx(&transaction, model_id)?;
    if let Some(reference) = credential_reference.as_deref() {
        cleanup_repository::insert_tx(
            &transaction,
            &ids::timestamped_id("credential-cleanup"),
            model_id,
            reference,
            "retire",
            &now,
        )?;
    }
    if let Err(error) = transaction.commit() {
        if ai_models::model_exists(connection, model_id)? {
            return Err(error.to_string());
        }
    }

    credential_cleanup::recover_pending(connection, credential_store)
        .map_err(|error| format!("AI 模型已删除，但{error}"))
}

pub fn recover_ai_credential_cleanup(
    connection: &Connection,
    credential_store: &dyn CredentialStore,
) -> LocalResult<()> {
    credential_cleanup::recover_pending(connection, credential_store)
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, cell::RefCell, collections::HashMap};

    use super::*;
    use crate::{
        domain::error::AppError,
        infrastructure::credentials::{CredentialResult, CredentialStore},
    };

    #[derive(Default)]
    struct TestCredentialStore {
        secrets: RefCell<HashMap<String, String>>,
        fail_delete: Cell<bool>,
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
            if self.fail_delete.get() {
                return Err(AppError::Infrastructure("injected delete failure".into()));
            }
            self.secrets.borrow_mut().remove(key);
            Ok(())
        }
    }

    fn database() -> Connection {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .execute_batch(
                "CREATE TABLE ai_model_configs (
                   id TEXT PRIMARY KEY, name TEXT NOT NULL, base_url TEXT NOT NULL,
                   api_key TEXT NOT NULL, api_key_ref TEXT NOT NULL DEFAULT '',
                   model TEXT NOT NULL, enabled INTEGER NOT NULL, is_default INTEGER NOT NULL,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                 );
                 CREATE TABLE credential_cleanup_intents (
                   id TEXT PRIMARY KEY, model_id TEXT NOT NULL,
                   credential_reference TEXT NOT NULL UNIQUE, operation TEXT NOT NULL,
                   attempt INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                   created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                 );
                 INSERT INTO ai_model_configs VALUES
                   ('model-a', 'model', 'https://example.com/v1', '', 'model-ref', 'model', 1, 1, 'now', 'now');",
            )
            .expect("schema");
        connection
    }

    #[test]
    fn delete_commits_model_removal_and_retries_credential_cleanup() {
        let mut connection = database();
        let store = TestCredentialStore::default();
        store
            .secrets
            .borrow_mut()
            .insert("model-ref".into(), "secret".into());
        store.fail_delete.set(true);

        let error = delete_ai_model(&mut connection, &store, "model-a")
            .expect_err("cleanup remains pending");
        assert!(error.contains("AI 模型已删除"));
        assert!(!ai_models::model_exists(&connection, "model-a").unwrap());
        assert_eq!(cleanup_repository::list(&connection).unwrap()[0].attempt, 1);

        store.fail_delete.set(false);
        recover_ai_credential_cleanup(&connection, &store).expect("startup recovery");
        assert!(!store.secrets.borrow().contains_key("model-ref"));
        assert!(cleanup_repository::list(&connection).unwrap().is_empty());
        delete_ai_model(&mut connection, &store, "model-a").expect("idempotent delete");
    }
}
