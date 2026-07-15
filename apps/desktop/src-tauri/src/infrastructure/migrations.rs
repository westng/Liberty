use std::{fs, path::PathBuf};

use rusqlite::{
    params, Connection, DatabaseName, OptionalExtension, Transaction, TransactionBehavior,
};

use crate::infrastructure::{
    credentials::{
        credential_key_for_ai_model, credential_key_for_remote_api_token, CredentialStore,
    },
    ids,
};
use crate::local_db::LocalResult;

pub const CURRENT_SCHEMA_VERSION: i64 = 8;

pub type MigrationFn = for<'transaction> fn(&Transaction<'transaction>) -> LocalResult<()>;

#[derive(Clone, Copy)]
enum MigrationKind {
    Sql(MigrationFn),
    Credentials,
}

#[derive(Clone, Copy)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    kind: MigrationKind,
}

impl Migration {
    pub const fn new(version: i64, name: &'static str, apply: MigrationFn) -> Self {
        Self {
            version,
            name,
            kind: MigrationKind::Sql(apply),
        }
    }

    pub const fn credentials(version: i64, name: &'static str) -> Self {
        Self {
            version,
            name,
            kind: MigrationKind::Credentials,
        }
    }
}

pub fn ensure_schema_version(conn: &Connection) -> LocalResult<i64> {
    let version = read_schema_version(conn)?;
    reject_unsupported_version(version)?;
    Ok(version)
}

pub fn run_migrations(
    conn: &Connection,
    credential_store: &dyn CredentialStore,
    migrations: &[Migration],
) -> LocalResult<i64> {
    let mut version = ensure_schema_version(conn)?;
    validate_migration_sequence(migrations)?;
    if version < CURRENT_SCHEMA_VERSION {
        create_consistent_backup(conn)?;
        version = ensure_schema_version(conn)?;
    }

    while version < CURRENT_SCHEMA_VERSION {
        let migration = &migrations[version as usize];
        if migration.version != version + 1 {
            return Err(format!(
                "数据库迁移不连续: 当前版本 {version}，下一迁移版本 {}。",
                migration.version
            ));
        }

        version = match migration.kind {
            MigrationKind::Sql(apply) => run_sql_migration(conn, migration, apply)?,
            MigrationKind::Credentials => {
                run_credential_migration(conn, credential_store, migration)?
            }
        };
    }

    if version != CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "数据库迁移未完成: 当前版本 {version}，应用需要版本 {CURRENT_SCHEMA_VERSION}。"
        ));
    }

    Ok(version)
}

fn run_sql_migration(
    conn: &Connection,
    migration: &Migration,
    apply: MigrationFn,
) -> LocalResult<i64> {
    let transaction = exclusive_transaction(conn, migration)?;
    let version = ensure_schema_version(&transaction)?;
    if version >= migration.version {
        return Ok(version);
    }
    ensure_next_version(version, migration)?;
    apply(&transaction).map_err(|error| migration_error(migration, error))?;
    record_schema_version(&transaction, migration)?;
    transaction.commit().map_err(|error| {
        format!(
            "无法提交数据库迁移 v{} ({}): {error}",
            migration.version, migration.name
        )
    })?;
    Ok(migration.version)
}

fn run_credential_migration(
    conn: &Connection,
    credential_store: &dyn CredentialStore,
    migration: &Migration,
) -> LocalResult<i64> {
    let plan = CredentialMigrationPlan::read(conn)?;
    let staged = plan.stage(credential_store)?;
    let result = publish_credential_migration(conn, migration, &staged);
    match result {
        Ok(PublishOutcome::Applied) => {
            staged.finalize(credential_store);
            Ok(migration.version)
        }
        Ok(PublishOutcome::AlreadyApplied(version)) => {
            staged.rollback(credential_store)?;
            Ok(version)
        }
        Err(error) => {
            if credential_migration_was_published(conn, migration.version, &staged)? {
                staged.finalize(credential_store);
                Ok(migration.version)
            } else {
                Err(staged.rollback_with_error(credential_store, error))
            }
        }
    }
}

