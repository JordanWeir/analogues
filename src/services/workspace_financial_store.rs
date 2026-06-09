use crate::{
    services::{
        concept_catalog::ConceptCatalog, concept_review::ConceptReviewDecisionRecord,
        workspace_store::execute_schema,
    },
    workspace::{CanonicalMapping, ConceptCatalogEntry, FundamentalObservation, SecRawFact},
};
use loco_rs::prelude::*;
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, QueryResult, Statement,
    TransactionTrait,
};
use std::{collections::BTreeMap, path::Path};

const BULK_INSERT_CHUNK_SIZE: usize = 250;

pub struct WorkspaceFinancialStore<'a> {
    db: &'a DatabaseConnection,
}

/// Rows written to the compressed `fundamentals` table.
pub struct FundamentalInsert<'a> {
    pub key: &'a str,
    pub label: &'a str,
    pub value: Option<f64>,
    pub text: Option<String>,
    pub unit: Option<&'a str>,
    pub period: Option<String>,
    pub source_note: Option<String>,
}

/// Phase 1 persistence: raw SEC facts and stock metadata only.
pub struct RawIngestPersist<'a> {
    pub fetched_at: &'a str,
    pub company_name: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub source_note: &'a str,
    pub raw_sec_facts: &'a [SecRawFact],
}

/// Phase 1–2 persistence: raw SEC facts and concept catalog only.
pub struct IngestPersist<'a> {
    pub fetched_at: &'a str,
    pub company_name: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub source_note: &'a str,
    pub raw_sec_facts: &'a [SecRawFact],
    pub concept_catalog_entries: &'a [ConceptCatalogEntry],
}

/// Phase 3 persistence: canonical mappings and review audit trail only.
pub struct ResolutionPersist<'a> {
    pub fetched_at: &'a str,
    pub concept_review_decisions: &'a [ConceptReviewDecisionRecord],
    pub canonical_mappings: &'a [CanonicalMapping],
    pub quality_flags: &'a [String],
}

/// Phase 4 persistence: observations, headline fundamentals, and quality flags.
pub struct DerivedPersist<'a> {
    pub fetched_at: &'a str,
    pub observations: &'a [FundamentalObservation],
    pub quality_flags: &'a [String],
    pub fundamentals: &'a [FundamentalInsert<'a>],
}

/// Stock metadata loaded from an existing workspace.
pub struct WorkspaceStockInfo {
    pub ticker: String,
    pub company_name: Option<String>,
    pub currency: Option<String>,
    pub source_note: Option<String>,
    pub updated_at: String,
}

/// Decomposed financial snapshot payload for persistence without task-layer types.
pub struct SnapshotPersist<'a> {
    pub fetched_at: &'a str,
    pub company_name: Option<&'a str>,
    pub currency: Option<&'a str>,
    pub source_note: &'a str,
    pub raw_sec_facts: &'a [SecRawFact],
    pub concept_catalog_entries: &'a [ConceptCatalogEntry],
    pub concept_review_decisions: &'a [ConceptReviewDecisionRecord],
    pub canonical_mappings: &'a [CanonicalMapping],
    pub observations: &'a [FundamentalObservation],
    pub quality_flags: &'a [String],
    pub fundamentals: &'a [FundamentalInsert<'a>],
}

/// Create a standalone SQLite file with schema and ingest layers for tests or playgrounds.
pub async fn materialize_standalone_ingest_workspace(
    sqlite_path: &Path,
    ticker: &str,
    raw_facts: &[SecRawFact],
    catalog_entries: &[ConceptCatalogEntry],
    fetched_at: &str,
) -> Result<()> {
    if let Some(parent) = sqlite_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            Error::string(&format!(
                "failed to create parent directory for {}: {err}",
                sqlite_path.display()
            ))
        })?;
    }
    let url = format!(
        "sqlite://{}?mode=rwc",
        sqlite_path.to_string_lossy().replace('\\', "/")
    );
    let db = Database::connect(&url)
        .await
        .map_err(|err| Error::string(&format!("failed to open standalone workspace: {err}")))?;
    execute_schema(&db).await?;
    ConceptCatalog::seed_canonical_definitions(&db, fetched_at).await?;
    execute_sql(
        &db,
        &format!(
            "INSERT INTO run_metadata (
                id, ticker, run_slug, workspace_path, sqlite_path, status, schema_version,
                created_at, financial_fetch_status, financial_fetch_error
            ) VALUES (
                1, '{}', 'standalone-ingest', '{}', '{}', 'review', 1, '{}', 'ok', NULL
            )",
            sql_quote(ticker),
            sql_quote(&sqlite_path.to_string_lossy()),
            sql_quote(&sqlite_path.to_string_lossy()),
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
    WorkspaceFinancialStore::insert_raw_sec_facts(&db, raw_facts).await?;
    WorkspaceFinancialStore::insert_concept_catalog_entries(&db, catalog_entries, fetched_at)
        .await?;
    Ok(())
}

