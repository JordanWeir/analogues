use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

pub fn sql_quote(value: &str) -> String {
    value.replace('\'', "''")
}

/// Format an optional string as a SQL literal; `None` and empty strings become `NULL`.
pub fn sql_literal(value: Option<&str>) -> String {
    value.filter(|text| !text.is_empty()).map_or_else(
        || "NULL".to_string(),
        |text| format!("'{}'", sql_quote(text)),
    )
}

/// Format an optional string as a SQL literal; only `None` becomes `NULL`.
pub fn sql_value(value: Option<&str>) -> String {
    value.map_or_else(
        || "NULL".to_string(),
        |value| format!("'{}'", sql_quote(value)),
    )
}

pub fn sql_number(value: Option<f64>) -> String {
    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
}

pub fn sql_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
}

pub async fn scalar_i64(db: &impl ConnectionTrait, sql: &str) -> Result<i64> {
    scalar_i64_as(db, sql, "count").await
}

pub async fn scalar_i64_as(db: &impl ConnectionTrait, sql: &str, column: &str) -> Result<i64> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))?
        .ok_or_else(|| Error::string("query returned no row"))?;
    row.try_get::<i64>("", column)
        .map_err(|err| Error::string(&format!("parse {column}: {err}")))
}

pub async fn execute_sql(db: &impl ConnectionTrait, sql: &str) -> Result<()> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("SQL failed: {err}")))?;
    Ok(())
}

pub async fn last_insert_rowid(db: &impl ConnectionTrait) -> Result<i64> {
    scalar_i64_as(db, "SELECT last_insert_rowid() AS id", "id").await
}