fn publish_credential_migration(
    conn: &Connection,
    migration: &Migration,
    staged: &StagedCredentialMigration,
) -> LocalResult<PublishOutcome> {
    let transaction = exclusive_transaction(conn, migration)?;
    let version = ensure_schema_version(&transaction)?;
    if version >= migration.version {
        return Ok(PublishOutcome::AlreadyApplied(version));
    }
    ensure_next_version(version, migration)?;
    staged
        .apply(&transaction)
        .map_err(|error| migration_error(migration, error))?;
    record_schema_version(&transaction, migration)?;
    transaction.commit().map_err(|error| {
        format!(
            "无法提交数据库迁移 v{} ({}): {error}",
            migration.version, migration.name
        )
    })?;
    Ok(PublishOutcome::Applied)
}

fn exclusive_transaction<'connection>(
    conn: &'connection Connection,
    migration: &Migration,
) -> LocalResult<Transaction<'connection>> {
    Transaction::new_unchecked(conn, TransactionBehavior::Exclusive).map_err(|error| {
        format!(
            "无法获取数据库迁移排他锁 v{} ({}): {error}",
            migration.version, migration.name
        )
    })
}

fn ensure_next_version(version: i64, migration: &Migration) -> LocalResult<()> {
    if migration.version == version + 1 {
        Ok(())
    } else {
        Err(format!(
            "数据库迁移不连续: 当前版本 {version}，下一迁移版本 {}。",
            migration.version
        ))
    }
}

fn record_schema_version(transaction: &Transaction<'_>, migration: &Migration) -> LocalResult<()> {
    transaction
        .execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![migration.version.to_string()],
        )
        .map_err(|error| {
            format!(
                "无法记录数据库迁移 v{} ({}): {error}",
                migration.version, migration.name
            )
        })?;
    Ok(())
}

fn migration_error(migration: &Migration, error: String) -> String {
    format!(
        "数据库迁移 v{} ({}) 失败: {error}",
        migration.version, migration.name
    )
}

fn create_consistent_backup(conn: &Connection) -> LocalResult<Option<PathBuf>> {
    let version = ensure_schema_version(conn)?;
    if version == 0 || version >= CURRENT_SCHEMA_VERSION {
        return Ok(None);
    }

    let Some(backup_path) = migration_backup_path(conn, version) else {
        // In-memory databases are used by isolated tests and have no durable
        // source file to preserve. Production databases always have a path.
        return Ok(None);
    };
    // SQLite's backup API owns the source read snapshot. Starting it from a
    // write transaction on the same connection can wait on its own lock.
    if let Err(error) = conn.backup(DatabaseName::Main, &backup_path, None) {
        let _ = fs::remove_file(&backup_path);
        return Err(format!(
            "无法创建数据库迁移前备份 {}: {error}",
            backup_path.display()
        ));
    }
    Ok(Some(backup_path))
}

fn migration_backup_path(conn: &Connection, version: i64) -> Option<PathBuf> {
    if version <= 0 {
        return None;
    }
    let source = PathBuf::from(conn.path()?.trim());
    if source.as_os_str().is_empty() {
        return None;
    }
    let file_name = source.file_name()?.to_string_lossy();
    Some(source.with_file_name(format!(
        "{file_name}.pre-migration-v{version}-{}.bak",
        ids::timestamped_id("snapshot")
    )))
}

#[derive(Clone, Copy)]
enum PublishOutcome {
    Applied,
    AlreadyApplied(i64),
}

pub fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    statement: &str,
) -> LocalResult<()> {
    if table_has_column(conn, table, column)? {
        return Ok(());
    }

    conn.execute(statement, []).map_err(|err| err.to_string())?;
    Ok(())
}

struct CredentialMigrationPlan {
    entries: Vec<CredentialMigrationEntry>,
}