impl<'a> WorkspaceFinancialStore<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn persist_raw_ingest(&self, input: &RawIngestPersist<'_>) -> Result<()> {
        let txn = self.db.begin().await.map_err(|err| {
            Error::string(&format!("failed to begin raw ingest transaction: {err}"))
        })?;
        self.persist_raw_ingest_in(&txn, input).await?;
        txn.commit().await.map_err(|err| {
            Error::string(&format!("failed to commit raw ingest transaction: {err}"))
        })?;
        Ok(())
    }

    pub async fn persist_raw_ingest_in(
        &self,
        db: &impl ConnectionTrait,
        input: &RawIngestPersist<'_>,
    ) -> Result<()> {
        execute_sql(
            db,
            &format!(
                "UPDATE stock_info
                 SET company_name = {}, currency = {}, source_note = {}, updated_at = '{}'
                 WHERE id = 1",
                sql_value(input.company_name),
                sql_value(input.currency),
                sql_value(Some(input.source_note)),
                sql_quote(input.fetched_at),
            ),
        )
        .await?;

        Self::insert_raw_sec_facts(db, input.raw_sec_facts).await?;
        Ok(())
    }

    pub async fn persist_catalog_entries(
        &self,
        entries: &[ConceptCatalogEntry],
        updated_at: &str,
    ) -> Result<()> {
        Self::insert_concept_catalog_entries(self.db, entries, updated_at).await
    }

    pub async fn persist_ingestion(&self, input: &IngestPersist<'_>) -> Result<()> {
        let txn = self.db.begin().await.map_err(|err| {
            Error::string(&format!("failed to begin ingestion transaction: {err}"))
        })?;
        self.persist_ingestion_in(&txn, input).await?;
        txn.commit().await.map_err(|err| {
            Error::string(&format!("failed to commit ingestion transaction: {err}"))
        })?;
        Ok(())
    }

    pub async fn persist_ingestion_in(
        &self,
        db: &impl ConnectionTrait,
        input: &IngestPersist<'_>,
    ) -> Result<()> {
        execute_sql(
            db,
            &format!(
                "UPDATE stock_info
                 SET company_name = {}, currency = {}, source_note = {}, updated_at = '{}'
                 WHERE id = 1",
                sql_value(input.company_name),
                sql_value(input.currency),
                sql_value(Some(input.source_note)),
                sql_quote(input.fetched_at),
            ),
        )
        .await?;

        Self::insert_raw_sec_facts(db, input.raw_sec_facts).await?;
        Self::insert_concept_catalog_entries(db, input.concept_catalog_entries, input.fetched_at)
            .await?;

        Ok(())
    }

    pub async fn persist_snapshot(&self, input: &SnapshotPersist<'_>) -> Result<()> {
        let txn = self.db.begin().await.map_err(|err| {
            Error::string(&format!(
                "failed to begin financial snapshot transaction: {err}"
            ))
        })?;
        self.persist_snapshot_in(&txn, input).await?;
        txn.commit().await.map_err(|err| {
            Error::string(&format!(
                "failed to commit financial snapshot transaction: {err}"
            ))
        })?;
        Ok(())
    }

    pub async fn persist_canonical_resolution(&self, input: &ResolutionPersist<'_>) -> Result<()> {
        let txn = self.db.begin().await.map_err(|err| {
            Error::string(&format!(
                "failed to begin canonical resolution transaction: {err}"
            ))
        })?;
        self.persist_canonical_resolution_in(&txn, input).await?;
        txn.commit().await.map_err(|err| {
            Error::string(&format!(
                "failed to commit canonical resolution transaction: {err}"
            ))
        })?;
        Ok(())
    }

    pub async fn persist_canonical_resolution_in(
        &self,
        db: &impl ConnectionTrait,
        input: &ResolutionPersist<'_>,
    ) -> Result<()> {
        Self::insert_concept_review_decisions(db, input.concept_review_decisions).await?;
        for mapping in input.canonical_mappings {
            Self::insert_canonical_mapping(db, mapping, input.fetched_at).await?;
        }
        for flag in input.quality_flags {
            Self::insert_data_quality_flag(db, flag, input.fetched_at).await?;
        }
        Ok(())
    }

    pub async fn persist_derived_fundamentals(&self, input: &DerivedPersist<'_>) -> Result<()> {
        let txn = self.db.begin().await.map_err(|err| {
            Error::string(&format!(
                "failed to begin derived fundamentals transaction: {err}"
            ))
        })?;
        Self::clear_derived_layers(&txn).await?;
        self.persist_derived_fundamentals_in(&txn, input).await?;
        txn.commit().await.map_err(|err| {
            Error::string(&format!(
                "failed to commit derived fundamentals transaction: {err}"
            ))
        })?;
        Ok(())
    }

    pub async fn persist_derived_fundamentals_in(
        &self,
        db: &impl ConnectionTrait,
        input: &DerivedPersist<'_>,
    ) -> Result<()> {
        Self::insert_observations(db, input.observations, input.fetched_at).await?;
        for flag in input.quality_flags {
            Self::insert_data_quality_flag(db, flag, input.fetched_at).await?;
        }
        for metric in input.fundamentals {
            Self::insert_fundamental(db, metric, input.fetched_at).await?;
        }
        Ok(())
    }

    pub async fn clear_derived_layers(db: &impl ConnectionTrait) -> Result<()> {
        execute_sql(db, "DELETE FROM fundamental_observations").await?;
        execute_sql(db, "DELETE FROM fundamentals").await?;
        execute_sql(db, "DELETE FROM data_quality_flags").await?;
        Ok(())
    }

    pub async fn load_stock_info(&self) -> Result<WorkspaceStockInfo> {
        let rows = query_all(
            self.db,
            "SELECT ticker, company_name, currency, source_note, updated_at
             FROM stock_info
             WHERE id = 1",
        )
        .await?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| Error::string("workspace is missing stock_info row"))?;
        Ok(WorkspaceStockInfo {
            ticker: row_string(&row, "ticker")?,
            company_name: row_opt_string(&row, "company_name")?,
            currency: row_opt_string(&row, "currency")?,
            source_note: row_opt_string(&row, "source_note")?,
            updated_at: row_string(&row, "updated_at")?,
        })
    }

    pub async fn persist_snapshot_in(
        &self,
        db: &impl ConnectionTrait,
        input: &SnapshotPersist<'_>,
    ) -> Result<()> {
        execute_sql(
            db,
            &format!(
                "UPDATE stock_info
                 SET company_name = {}, currency = {}, source_note = {}, updated_at = '{}'
                 WHERE id = 1",
                sql_value(input.company_name),
                sql_value(input.currency),
                sql_value(Some(input.source_note)),
                sql_quote(input.fetched_at),
            ),
        )
        .await?;

        Self::insert_raw_sec_facts(db, input.raw_sec_facts).await?;
        Self::insert_concept_catalog_entries(db, input.concept_catalog_entries, input.fetched_at)
            .await?;
        Self::insert_concept_review_decisions(db, input.concept_review_decisions).await?;
        for mapping in input.canonical_mappings {
            Self::insert_canonical_mapping(db, mapping, input.fetched_at).await?;
        }
        Self::insert_observations(db, input.observations, input.fetched_at).await?;
        for flag in input.quality_flags {
            Self::insert_data_quality_flag(db, flag, input.fetched_at).await?;
        }
        for metric in input.fundamentals {
            Self::insert_fundamental(db, metric, input.fetched_at).await?;
        }

        Ok(())
    }

    pub async fn load_sec_raw_facts(&self) -> Result<Vec<SecRawFact>> {
        let rows = query_all(
            self.db,
            "SELECT taxonomy, concept_name, label, description, unit, form, period_start, period_end,
                    filed_at, fiscal_year, fiscal_period, accession, frame, metric_value, raw_json, fetched_at
             FROM sec_raw_facts
             ORDER BY id",
        )
        .await?;
        rows.into_iter().map(row_to_sec_raw_fact).collect()
    }

    pub async fn load_concept_catalog_entries(&self) -> Result<Vec<ConceptCatalogEntry>> {
        let rows = query_all(
            self.db,
            "SELECT taxonomy, concept_name, label, description, unit, fact_count,
                    earliest_period_end, latest_period_end, latest_filed_at, min_value, max_value,
                    period_shape_counts, dominant_period_shape, series_usability, plot_readiness, narrative_tags
             FROM concept_catalog_entries
             ORDER BY id",
        )
        .await?;
        rows.into_iter().map(row_to_concept_catalog_entry).collect()
    }

    pub async fn load_active_canonical_mappings(&self) -> Result<Vec<CanonicalMapping>> {
        let rows = query_all(
            self.db,
            "SELECT m.canonical_key, d.metric_key, d.metric_label, d.statement_type,
                    m.taxonomy, m.concept_name, m.unit, m.confidence, m.rationale, m.selected_by, m.is_active
             FROM canonical_metric_mappings m
             JOIN canonical_metric_definitions d ON d.canonical_key = m.canonical_key
             WHERE m.is_active = 1
             ORDER BY m.canonical_key, m.taxonomy, m.concept_name, m.unit",
        )
        .await?;
        rows.into_iter().map(row_to_canonical_mapping).collect()
    }

    pub async fn load_concept_review_decisions(&self) -> Result<Vec<ConceptReviewDecisionRecord>> {
        let rows = query_all(
            self.db,
            "SELECT review_run_id, canonical_key, decision_type, taxonomy, concept_name, unit,
                    confidence, rationale, selected_by, warnings_json, payload_json, created_at
             FROM concept_review_decisions
             ORDER BY id",
        )
        .await?;
        rows.into_iter()
            .map(row_to_concept_review_decision)
            .collect()
    }

    pub async fn load_fundamental_observations(&self) -> Result<Vec<FundamentalObservation>> {
        let rows = query_all(
            self.db,
            "SELECT canonical_key, metric_key, metric_label, statement_type, period_type, period_start,
                    period_end, as_of_date, filed_at, fiscal_year, fiscal_period, metric_value, unit,
                    source_type, source_note, concept_name, form, accession, quality, is_derived
             FROM fundamental_observations
             ORDER BY id",
        )
        .await?;
        rows.into_iter()
            .map(row_to_fundamental_observation)
            .collect()
    }

    pub async fn insert_raw_sec_facts(
        db: &impl ConnectionTrait,
        facts: &[SecRawFact],
    ) -> Result<()> {
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

    pub async fn insert_concept_catalog_entries(
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
                    {values}
                    ON CONFLICT(taxonomy, concept_name, unit) DO UPDATE SET
                        label = excluded.label,
                        description = excluded.description,
                        fact_count = excluded.fact_count,
                        earliest_period_end = excluded.earliest_period_end,
                        latest_period_end = excluded.latest_period_end,
                        latest_filed_at = excluded.latest_filed_at,
                        min_value = excluded.min_value,
                        max_value = excluded.max_value,
                        period_shape_counts = excluded.period_shape_counts,
                        dominant_period_shape = excluded.dominant_period_shape,
                        series_usability = excluded.series_usability,
                        plot_readiness = excluded.plot_readiness,
                        narrative_tags = excluded.narrative_tags,
                        updated_at = excluded.updated_at"
                ),
            )
            .await?;
        }

        Ok(())
    }

    pub async fn insert_concept_review_decisions(
        db: &impl ConnectionTrait,
        decisions: &[ConceptReviewDecisionRecord],
    ) -> Result<()> {
        for chunk in decisions.chunks(BULK_INSERT_CHUNK_SIZE) {
            let values = chunk
                .iter()
                .map(concept_review_decision_values)
                .collect::<Vec<_>>()
                .join(",\n");
            execute_sql(
                db,
                &format!(
                    "INSERT INTO concept_review_decisions (
                        review_run_id, canonical_key, decision_type, taxonomy, concept_name, unit,
                        confidence, rationale, selected_by, warnings_json, payload_json, created_at
                    ) VALUES
                    {values}"
                ),
            )
            .await?;
        }

        Ok(())
    }

    pub async fn insert_canonical_mapping(
        db: &impl ConnectionTrait,
        mapping: &CanonicalMapping,
        updated_at: &str,
    ) -> Result<()> {
        execute_sql(
            db,
            &format!(
                "INSERT INTO canonical_metric_mappings (
                    canonical_key, taxonomy, concept_name, unit, confidence, rationale, selected_by,
                    is_active, created_at, updated_at
                ) VALUES (
                    '{}', '{}', '{}', '{}', '{}', '{}', '{}', {}, '{}', '{}'
                )
                ON CONFLICT(canonical_key, taxonomy, concept_name, unit) DO UPDATE SET
                    confidence = excluded.confidence,
                    rationale = excluded.rationale,
                    selected_by = excluded.selected_by,
                    is_active = excluded.is_active,
                    updated_at = excluded.updated_at",
                sql_quote(&mapping.canonical_key),
                sql_quote(&mapping.taxonomy),
                sql_quote(&mapping.concept_name),
                sql_quote(&mapping.unit),
                sql_quote(&mapping.confidence),
                sql_quote(&mapping.rationale),
                sql_quote(&mapping.selected_by),
                if mapping.is_active { 1 } else { 0 },
                sql_quote(updated_at),
                sql_quote(updated_at),
            ),
        )
        .await
    }

    pub async fn insert_observations(
        db: &impl ConnectionTrait,
        observations: &[FundamentalObservation],
        updated_at: &str,
    ) -> Result<()> {
        for chunk in observations.chunks(BULK_INSERT_CHUNK_SIZE) {
            let values = chunk
                .iter()
                .map(|observation| observation_values(observation, updated_at))
                .collect::<Vec<_>>()
                .join(",\n");
            execute_sql(
                db,
                &format!(
                    "INSERT INTO fundamental_observations (
                        canonical_key, metric_key, metric_label, statement_type, period_type, period_start, period_end,
                        as_of_date, filed_at, fiscal_year, fiscal_period, metric_value, unit,
                        source_type, source_note, concept_name, form, accession, quality, is_derived,
                        updated_at
                    ) VALUES
                    {values}"
                ),
            )
            .await?;
        }

        Ok(())
    }

    pub async fn insert_data_quality_flag(
        db: &impl ConnectionTrait,
        flag: &str,
        created_at: &str,
    ) -> Result<()> {
        execute_sql(
            db,
            &format!(
                "INSERT INTO data_quality_flags (flag_key, severity, description, created_at)
                 VALUES ('{}', 'info', '{}', '{}')
                 ON CONFLICT(flag_key, metric_key, period) DO UPDATE SET
                    severity = excluded.severity,
                    description = excluded.description,
                    created_at = excluded.created_at",
                sql_quote(flag),
                sql_quote(&flag.replace('_', " ")),
                sql_quote(created_at),
            ),
        )
        .await
    }

    pub async fn insert_fundamental(
        db: &impl ConnectionTrait,
        metric: &FundamentalInsert<'_>,
        updated_at: &str,
    ) -> Result<()> {
        execute_sql(
            db,
            &format!(
                "INSERT INTO fundamentals (
                    metric_key, metric_label, metric_value, metric_text, unit, period, source_note, updated_at
                ) VALUES (
                    '{}', '{}', {}, {}, {}, {}, {}, '{}'
                )
                ON CONFLICT(metric_key, period) DO UPDATE SET
                    metric_label = excluded.metric_label,
                    metric_value = excluded.metric_value,
                    metric_text = excluded.metric_text,
                    unit = excluded.unit,
                    source_note = excluded.source_note,
                    updated_at = excluded.updated_at",
                sql_quote(metric.key),
                sql_quote(metric.label),
                sql_number(metric.value),
                sql_value(metric.text.as_deref()),
                sql_value(metric.unit),
                sql_value(metric.period.as_deref()),
                sql_value(metric.source_note.as_deref()),
                sql_quote(updated_at),
            ),
        )
        .await
    }
}

