use rusqlite::{params, Connection};

use crate::local_db::LocalResult;

pub const CURRENT_SCHEMA_VERSION: i64 = 1;

pub fn ensure_schema_version(conn: &Connection) -> LocalResult<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO app_meta(key, value) VALUES('schema_version', ?1)",
        params![CURRENT_SCHEMA_VERSION.to_string()],
    )
    .map_err(|err| err.to_string())?;

    let version = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|err| err.to_string())?
        .parse::<i64>()
        .map_err(|err| format!("数据库 schema_version 无效: {err}"))?;

    if version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "数据库版本 {version} 高于当前应用支持的版本 {CURRENT_SCHEMA_VERSION}。"
        ));
    }

    Ok(version)
}

pub fn add_column_if_missing(conn: &Connection, statement: &str) -> LocalResult<()> {
    match conn.execute(statement, []) {
        Ok(_) => Ok(()),
        Err(err) if err.to_string().contains("duplicate column name") => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}
