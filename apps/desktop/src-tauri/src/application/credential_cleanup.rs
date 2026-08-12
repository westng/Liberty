use rusqlite::Connection;

use crate::{
    infrastructure::{credentials::CredentialStore, ids, repositories::credential_cleanup},
    local_db::LocalResult,
};

pub fn recover_pending(
    connection: &Connection,
    credential_store: &dyn CredentialStore,
) -> LocalResult<()> {
    let intents = credential_cleanup::list(connection)?;
    let mut failures = Vec::new();
    for intent in intents {
        if credential_cleanup::is_referenced(connection, &intent.credential_reference)? {
            credential_cleanup::finish(connection, &intent.id)?;
            continue;
        }
        match credential_store.delete_secret(&intent.credential_reference) {
            Ok(()) => credential_cleanup::finish(connection, &intent.id)?,
            Err(error) => {
                let error = String::from(error);
                credential_cleanup::record_failure(
                    connection,
                    &intent.id,
                    &error,
                    &chrono::Utc::now().to_rfc3339(),
                )?;
                failures.push(format!("{}: {error}", intent.id));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("AI 凭据清理仍有待重试项：{}", failures.join("；")))
    }
}

pub fn remember_rollback(
    connection: &mut Connection,
    model_id: &str,
    credential_reference: &str,
) -> LocalResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    credential_cleanup::insert(
        connection,
        &ids::timestamped_id("credential-cleanup"),
        model_id,
        credential_reference,
        "rollback",
        &now,
    )
}