async fn query_all(db: &DatabaseConnection, sql: &str) -> Result<Vec<QueryResult>> {
    db.query_all(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("financial store query failed: {err}")))
}

fn row_to_sec_raw_fact(row: QueryResult) -> Result<SecRawFact> {
    Ok(SecRawFact {
        taxonomy: row_string(&row, "taxonomy")?,
        concept_name: row_string(&row, "concept_name")?,
        label: row_opt_string(&row, "label")?,
        description: row_opt_string(&row, "description")?,
        unit: row_string(&row, "unit")?,
        form: row_opt_string(&row, "form")?,
        start: row_opt_string(&row, "period_start")?,
        end: row_opt_string(&row, "period_end")?,
        filed: row_opt_string(&row, "filed_at")?,
        fiscal_year: row_opt_i64(&row, "fiscal_year")?,
        fiscal_period: row_opt_string(&row, "fiscal_period")?,
        accession: row_opt_string(&row, "accession")?,
        frame: row_opt_string(&row, "frame")?,
        value: row_f64(&row, "metric_value")?,
        raw_json: row_string(&row, "raw_json")?,
        fetched_at: row_string(&row, "fetched_at")?,
    })
}

fn row_to_concept_catalog_entry(row: QueryResult) -> Result<ConceptCatalogEntry> {
    let period_shape_counts: BTreeMap<String, i64> =
        serde_json::from_str(&row_string(&row, "period_shape_counts")?).unwrap_or_default();
    let narrative_tags: Vec<String> =
        serde_json::from_str(&row_string(&row, "narrative_tags")?).unwrap_or_default();
    Ok(ConceptCatalogEntry {
        taxonomy: row_string(&row, "taxonomy")?,
        concept_name: row_string(&row, "concept_name")?,
        label: row_opt_string(&row, "label")?,
        description: row_opt_string(&row, "description")?,
        unit: row_string(&row, "unit")?,
        fact_count: row_i64(&row, "fact_count")?,
        earliest_period_end: row_opt_string(&row, "earliest_period_end")?,
        latest_period_end: row_opt_string(&row, "latest_period_end")?,
        latest_filed_at: row_opt_string(&row, "latest_filed_at")?,
        min_value: row_opt_f64(&row, "min_value")?,
        max_value: row_opt_f64(&row, "max_value")?,
        period_shape_counts,
        dominant_period_shape: row_string(&row, "dominant_period_shape")?,
        series_usability: row_string(&row, "series_usability")?,
        plot_readiness: row_string(&row, "plot_readiness")?,
        narrative_tags,
    })
}

