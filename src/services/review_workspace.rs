use crate::{
    services::concept_catalog::ConceptCatalog,
    tasks::init_workspace::{ConceptCatalogEntry, SecRawFact, SCHEMA_STATEMENTS},
};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const BULK_INSERT_CHUNK_SIZE: usize = 250;

const REVIEW_SCHEMA_STATEMENTS: &[&str] = &[
    SCHEMA_STATEMENTS[0],
    SCHEMA_STATEMENTS[1],
    SCHEMA_STATEMENTS[2],
    SCHEMA_STATEMENTS[3],
    SCHEMA_STATEMENTS[4],
    SCHEMA_STATEMENTS[5],
    SCHEMA_STATEMENTS[6],
    SCHEMA_STATEMENTS[7],
    SCHEMA_STATEMENTS[8],
    SCHEMA_STATEMENTS[9],
    SCHEMA_STATEMENTS[10],
    SCHEMA_STATEMENTS[11],
];

/// Materialize a temporary workspace SQLite file for agent-driven concept review.
pub async fn materialize_review_workspace(
    ticker: &str,
    raw_facts: &[SecRawFact],
    catalog_entries: &[ConceptCatalogEntry],
    fetched_at: &str,
) -> Result<PathBuf> {
    let path = review_workspace_path();
    let url = format!(
        "sqlite://{}?mode=rwc",
        path.to_string_lossy().replace('\\', "/")
    );
    let db = Database::connect(&url)
        .await
        .map_err(|err| Error::string(&format!("failed to create review workspace: {err}")))?;

    for statement in REVIEW_SCHEMA_STATEMENTS {
        execute_sql(&db, statement).await?;
    }

    ConceptCatalog::seed_canonical_definitions(&db, fetched_at).await?;

    execute_sql(
        &db,
        &format!(
            "INSERT INTO run_metadata (
                id, ticker, run_slug, workspace_path, sqlite_path, status, schema_version,
                created_at, financial_fetch_status, financial_fetch_error
            ) VALUES (
                1, '{}', 'review-playground', '{}', '{}', 'review', 1, '{}', 'ok', NULL
            )",
            sql_quote(ticker),
            sql_quote(&path.to_string_lossy()),
            sql_quote(&path.to_string_lossy()),
            sql_quote(fetched_at),
        ),
    )
    .await?;

    execute_sql(
        &db,
        &format!(
            "INSERT INTO stock_info (id, ticker, updated_at) VALUES (1, '{}', '{}')",
            sql_quote(ticker),
            sql_quote(fetched_at),
        ),
    )
    .await?;

    insert_raw_sec_facts(&db, raw_facts).await?;
    insert_concept_catalog_entries(&db, catalog_entries, fetched_at).await?;

    Ok(path)
}

pub fn workspace_schema_hint() -> &'static str {
    r#"Workspace SQLite tables available via workspace_sql:
- canonical_metric_definitions(canonical_key, metric_key, metric_label, statement_type, unit_hint, display_order)
- concept_catalog_entries(taxonomy, concept_name, label, description, unit, fact_count, earliest_period_end, latest_period_end, latest_filed_at, min_value, max_value, dominant_period_shape, series_usability, plot_readiness, narrative_tags)
- sec_raw_facts(taxonomy, concept_name, label, description, unit, form, period_start, period_end, filed_at, fiscal_year, fiscal_period, accession, frame, metric_value, fetched_at)
- raw_fact_metric_catalog view: grouped concept inventory from sec_raw_facts

Start by selecting all rows from canonical_metric_definitions ordered by display_order.
For each canonical metric, investigate concept_catalog_entries and sec_raw_facts to choose the best direct SEC XBRL concept.
Query latest metric_value and period_end for your selected concept before validating online."#
}

async fn execute_sql(db: &impl ConnectionTrait, sql: &str) -> Result<()> {
    db.execute(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("review workspace SQL failed: {err}")))
        .map(|_| ())
}

async fn insert_raw_sec_facts(db: &impl ConnectionTrait, facts: &[SecRawFact]) -> Result<()> {
    for chunk in facts.chunks(BULK_INSERT_CHUNK_SIZE) {
        let values = chunk
            .iter()
            .map(raw_sec_fact_values)
            .collect::<Vec<_>>()
            .join(",\n");
        execute_sql(
            db,
            &format!(
                "INSERT INTO sec_raw_facts (
                    taxonomy, concept_name, label, description, unit, form, period_start, period_end,
                    filed_at, fiscal_year, fiscal_period, accession, frame, metric_value, raw_json,
                    fetched_at
                ) VALUES
                {values}"
            ),
        )
        .await?;
    }
    Ok(())
}