struct CredentialMigrationEntry {
    target: CredentialTarget,
    plaintext: String,
    previous_reference: String,
    staged_reference: String,
}

enum CredentialTarget {
    Settings,
    AiModel(String),
}

struct StagedCredentialMigration {
    entries: Vec<CredentialMigrationEntry>,
}

impl CredentialMigrationPlan {
    fn read(conn: &Connection) -> LocalResult<Self> {
        let mut entries = Vec::new();
        if let Some((plaintext, previous_reference)) = conn
            .query_row(
                "SELECT api_token, COALESCE(api_token_ref, '')
                 FROM app_settings WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?
        {
            let plaintext = plaintext.trim().to_string();
            if !plaintext.is_empty() {
                entries.push(CredentialMigrationEntry::new(
                    CredentialTarget::Settings,
                    plaintext,
                    previous_reference,
                    credential_key_for_remote_api_token(),
                ));
            }
        }

        let model_credentials = {
            let mut statement = conn
                .prepare(
                    "SELECT id, api_key, COALESCE(api_key_ref, '')
                     FROM ai_model_configs
                     WHERE TRIM(api_key) <> ''
                     ORDER BY id",
                )
                .map_err(|error| error.to_string())?;
            let credentials = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            credentials
        };
        for (model_id, plaintext, previous_reference) in model_credentials {
            entries.push(CredentialMigrationEntry::new(
                CredentialTarget::AiModel(model_id.clone()),
                plaintext.trim().to_string(),
                previous_reference,
                &credential_key_for_ai_model(&model_id),
            ));
        }
        Ok(Self { entries })
    }

    fn stage(
        self,
        credential_store: &dyn CredentialStore,
    ) -> LocalResult<StagedCredentialMigration> {
        let mut staged = StagedCredentialMigration {
            entries: Vec::with_capacity(self.entries.len()),
        };
        for entry in self.entries {
            if let Err(error) = credential_store
                .set_secret(&entry.staged_reference, &entry.plaintext)
                .map_err(String::from)
            {
                return Err(staged.rollback_with_error(
                    credential_store,
                    format!(
                        "无法把{}迁移到系统凭据库；数据库明文未清除，可修复凭据库后重试: {error}",
                        entry.label()
                    ),
                ));
            }
            staged.entries.push(entry);
        }
        Ok(staged)
    }
}

impl CredentialMigrationEntry {
    fn new(
        target: CredentialTarget,
        plaintext: String,
        previous_reference: String,
        base_reference: &str,
    ) -> Self {
        Self {
            target,
            plaintext,
            previous_reference,
            staged_reference: format!(
                "{base_reference}:migration:{}",
                ids::timestamped_id("write")
            ),
        }
    }

    fn label(&self) -> String {
        match &self.target {
            CredentialTarget::Settings => "远端 API token".into(),
            CredentialTarget::AiModel(model_id) => format!("AI 模型 {model_id} API key"),
        }
    }
}

impl StagedCredentialMigration {
    fn apply(&self, transaction: &Transaction<'_>) -> LocalResult<()> {
        for entry in &self.entries {
            let updated = match &entry.target {
                CredentialTarget::Settings => transaction.execute(
                    "UPDATE app_settings
                     SET api_token = '', api_token_ref = ?1
                     WHERE id = 1 AND api_token = ?2
                       AND COALESCE(api_token_ref, '') = ?3",
                    params![
                        entry.staged_reference,
                        entry.plaintext,
                        entry.previous_reference
                    ],
                ),
                CredentialTarget::AiModel(model_id) => transaction.execute(
                    "UPDATE ai_model_configs
                     SET api_key = '', api_key_ref = ?1
                     WHERE id = ?2 AND api_key = ?3
                       AND COALESCE(api_key_ref, '') = ?4",
                    params![
                        entry.staged_reference,
                        model_id,
                        entry.plaintext,
                        entry.previous_reference
                    ],
                ),
            }
            .map_err(|error| error.to_string())?;
            if updated != 1 {
                return Err(format!(
                    "{}在凭据暂存期间发生变化，迁移已取消。",
                    entry.label()
                ));
            }
        }
        Ok(())
    }