fn row_to_canonical_mapping(row: QueryResult) -> Result<CanonicalMapping> {
    Ok(CanonicalMapping {
        canonical_key: row_string(&row, "canonical_key")?,
        metric_key: row_string(&row, "metric_key")?,
        metric_label: row_string(&row, "metric_label")?,
        statement_type: row_string(&row, "statement_type")?,
        taxonomy: row_string(&row, "taxonomy")?,
        concept_name: row_string(&row, "concept_name")?,
        unit: row_string(&row, "unit")?,
        confidence: row_string(&row, "confidence")?,
        rationale: row_string(&row, "rationale")?,
        selected_by: row_string(&row, "selected_by")?,
        is_active: row_i64(&row, "is_active")? != 0,
    })
}

fn row_to_concept_review_decision(row: QueryResult) -> Result<ConceptReviewDecisionRecord> {
    let warnings: Vec<String> =
        serde_json::from_str(&row_string(&row, "warnings_json")?).unwrap_or_default();
    Ok(ConceptReviewDecisionRecord {
        review_run_id: row_string(&row, "review_run_id")?,
        canonical_key: row_opt_string(&row, "canonical_key")?,
        decision_type: row_string(&row, "decision_type")?,
        taxonomy: row_opt_string(&row, "taxonomy")?,
        concept_name: row_opt_string(&row, "concept_name")?,
        unit: row_opt_string(&row, "unit")?,
        confidence: row_string(&row, "confidence")?,
        rationale: row_string(&row, "rationale")?,
        selected_by: row_string(&row, "selected_by")?,
        warnings,
        payload_json: row_string(&row, "payload_json")?,
        created_at: row_string(&row, "created_at")?,
    })
}

