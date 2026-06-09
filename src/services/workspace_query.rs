use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;

const MAX_RESULT_ROWS: usize = 200;
const SQL_LOG_PREVIEW_CHARS: usize = 240;

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceQueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub row_count: usize,
    pub truncated: bool,
}

// @TODO: This is too strongly oriented towards the Catalog Agent Specifically.  This must be generic for all agents.
pub fn workspace_sql_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "workspace_sql",
            "description": "Run a read-only SQL query against the stock research workspace SQLite database (ingest + derived catalog stage). Use SELECT, WITH, EXPLAIN, or PRAGMA only. Search concept_catalog_entries first for candidates (filter by unit, latest_period_end, series_usability); confirm picks in sec_raw_facts. Targets live in canonical_metric_definitions. Always ORDER BY before LIMIT. Prefer filtered catalog queries over full-table scans. Results truncate at 200 rows. Do not wrap queries in markdown fences or leading SQL comments.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Single SQLite read-only statement starting with SELECT, WITH, PRAGMA, or EXPLAIN (no markdown fences or leading comments). Use ORDER BY ... DESC before LIMIT. For catalog search: ORDER BY latest_period_end DESC, fact_count DESC LIMIT 15-20. For latest facts: ORDER BY period_end DESC, filed_at DESC LIMIT 10."
                    }
                },
                "required": ["query"]
            }
        }
    })
}

/// Strip markdown fences and leading SQL comments so model output validates reliably.
pub fn normalize_workspace_sql(sql: &str) -> String {
    let mut text = sql.trim().to_string();
    if text.starts_with("```") {
        text = text
            .trim_start_matches("```sql")
            .trim_start_matches("```SQL")
            .trim_start_matches("```")
            .trim()
            .to_string();
        if let Some(body) = text.strip_suffix("```") {
            text = body.trim().to_string();
        }
    }

    loop {
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            return String::new();
        }
        if let Some(rest) = trimmed.strip_prefix("--") {
            text = match rest.find('\n') {
                Some(index) => rest[index + 1..].to_string(),
                None => String::new(),
            };
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("/*") {
            text = match rest.find("*/") {
                Some(index) => rest[index + 2..].to_string(),
                None => String::new(),
            };
            continue;
        }
        return trimmed.to_string();
    }
}

pub fn validate_read_only_sql(sql: &str) -> Result<()> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err(Error::string("workspace_sql query cannot be empty"));
    }
    if trimmed.contains(';') {
        let without_trailing = trimmed.trim_end_matches(';').trim();
        if without_trailing.contains(';') {
            return Err(Error::string(
                "workspace_sql only supports a single SQL statement per call",
            ));
        }
    }

    let upper = trimmed.to_uppercase();
    for forbidden in [
        "INSERT", "UPDATE", "DELETE", "DROP", "ALTER", "CREATE", "ATTACH", "DETACH", "REPLACE",
        "TRUNCATE", "GRANT", "REINDEX", "VACUUM",
    ] {
        if contains_sql_keyword(&upper, forbidden) {
            return Err(Error::string(&format!(
                "workspace_sql rejected forbidden keyword: {forbidden}"
            )));
        }
    }

    let starts_read_only = upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("PRAGMA")
        || upper.starts_with("EXPLAIN");
    if !starts_read_only {
        return Err(Error::string(
            "workspace_sql only allows SELECT, WITH, PRAGMA, or EXPLAIN statements",
        ));
    }

    Ok(())
}

fn prepare_workspace_sql(raw_sql: &str) -> Result<String> {
    let normalized = normalize_workspace_sql(raw_sql);
    if let Err(err) = validate_read_only_sql(&normalized) {
        tracing::warn!(
            preview = %sql_preview(raw_sql),
            normalized_preview = %sql_preview(&normalized),
            error = %err,
            "workspace_sql rejected query"
        );
        return Err(err);
    }
    Ok(normalized.trim_end_matches(';').trim().to_string())
}