async fn insert_concept_catalog_entries(
    db: &impl ConnectionTrait,
    entries: &[ConceptCatalogEntry],
    updated_at: &str,
) -> Result<()> {
    for chunk in entries.chunks(BULK_INSERT_CHUNK_SIZE) {
        let values = chunk
            .iter()
            .map(|entry| concept_catalog_entry_values(entry, updated_at))
            .collect::<Vec<_>>()
            .join(",\n");
        execute_sql(
            db,
            &format!(
                "INSERT INTO concept_catalog_entries (
                    taxonomy, concept_name, label, description, unit, fact_count,
                    earliest_period_end, latest_period_end, latest_filed_at, min_value, max_value,
                    period_shape_counts, dominant_period_shape, series_usability, plot_readiness,
                    narrative_tags, updated_at
                ) VALUES
                {values}"
            ),
        )
        .await?;
    }
    Ok(())
}

fn raw_sec_fact_values(fact: &SecRawFact) -> String {
    format!(
        "('{}', '{}', {}, {}, '{}', {}, {}, {}, {}, {}, {}, {}, {}, {}, '{}', '{}')",
        sql_quote(&fact.taxonomy),
        sql_quote(&fact.concept_name),
        sql_value(fact.label.as_deref()),
        sql_value(fact.description.as_deref()),
        sql_quote(&fact.unit),
        sql_value(fact.form.as_deref()),
        sql_value(fact.start.as_deref()),
        sql_value(fact.end.as_deref()),
        sql_value(fact.filed.as_deref()),
        sql_i64(fact.fiscal_year),
        sql_value(fact.fiscal_period.as_deref()),
        sql_value(fact.accession.as_deref()),
        sql_value(fact.frame.as_deref()),
        fact.value,
        sql_quote(&fact.raw_json),
        sql_quote(&fact.fetched_at),
    )
}

fn concept_catalog_entry_values(entry: &ConceptCatalogEntry, updated_at: &str) -> String {
    let period_shape_counts =
        serde_json::to_string(&entry.period_shape_counts).unwrap_or_else(|_| "{}".to_string());
    let narrative_tags =
        serde_json::to_string(&entry.narrative_tags).unwrap_or_else(|_| "[]".to_string());
    format!(
        "('{}', '{}', {}, {}, '{}', {}, {}, {}, {}, {}, {}, '{}', '{}', '{}', '{}', '{}', '{}')",
        sql_quote(&entry.taxonomy),
        sql_quote(&entry.concept_name),
        sql_value(entry.label.as_deref()),
        sql_value(entry.description.as_deref()),
        sql_quote(&entry.unit),
        entry.fact_count,
        sql_value(entry.earliest_period_end.as_deref()),
        sql_value(entry.latest_period_end.as_deref()),
        sql_value(entry.latest_filed_at.as_deref()),
        sql_option_f64(entry.min_value),
        sql_option_f64(entry.max_value),
        sql_quote(&period_shape_counts),
        sql_quote(&entry.dominant_period_shape),
        sql_quote(&entry.series_usability),
        sql_quote(&entry.plot_readiness),
        sql_quote(&narrative_tags),
        sql_quote(updated_at),
    )
}

fn sql_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn sql_value(value: Option<&str>) -> String {
    value
        .map(|inner| format!("'{}'", sql_quote(inner)))
        .unwrap_or_else(|| "NULL".to_string())
}

fn sql_i64(value: Option<i64>) -> String {
    value
        .map(|inner| inner.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

fn sql_option_f64(value: Option<f64>) -> String {
    value
        .map(|inner| inner.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

pub fn cleanup_review_workspace(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn review_workspace_path() -> PathBuf {
    let filename = format!("analogues-review-{}.sqlite", Uuid::new_v4());
    let target_dir = PathBuf::from("target");
    if std::fs::create_dir_all(&target_dir).is_ok() {
        return target_dir.join(filename);
    }
    std::env::temp_dir().join(filename)
}