fn row_to_fundamental_observation(row: QueryResult) -> Result<FundamentalObservation> {
    Ok(FundamentalObservation {
        canonical_key: row_opt_string(&row, "canonical_key")?,
        metric_key: row_string(&row, "metric_key")?,
        metric_label: row_string(&row, "metric_label")?,
        statement_type: row_string(&row, "statement_type")?,
        period_type: row_string(&row, "period_type")?,
        period_start: row_opt_string(&row, "period_start")?,
        period_end: row_opt_string(&row, "period_end")?,
        as_of_date: row_opt_string(&row, "as_of_date")?,
        filed_at: row_opt_string(&row, "filed_at")?,
        fiscal_year: row_opt_i64(&row, "fiscal_year")?,
        fiscal_period: row_opt_string(&row, "fiscal_period")?,
        value: row_f64(&row, "metric_value")?,
        unit: row_opt_string(&row, "unit")?,
        source_type: row_string(&row, "source_type")?,
        source_note: row_opt_string(&row, "source_note")?,
        concept_name: row_opt_string(&row, "concept_name")?,
        form: row_opt_string(&row, "form")?,
        accession: row_opt_string(&row, "accession")?,
        quality: row_opt_string(&row, "quality")?,
        is_derived: row_i64(&row, "is_derived")? != 0,
    })
}