    fn rollback(&self, credential_store: &dyn CredentialStore) -> LocalResult<()> {
        let mut failures = Vec::new();
        for entry in &self.entries {
            if let Err(error) = credential_store.delete_secret(&entry.staged_reference) {
                failures.push(format!("{}: {error}", entry.staged_reference));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!("清理暂存凭据失败: {}", failures.join("; ")))
        }
    }

    fn rollback_with_error(&self, credential_store: &dyn CredentialStore, error: String) -> String {
        match self.rollback(credential_store) {
            Ok(()) => error,
            Err(cleanup_error) => format!("{error}; {cleanup_error}"),
        }
    }

    fn finalize(&self, credential_store: &dyn CredentialStore) {
        for entry in &self.entries {
            if !entry.previous_reference.is_empty()
                && entry.previous_reference != entry.staged_reference
            {
                let _ = credential_store.delete_secret(&entry.previous_reference);
            }
        }
    }
}

fn credential_migration_was_published(
    conn: &Connection,
    version: i64,
    staged: &StagedCredentialMigration,
) -> LocalResult<bool> {
    if ensure_schema_version(conn)? < version {
        return Ok(false);
    }
    for entry in &staged.entries {
        let published = match &entry.target {
            CredentialTarget::Settings => conn.query_row(
                "SELECT 1 FROM app_settings
                 WHERE id = 1 AND api_token = '' AND api_token_ref = ?1",
                params![entry.staged_reference],
                |_| Ok(()),
            ),
            CredentialTarget::AiModel(model_id) => conn.query_row(
                "SELECT 1 FROM ai_model_configs
                 WHERE id = ?1 AND api_key = '' AND api_key_ref = ?2",
                params![model_id, entry.staged_reference],
                |_| Ok(()),
            ),
        }
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
        if !published {
            return Ok(false);
        }
    }
    Ok(true)
}

fn read_schema_version(conn: &Connection) -> LocalResult<i64> {
    let has_metadata_table = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'app_meta'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(|err| format!("无法检查数据库元数据表: {err}"))?
        .is_some();
    if !has_metadata_table {
        return Ok(0);
    }

    let stored_version = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("无法读取数据库 schema_version: {err}"))?;
    let Some(stored_version) = stored_version else {
        return Ok(0);
    };

    let version = stored_version
        .parse::<i64>()
        .map_err(|err| format!("数据库 schema_version 无效: {err}"))?;
    if version < 0 {
        return Err(format!("数据库 schema_version 无效: {version}"));
    }
    Ok(version)
}

fn reject_unsupported_version(version: i64) -> LocalResult<()> {
    if version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "数据库版本 {version} 高于当前应用支持的版本 {CURRENT_SCHEMA_VERSION}。"
        ));
    }
    Ok(())
}

fn validate_migration_sequence(migrations: &[Migration]) -> LocalResult<()> {
    if migrations.len() != CURRENT_SCHEMA_VERSION as usize {
        return Err(format!(
            "数据库迁移列表不完整: 需要 {CURRENT_SCHEMA_VERSION} 个版本，实际为 {} 个。",
            migrations.len()
        ));
    }

    for (index, migration) in migrations.iter().enumerate() {
        let expected_version = index as i64 + 1;
        if migration.version != expected_version {
            return Err(format!(
                "数据库迁移列表顺序无效: 期望 v{expected_version}，实际为 v{}。",
                migration.version
            ));
        }
    }
    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> LocalResult<bool> {
    if !table
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!("数据库表名无效: {table}"));
    }

    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|err| err.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| err.to_string())?;
    for existing_column in columns {
        if existing_column.map_err(|err| err.to_string())? == column {
            return Ok(true);
        }
    }
    Ok(false)
}