pub async fn execute_workspace_query(
    sqlite_path: &Path,
    sql: &str,
) -> Result<WorkspaceQueryResult> {
    let sql = prepare_workspace_sql(sql)?;

    let path = sqlite_path
        .canonicalize()
        .map_err(|err| Error::string(&format!("invalid workspace sqlite path: {err}")))?;
    let url = format!(
        "sqlite://{}?mode=ro",
        path.to_string_lossy().replace('\\', "/")
    );
    let db = Database::connect(&url)
        .await
        .map_err(|err| Error::string(&format!("failed to open workspace sqlite: {err}")))?;

    let rows = db
        .query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.clone()))
        .await
        .map_err(|err| {
            tracing::warn!(
                preview = %sql_preview(&sql),
                error = %err,
                "workspace_sql query failed"
            );
            Error::string(&format!("workspace_sql query failed: {err}"))
        })?;

    let row_count = rows.len();
    let truncated = row_count > MAX_RESULT_ROWS;
    let rows = rows.into_iter().take(MAX_RESULT_ROWS);

    let mut columns = Vec::new();
    let mut json_rows = Vec::new();
    for row in rows {
        if columns.is_empty() {
            columns = row.column_names();
        }
        let mut object = serde_json::Map::new();
        for column in &columns {
            let value = json_value_from_row(&row, column);
            object.insert(column.clone(), value);
        }
        json_rows.push(Value::Object(object));
    }

    Ok(WorkspaceQueryResult {
        columns,
        rows: json_rows,
        row_count,
        truncated,
    })
}

pub async fn execute_workspace_query_json(sqlite_path: &Path, sql: &str) -> Result<String> {
    let result = execute_workspace_query(sqlite_path, sql).await?;
    serde_json::to_string_pretty(&result)
        .map_err(|err| Error::string(&format!("failed to serialize workspace_sql result: {err}")))
}

fn sql_preview(sql: &str) -> String {
    let collapsed: String = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() <= SQL_LOG_PREVIEW_CHARS {
        collapsed
    } else {
        format!("{}...", &collapsed[..SQL_LOG_PREVIEW_CHARS])
    }
}

fn json_value_from_row(row: &sea_orm::QueryResult, column: &str) -> Value {
    if let Ok(value) = row.try_get::<String>("", column) {
        return Value::String(value);
    }
    if let Ok(value) = row.try_get::<i64>("", column) {
        return json!(value);
    }
    if let Ok(value) = row.try_get::<f64>("", column) {
        return json!(value);
    }
    if let Ok(value) = row.try_get::<bool>("", column) {
        return json!(value);
    }
    Value::Null
}

fn contains_sql_keyword(sql: &str, keyword: &str) -> bool {
    sql.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == keyword)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::{
            concept_catalog::ConceptCatalog,
            workspace_financial_store::materialize_standalone_ingest_workspace,
        },
        workspace::SecRawFact,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn sample_fact() -> SecRawFact {
        SecRawFact {
            taxonomy: "us-gaap".to_string(),
            concept_name: "Revenues".to_string(),
            label: Some("Revenues".to_string()),
            description: None,
            unit: "USD".to_string(),
            form: None,
            start: None,
            end: Some("2026-02-28".to_string()),
            filed: None,
            fiscal_year: None,
            fiscal_period: None,
            accession: None,
            frame: None,
            value: 1_000.0,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-07".to_string(),
        }
    }

    #[tokio::test]
    async fn executes_query_against_materialized_review_workspace() {
        let facts = vec![sample_fact()];
        let entries = ConceptCatalog::materialize_catalog_entries(&facts);
        let path =
            PathBuf::from("target").join(format!("workspace-query-test-{}.sqlite", Uuid::new_v4()));
        materialize_standalone_ingest_workspace(
            &path,
            "ORCL",
            &facts,
            &entries,
            "2026-06-07T00:00:00Z",
        )
        .await
        .unwrap();
        let result = execute_workspace_query(
            &path,
            "SELECT canonical_key, metric_label FROM canonical_metric_definitions ORDER BY display_order LIMIT 3",
        )
        .await
        .unwrap();
        assert!(!result.rows.is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn accepts_select_queries() {
        validate_read_only_sql("SELECT * FROM sec_raw_facts LIMIT 5").unwrap();
    }

    #[test]
    fn rejects_write_queries() {
        assert!(validate_read_only_sql("DELETE FROM sec_raw_facts").is_err());
    }

    #[test]
    fn rejects_multiple_statements() {
        assert!(validate_read_only_sql("SELECT 1; SELECT 2").is_err());
    }

    #[test]
    fn normalizes_leading_line_comments() {
        let normalized = normalize_workspace_sql("-- latest revenue\nSELECT 1");
        assert_eq!(normalized, "SELECT 1");
        validate_read_only_sql(&normalized).unwrap();
    }

    #[test]
    fn normalizes_leading_block_comments() {
        let normalized = normalize_workspace_sql("/* balance sheet */ SELECT 1");
        assert_eq!(normalized, "SELECT 1");
        validate_read_only_sql(&normalized).unwrap();
    }

    #[test]
    fn normalizes_markdown_sql_fence() {
        let normalized = normalize_workspace_sql("```sql\nSELECT 1\n```");
        assert_eq!(normalized, "SELECT 1");
        validate_read_only_sql(&normalized).unwrap();
    }
}