fn row_string(row: &QueryResult, column: &str) -> Result<String> {
    row.try_get::<String>("", column)
        .map_err(|err| Error::string(&format!("missing column {column}: {err}")))
}

fn row_opt_string(row: &QueryResult, column: &str) -> Result<Option<String>> {
    row.try_get::<Option<String>>("", column)
        .map_err(|err| Error::string(&format!("missing column {column}: {err}")))
}

fn row_i64(row: &QueryResult, column: &str) -> Result<i64> {
    row.try_get::<i64>("", column)
        .map_err(|err| Error::string(&format!("missing column {column}: {err}")))
}

fn row_opt_i64(row: &QueryResult, column: &str) -> Result<Option<i64>> {
    row.try_get::<Option<i64>>("", column)
        .map_err(|err| Error::string(&format!("missing column {column}: {err}")))
}

fn row_f64(row: &QueryResult, column: &str) -> Result<f64> {
    row.try_get::<f64>("", column)
        .map_err(|err| Error::string(&format!("missing column {column}: {err}")))
}

fn row_opt_f64(row: &QueryResult, column: &str) -> Result<Option<f64>> {
    row.try_get::<Option<f64>>("", column)
        .map_err(|err| Error::string(&format!("missing column {column}: {err}")))
}

async fn execute_sql(db: &impl ConnectionTrait, sql: &str) -> Result<()> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("failed to execute SQL statement: {err}")))?;
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
        sql_number(entry.min_value),
        sql_number(entry.max_value),
        sql_quote(&period_shape_counts),
        sql_quote(&entry.dominant_period_shape),
        sql_quote(&entry.series_usability),
        sql_quote(&entry.plot_readiness),
        sql_quote(&narrative_tags),
        sql_quote(updated_at),
    )
}

