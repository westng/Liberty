use rusqlite::Connection;

use crate::{
    application::credential_cleanup,
    infrastructure::{
        credentials::{CredentialStore, CredentialWritePlan},
        ids,
        repositories::{ai_models, credential_cleanup as cleanup_repository},
    },
    local_db::{AiModelCredentialUpdate, AiModelSaveInput, LocalResult},
};

pub fn save_ai_model(
    connection: &mut Connection,
    credential_store: &dyn CredentialStore,
    model: &AiModelSaveInput,
) -> LocalResult<()> {
    validate_model(model)?;
    let existing_reference = ai_models::find_credential_reference(connection, &model.id)?;
    let write_plan = match &model.credential {
        AiModelCredentialUpdate::Set { value } => Some(
            CredentialWritePlan::stage(
                credential_store,
                &model.id,
                existing_reference.as_deref(),
                value,
            )
            .map_err(String::from)?,
        ),
        AiModelCredentialUpdate::Keep | AiModelCredentialUpdate::Clear
            if existing_reference.is_none() =>
        {
            return Err("新建 AI 模型必须设置 API Key。".into());
        }
        _ => None,
    };

    let target_reference = match (&model.credential, write_plan.as_ref()) {
        (AiModelCredentialUpdate::Set { .. }, Some(plan)) => Some(plan.staged_reference()),
        (AiModelCredentialUpdate::Keep, _) => existing_reference.as_deref(),
        (AiModelCredentialUpdate::Clear, _) => None,
        _ => return Err("AI 凭据写入计划无效。".into()),
    };
    let retired_reference = match (&model.credential, write_plan.as_ref()) {
        (AiModelCredentialUpdate::Set { .. }, Some(plan)) => plan.previous_reference(),
        (AiModelCredentialUpdate::Clear, _) => existing_reference.as_deref(),
        (AiModelCredentialUpdate::Keep, _) => None,
        _ => None,
    };

    let publish_result = publish_model(connection, model, target_reference, retired_reference);
    if let Err(error) = publish_result {
        if ai_models::credential_reference_matches(connection, &model.id, target_reference)? {
            return credential_cleanup::recover_pending(connection, credential_store)
                .map_err(|cleanup_error| format!("AI 模型已保存，但{cleanup_error}"));
        }
        if let Some(plan) = write_plan {
            let staged_reference = plan.staged_reference().to_string();
            if let Err(rollback_error) = plan.rollback(credential_store) {
                credential_cleanup::remember_rollback(connection, &model.id, &staged_reference)?;
                return Err(format!(
                    "{error}；暂存凭据清理失败，已记录重试：{rollback_error}"
                ));
            }
        }
        return Err(error);
    }

    if let Err(error) = credential_cleanup::recover_pending(connection, credential_store) {
        return Err(format!("AI 模型已保存，但{error}"));
    }
    Ok(())
}

fn publish_model(
    connection: &mut Connection,
    model: &AiModelSaveInput,
    target_reference: Option<&str>,
    retired_reference: Option<&str>,
) -> LocalResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    ai_models::save_ai_model_metadata_tx(&transaction, model, target_reference)?;
    if let Some(reference) = retired_reference
        .filter(|reference| Some(*reference) != target_reference && !reference.trim().is_empty())
    {
        cleanup_repository::insert_tx(
            &transaction,
            &ids::timestamped_id("credential-cleanup"),
            &model.id,
            reference,
            "retire",
            &now,
        )?;
    }
    transaction.commit().map_err(|error| error.to_string())
}

fn validate_model(model: &AiModelSaveInput) -> LocalResult<()> {
    if model.id.trim().is_empty()
        || model.name.trim().is_empty()
        || model.base_url.trim().is_empty()
        || model.model.trim().is_empty()
    {
        Err("AI 模型 ID、名称、接口地址和模型名称不能为空。".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, cell::RefCell, collections::HashMap};

    use super::*;
    use crate::{
        application::credential_cleanup,
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
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL CHECK (name <> 'fail'),
                   base_url TEXT NOT NULL,
                   api_key TEXT NOT NULL,
                   api_key_ref TEXT NOT NULL DEFAULT '',
                   model TEXT NOT NULL,
                   enabled INTEGER NOT NULL,
                   is_default INTEGER NOT NULL,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );
                 CREATE TABLE credential_cleanup_intents (
                   id TEXT PRIMARY KEY,
                   model_id TEXT NOT NULL,
                   credential_reference TEXT NOT NULL UNIQUE,
                   operation TEXT NOT NULL,
                   attempt INTEGER NOT NULL DEFAULT 0,
                   last_error TEXT,
                   created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL
                 );",
            )
            .expect("schema");
        connection
    }

    fn model(name: &str, value: &str) -> AiModelSaveInput {
        AiModelSaveInput {
            id: "model-a".into(),
            name: name.into(),
            base_url: "https://example.com/v1".into(),
            model: "example".into(),
            enabled: true,
            is_default: true,
            created_at: "2026-08-12T00:00:00Z".into(),
            updated_at: "2026-08-12T00:00:00Z".into(),
            credential: AiModelCredentialUpdate::Set {
                value: value.into(),
            },
        }
    }

    #[test]
    fn database_failure_rolls_back_staged_credential() {
        let mut connection = database();
        let store = TestCredentialStore::default();

        let error = save_ai_model(&mut connection, &store, &model("fail", "secret"))
            .expect_err("database failure");

        assert!(error.contains("CHECK constraint"));
        assert!(store.secrets.borrow().is_empty());
        assert!(!ai_models::model_exists(&connection, "model-a").unwrap());
    }

    #[test]
    fn committed_model_survives_retired_credential_cleanup_failure() {
        let mut connection = database();
        connection
            .execute(
                "INSERT INTO ai_model_configs VALUES
                 ('model-a', 'old', 'https://example.com/v1', '', 'old-ref', 'old', 1, 1, 'now', 'now')",
                [],
            )
            .expect("seed model");
        let store = TestCredentialStore::default();
        store
            .secrets
            .borrow_mut()
            .insert("old-ref".into(), "old-secret".into());
        store.fail_delete.set(true);

        let error = save_ai_model(&mut connection, &store, &model("new", "new-secret"))
            .expect_err("cleanup remains pending");
        assert!(error.contains("AI 模型已保存"));
        let new_reference = ai_models::find_credential_reference(&connection, "model-a")
            .unwrap()
            .expect("new reference");
        assert_ne!(new_reference, "old-ref");
        assert_eq!(
            store.secrets.borrow().get(&new_reference),
            Some(&"new-secret".into())
        );
        assert_eq!(cleanup_repository::list(&connection).unwrap()[0].attempt, 1);

        store.fail_delete.set(false);
        credential_cleanup::recover_pending(&connection, &store).expect("retry cleanup");
        assert!(!store.secrets.borrow().contains_key("old-ref"));
        assert!(cleanup_repository::list(&connection).unwrap().is_empty());
    }
}