fn concept_review_decision_values(decision: &ConceptReviewDecisionRecord) -> String {
    let warnings_json =
        serde_json::to_string(&decision.warnings).unwrap_or_else(|_| "[]".to_string());
    format!(
        "('{}', {}, '{}', {}, {}, {}, '{}', '{}', '{}', '{}', '{}', '{}')",
        sql_quote(&decision.review_run_id),
        sql_value(decision.canonical_key.as_deref()),
        sql_quote(&decision.decision_type),
        sql_value(decision.taxonomy.as_deref()),
        sql_value(decision.concept_name.as_deref()),
        sql_value(decision.unit.as_deref()),
        sql_quote(&decision.confidence),
        sql_quote(&decision.rationale),
        sql_quote(&decision.selected_by),
        sql_quote(&warnings_json),
        sql_quote(&decision.payload_json),
        sql_quote(&decision.created_at),
    )
}

fn observation_values(observation: &FundamentalObservation, updated_at: &str) -> String {
    format!(
        "({}, '{}', '{}', '{}', '{}', {}, {}, {}, {}, {}, {}, {}, {}, '{}', {}, {}, {}, {}, {}, {}, '{}')",
        sql_value(observation.canonical_key.as_deref()),
        sql_quote(&observation.metric_key),
        sql_quote(&observation.metric_label),
        sql_quote(&observation.statement_type),
        sql_quote(&observation.period_type),
        sql_value(observation.period_start.as_deref()),
        sql_value(observation.period_end.as_deref()),
        sql_value(observation.as_of_date.as_deref()),
        sql_value(observation.filed_at.as_deref()),
        sql_i64(observation.fiscal_year),
        sql_value(observation.fiscal_period.as_deref()),
        observation.value,
        sql_value(observation.unit.as_deref()),
        sql_quote(&observation.source_type),
        sql_value(observation.source_note.as_deref()),
        sql_value(observation.concept_name.as_deref()),
        sql_value(observation.form.as_deref()),
        sql_value(observation.accession.as_deref()),
        sql_value(observation.quality.as_deref()),
        if observation.is_derived { 1 } else { 0 },
        sql_quote(updated_at),
    )
}

fn sql_quote(value: &str) -> String {
    value.replace('\'', "''")
}

fn sql_value(value: Option<&str>) -> String {
    value.map_or_else(
        || "NULL".to_string(),
        |value| format!("'{}'", sql_quote(value)),
    )
}

fn sql_number(value: Option<f64>) -> String {
    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
}

fn sql_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "NULL".to_string(), |value| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::{concept_catalog::ConceptCatalog, workspace_store::execute_schema},
        workspace::{seed_database, InitWorkspaceRequest, SecRawFact, WorkspacePaths},
    };
    use sea_orm::Database;

    fn sample_fact() -> SecRawFact {
        SecRawFact {
            taxonomy: "us-gaap".to_string(),
            concept_name: "Revenues".to_string(),
            label: Some("Revenues".to_string()),
            description: None,
            unit: "USD".to_string(),
            form: Some("10-K".to_string()),
            start: Some("2025-01-01".to_string()),
            end: Some("2025-12-31".to_string()),
            filed: Some("2026-02-15".to_string()),
            fiscal_year: Some(2025),
            fiscal_period: Some("FY".to_string()),
            accession: Some("0000123456-26-000001".to_string()),
            frame: None,
            value: 1_000_000.0,
            raw_json: r#"{"val":1000000}"#.to_string(),
            fetched_at: "2026-06-07T00:00:00Z".to_string(),
        }
    }

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:?mode=rwc")
            .await
            .expect("in-memory sqlite");
        execute_schema(&db).await.expect("schema");
        seed_database(
            &db,
            &InitWorkspaceRequest {
                ticker: "MSFT".to_string(),
                date: "2026-06-07".to_string(),
                base_dir: std::path::PathBuf::from("reports/stock-narrative-research"),
                fetch_financials: false,
                mapping_strategy: Some(
                    crate::services::canonical_mapping::ConceptMappingStrategy::CandidateScoring,
                ),
            },
            &WorkspacePaths {
                run_slug: "MSFT-2026-06-07-1".to_string(),
                workspace_dir: std::path::PathBuf::from("/tmp/msft"),
                sqlite_path: std::path::PathBuf::from("/tmp/msft/run.sqlite"),
                generated_dir: std::path::PathBuf::from("/tmp/msft/generated"),
            },
        )
        .await
        .expect("seed");
        db
    }

    #[tokio::test]
    async fn round_trips_sec_raw_facts() {
        let db = test_db().await;
        let store = WorkspaceFinancialStore::new(&db);
        let facts = vec![sample_fact()];
        WorkspaceFinancialStore::insert_raw_sec_facts(&db, &facts)
            .await
            .expect("insert");
        let loaded = store.load_sec_raw_facts().await.expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].concept_name, "Revenues");
        assert_eq!(loaded[0].value, 1_000_000.0);
    }

    #[tokio::test]
    async fn persist_ingestion_writes_facts_and_catalog_only() {
        let db = test_db().await;
        let store = WorkspaceFinancialStore::new(&db);
        let facts = vec![sample_fact()];
        let entries = ConceptCatalog::materialize_catalog_entries(&facts);
        store
            .persist_ingestion(&IngestPersist {
                fetched_at: "2026-06-07T00:00:00Z",
                company_name: Some("Example Corp"),
                currency: Some("USD"),
                source_note: "ingest only",
                raw_sec_facts: &facts,
                concept_catalog_entries: &entries,
            })
            .await
            .expect("persist ingestion");

        let loaded_facts = store.load_sec_raw_facts().await.expect("load facts");
        let loaded_entries = store
            .load_concept_catalog_entries()
            .await
            .expect("load catalog");
        let mappings = store
            .load_active_canonical_mappings()
            .await
            .expect("load mappings");

        assert_eq!(loaded_facts.len(), 1);
        assert_eq!(loaded_entries.len(), entries.len());
        assert!(mappings.is_empty());
    }

    #[tokio::test]
    async fn round_trips_concept_catalog_entries() {
        let db = test_db().await;
        let store = WorkspaceFinancialStore::new(&db);
        let facts = vec![sample_fact()];
        let entries = ConceptCatalog::materialize_catalog_entries(&facts);
        WorkspaceFinancialStore::insert_concept_catalog_entries(
            &db,
            &entries,
            "2026-06-07T00:00:00Z",
        )
        .await
        .expect("insert");
        let loaded = store.load_concept_catalog_entries().await.expect("load");
        assert_eq!(loaded.len(), entries.len());
        assert_eq!(loaded[0].concept_name, entries[0].concept_name);
    }
}
