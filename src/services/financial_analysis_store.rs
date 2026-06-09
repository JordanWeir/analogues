use crate::agents::financial_model_explorer::types::{
    AnalysisExperimentInput, AnalysisOutputRow, CruxCandidateInput, CruxTriageOutput,
    DataGapInput, QualityFlagInput, SupportingMetricPromotion,
};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct CruxCandidateRecord {
    pub id: i64,
    pub crux_key: String,
    pub title: String,
    pub statement: String,
    pub disposition: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct AnalysisExperimentRecord {
    pub experiment_key: String,
    pub crux_id: Option<i64>,
    pub question: String,
    pub disposition: String,
    pub outputs_json: String,
}

#[derive(Debug, Clone)]
pub struct AnalysisRunRecord {
    pub run_key: String,
    pub status: String,
    pub execution_status: String,
}

#[derive(Debug, Clone)]
pub struct FinancialAnalysisStore<'a> {
    db: &'a sea_orm::DatabaseConnection,
}

impl<'a> FinancialAnalysisStore<'a> {
    pub fn new(db: &'a sea_orm::DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn count_promoted_cruxes(&self) -> Result<i64> {
        scalar_i64(
            self.db,
            "SELECT COUNT(*) FROM crux_candidates WHERE disposition = 'promoted' AND status = 'active'",
        )
        .await
    }

    pub async fn count_promoted_experiments(&self) -> Result<i64> {
        scalar_i64(
            self.db,
            "SELECT COUNT(*) FROM analysis_experiments WHERE disposition = 'promoted'",
        )
        .await
    }

    pub async fn load_promoted_cruxes(&self) -> Result<Vec<CruxCandidateRecord>> {
        let rows = query_all(
            self.db,
            "SELECT id, crux_key, title, statement, disposition, status
             FROM crux_candidates
             WHERE disposition = 'promoted' AND status = 'active'
             ORDER BY id",
        )
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(CruxCandidateRecord {
                    id: row_i64(&row, 0)?,
                    crux_key: row_string(&row, 1)?,
                    title: row_string(&row, 2)?,
                    statement: row_string(&row, 3)?,
                    disposition: row_string(&row, 4)?,
                    status: row_string(&row, 5)?,
                })
            })
            .collect()
    }

    pub async fn load_crux_id_by_key(&self, crux_key: &str) -> Result<Option<i64>> {
        let rows = query_all(
            self.db,
            &format!(
                "SELECT id FROM crux_candidates WHERE crux_key = {} LIMIT 1",
                sql_quote(crux_key)
            ),
        )
        .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(row_i64(&rows[0], 0)?))
    }

    pub async fn persist_crux_triage(
        &self,
        output: &CruxTriageOutput,
        selected_by: &str,
        created_at: &str,
        worker_run_id: Option<&str>,
    ) -> Result<Vec<i64>> {
        let mut crux_ids = Vec::new();
        for crux in &output.cruxes {
            let payload = json!({
                "limitations": crux.limitations,
                "cluster_members": crux.cluster_members,
                "linked_claim_ids": crux.linked_claim_ids,
            });
            self.db
                .execute(Statement::from_string(
                    DatabaseBackend::Sqlite,
                    format!(
                        "INSERT INTO crux_candidates (
                            crux_key, title, statement, bridge_archetype, narrative_side,
                            watch_condition, confirming_signal, breaking_signal,
                            disposition, status, rationale, worker_run_id, created_by, created_at, payload_json
                        ) VALUES (
                            {key}, {title}, {statement}, {bridge}, {side},
                            {watch}, {confirm}, {break_sig},
                            {disposition}, 'active', {rationale}, {worker_run}, {created_by}, {created_at}, {payload}
                        )
                        ON CONFLICT(crux_key) DO UPDATE SET
                            title = excluded.title,
                            statement = excluded.statement,
                            bridge_archetype = excluded.bridge_archetype,
                            narrative_side = excluded.narrative_side,
                            watch_condition = excluded.watch_condition,
                            confirming_signal = excluded.confirming_signal,
                            breaking_signal = excluded.breaking_signal,
                            disposition = excluded.disposition,
                            rationale = excluded.rationale,
                            worker_run_id = excluded.worker_run_id,
                            payload_json = excluded.payload_json",
                        key = sql_quote(&crux.crux_key),
                        title = sql_quote(&crux.title),
                        statement = sql_quote(&crux.statement),
                        bridge = sql_quote_opt(crux.bridge_archetype.as_deref()),
                        side = sql_quote_opt(crux.narrative_side.as_deref()),
                        watch = sql_quote(&crux.watch_condition),
                        confirm = sql_quote(&crux.confirming_signal),
                        break_sig = sql_quote(&crux.breaking_signal),
                        disposition = sql_quote(&crux.disposition),
                        rationale = sql_quote(&crux.rationale),
                        worker_run = sql_quote_opt(worker_run_id),
                        created_by = sql_quote(selected_by),
                        created_at = sql_quote(created_at),
                        payload = sql_quote(&payload.to_string()),
                    ),
                ))
                .await
                .map_err(|err| Error::string(&format!("failed to persist crux: {err}")))?;

            let crux_id = self
                .load_crux_id_by_key(&crux.crux_key)
                .await?
                .ok_or_else(|| Error::string("crux insert did not persist"))?;
            crux_ids.push(crux_id);
        }

        for metric in &output.supporting_metrics {
            let crux_id = if let Some(key) = metric.crux_key.as_deref() {
                self.load_crux_id_by_key(key).await?
            } else {
                None
            };
            self.insert_supporting_metric(metric, crux_id, selected_by, created_at)
                .await?;
        }

        for flag in &output.quality_flags {
            self.insert_quality_flag(flag, created_at).await?;
        }

        for gap in &output.open_questions {
            self.insert_data_gap(gap, created_at).await?;
        }

        Ok(crux_ids)
    }

    pub async fn insert_analysis_run(
        &self,
        run_key: &str,
        crux_id: Option<i64>,
        question: &str,
        executed_sql: &str,
        period_basis: &str,
        execution_status: &str,
        row_count: Option<i64>,
        error_message: Option<&str>,
        result_json: &str,
        assumptions_json: &str,
        inputs_json: &str,
        created_at: &str,
        worker_run_id: Option<&str>,
    ) -> Result<()> {
        self.db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "INSERT INTO analysis_runs (
                        run_key, crux_id, question, executed_sql, period_basis,
                        execution_status, row_count, error_message, result_json,
                        assumptions_json, inputs_json, status, worker_run_id, created_at
                    ) VALUES (
                        {run_key}, {crux_id}, {question}, {sql}, {period_basis},
                        {status}, {row_count}, {error}, {result},
                        {assumptions}, {inputs}, 'draft', {worker_run}, {created_at}
                    )",
                    run_key = sql_quote(run_key),
                    crux_id = crux_opt_i64(crux_id),
                    question = sql_quote(question),
                    sql = sql_quote(executed_sql),
                    period_basis = sql_quote(period_basis),
                    status = sql_quote(execution_status),
                    row_count = row_count
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "NULL".to_string()),
                    error = sql_quote_opt(error_message),
                    result = sql_quote(result_json),
                    assumptions = sql_quote(assumptions_json),
                    inputs = sql_quote(inputs_json),
                    worker_run = sql_quote_opt(worker_run_id),
                    created_at = sql_quote(created_at),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to insert analysis run: {err}")))?;
        Ok(())
    }

    pub async fn finalize_analysis_run(
        &self,
        run_key: &str,
        experiment: &AnalysisExperimentInput,
        selected_by: &str,
        created_at: &str,
        worker_run_id: Option<&str>,
    ) -> Result<()> {
        let crux_id = if let Some(key) = experiment.crux_key.as_deref() {
            self.load_crux_id_by_key(key).await?
        } else {
            None
        };

        let assumptions_json = serde_json::to_string(&experiment.assumptions)
            .unwrap_or_else(|_| "[]".to_string());
        let inputs_json =
            serde_json::to_string(&experiment.inputs).unwrap_or_else(|_| "[]".to_string());
        let outputs_json =
            serde_json::to_string(&experiment.outputs).unwrap_or_else(|_| "[]".to_string());
        let bridge_json = experiment
            .bridge
            .as_ref()
            .and_then(|bridge| serde_json::to_string(bridge).ok());

        self.db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "INSERT INTO analysis_experiments (
                        experiment_key, crux_id, question, purpose, sql_body, period_basis,
                        disposition, rejection_reason, source_note, rationale, worker_run_id,
                        created_by, created_at, updated_at,
                        assumptions_json, inputs_json, outputs_json, bridge_json
                    ) VALUES (
                        {key}, {crux_id}, {question}, {purpose}, {sql}, {period_basis},
                        {disposition}, {rejection}, {source_note}, {rationale}, {worker_run},
                        {created_by}, {created_at}, {created_at},
                        {assumptions}, {inputs}, {outputs}, {bridge}
                    )
                    ON CONFLICT(experiment_key) DO UPDATE SET
                        crux_id = excluded.crux_id,
                        question = excluded.question,
                        purpose = excluded.purpose,
                        sql_body = excluded.sql_body,
                        period_basis = excluded.period_basis,
                        disposition = excluded.disposition,
                        rejection_reason = excluded.rejection_reason,
                        source_note = excluded.source_note,
                        rationale = excluded.rationale,
                        worker_run_id = excluded.worker_run_id,
                        updated_at = excluded.updated_at,
                        assumptions_json = excluded.assumptions_json,
                        inputs_json = excluded.inputs_json,
                        outputs_json = excluded.outputs_json,
                        bridge_json = excluded.bridge_json",
                    key = sql_quote(&experiment.experiment_key),
                    crux_id = crux_opt_i64(crux_id),
                    question = sql_quote(&experiment.question),
                    purpose = sql_quote(&experiment.purpose),
                    sql = sql_quote(&experiment.sql_body),
                    period_basis = sql_quote(&experiment.period_basis),
                    disposition = sql_quote(&experiment.disposition),
                    rejection = sql_quote_opt(experiment.rejection_reason.as_deref()),
                    source_note = sql_quote_opt(experiment.source_note.as_deref()),
                    rationale = sql_quote_opt(experiment.rationale.as_deref()),
                    worker_run = sql_quote_opt(worker_run_id),
                    created_by = sql_quote(selected_by),
                    created_at = sql_quote(created_at),
                    assumptions = sql_quote(&assumptions_json),
                    inputs = sql_quote(&inputs_json),
                    outputs = sql_quote(&outputs_json),
                    bridge = bridge_json
                        .as_deref()
                        .map(sql_quote)
                        .unwrap_or_else(|| "NULL".to_string()),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to persist experiment: {err}")))?;

        self.db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "UPDATE analysis_runs
                     SET status = 'finalized',
                         experiment_key = {experiment_key},
                         finalized_at = {created_at}
                     WHERE run_key = {run_key}",
                    experiment_key = sql_quote(&experiment.experiment_key),
                    created_at = sql_quote(created_at),
                    run_key = sql_quote(run_key),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to finalize analysis run: {err}")))?;

        Ok(())
    }

    pub async fn discard_analysis_run(&self, run_key: &str, finalized_at: &str) -> Result<()> {
        self.db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "UPDATE analysis_runs
                     SET status = 'discarded', finalized_at = {finalized_at}
                     WHERE run_key = {run_key}",
                    finalized_at = sql_quote(finalized_at),
                    run_key = sql_quote(run_key),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to discard analysis run: {err}")))?;
        Ok(())
    }

    pub async fn load_analysis_run(&self, run_key: &str) -> Result<Option<AnalysisRunRecord>> {
        let rows = query_all(
            self.db,
            &format!(
                "SELECT run_key, status, execution_status
                 FROM analysis_runs WHERE run_key = {} LIMIT 1",
                sql_quote(run_key)
            ),
        )
        .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(AnalysisRunRecord {
            run_key: row_string(&rows[0], 0)?,
            status: row_string(&rows[0], 1)?,
            execution_status: row_string(&rows[0], 2)?,
        }))
    }

    pub async fn narrative_context_present(&self) -> Result<bool> {
        let map_rows = scalar_i64(
            self.db,
            "SELECT COUNT(*) FROM narrative_map
             WHERE COALESCE(dominant, '') != ''
                OR COALESCE(bull, '') != ''
                OR COALESCE(bear, '') != ''",
        )
        .await?;
        if map_rows > 0 {
            return Ok(true);
        }
        let item_rows = scalar_i64(
            self.db,
            "SELECT COUNT(*) FROM narrative_map_items",
        )
        .await?;
        if item_rows > 0 {
            return Ok(true);
        }
        let claim_rows = scalar_i64(self.db, "SELECT COUNT(*) FROM claims").await?;
        Ok(claim_rows > 0)
    }

    async fn insert_supporting_metric(
        &self,
        metric: &SupportingMetricPromotion,
        crux_id: Option<i64>,
        selected_by: &str,
        created_at: &str,
    ) -> Result<()> {
        self.db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "INSERT INTO supporting_metric_selections (
                        selection_scope, crux_id, taxonomy, concept_name, unit, label,
                        rationale, period_basis, quality_status, selected_by, created_at
                    ) VALUES (
                        {scope}, {crux_id}, {taxonomy}, {concept}, {unit}, {label},
                        {rationale}, {period_basis}, {quality_status}, {selected_by}, {created_at}
                    )",
                    scope = sql_quote(&metric.selection_scope),
                    crux_id = crux_opt_i64(crux_id),
                    taxonomy = sql_quote(&metric.taxonomy),
                    concept = sql_quote(&metric.concept_name),
                    unit = sql_quote(&metric.unit),
                    label = sql_quote_opt(metric.label.as_deref()),
                    rationale = sql_quote(&metric.rationale),
                    period_basis = sql_quote_opt(metric.period_basis.as_deref()),
                    quality_status = sql_quote(
                        metric.quality_status.as_deref().unwrap_or("ok"),
                    ),
                    selected_by = sql_quote(selected_by),
                    created_at = sql_quote(created_at),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to insert supporting metric: {err}")))?;
        Ok(())
    }

    async fn insert_quality_flag(
        &self,
        flag: &QualityFlagInput,
        created_at: &str,
    ) -> Result<()> {
        self.db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "INSERT INTO data_quality_flags (flag_key, severity, description, metric_key, period, created_at)
                     VALUES ({key}, {severity}, {description}, {metric_key}, {period}, {created_at})
                     ON CONFLICT(flag_key, metric_key, period) DO UPDATE SET
                        severity = excluded.severity,
                        description = excluded.description",
                    key = sql_quote(&flag.flag_key),
                    severity = sql_quote(&flag.severity),
                    description = sql_quote(&flag.description),
                    metric_key = sql_quote_opt(flag.metric_key.as_deref()),
                    period = sql_quote_opt(flag.period.as_deref()),
                    created_at = sql_quote(created_at),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to insert quality flag: {err}")))?;
        Ok(())
    }

    async fn insert_data_gap(&self, gap: &DataGapInput, created_at: &str) -> Result<()> {
        self.db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "INSERT INTO data_gaps (gap_key, description, status, created_at)
                     VALUES ({key}, {description}, 'open', {created_at})
                     ON CONFLICT(gap_key) DO UPDATE SET description = excluded.description",
                    key = sql_quote(&gap.gap_key),
                    description = sql_quote(&gap.description),
                    created_at = sql_quote(created_at),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("failed to insert data gap: {err}")))?;
        Ok(())
    }
}

pub fn outputs_include_arithmetic_and_interpretation(outputs: &[AnalysisOutputRow]) -> bool {
    let has_arithmetic = outputs.iter().any(|row| {
        matches!(
            row.kind.as_str(),
            "arithmetic" | "ratio" | "series_point" | "bridge_step"
        )
    });
    let has_interpretation = outputs
        .iter()
        .any(|row| row.kind == "interpretation" && row.text.as_deref().is_some_and(|t| !t.is_empty()));
    has_arithmetic && has_interpretation
}

async fn scalar_i64(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<i64> {
    let rows = query_all(db, sql).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    row_i64(&rows[0], 0)
}

async fn query_all(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
) -> Result<Vec<sea_orm::QueryResult>> {
    db.query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))
}

fn row_i64(row: &sea_orm::QueryResult, index: usize) -> Result<i64> {
    row.try_get_by_index::<i64>(index)
        .map_err(|err| Error::string(&format!("expected integer column {index}: {err}")))
}

fn row_string(row: &sea_orm::QueryResult, index: usize) -> Result<String> {
    row.try_get_by_index::<String>(index)
        .map_err(|err| Error::string(&format!("expected text column {index}: {err}")))
}

fn sql_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn sql_quote_opt(value: Option<&str>) -> String {
    value.map(sql_quote).unwrap_or_else(|| "NULL".to_string())
}

fn crux_opt_i64(value: Option<i64>) -> String {
    value
        .map(|id| id.to_string())
        .unwrap_or_else(|| "NULL".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agents::financial_model_explorer::types::ClusterMemberInput,
        services::workspace_store::execute_schema,
    };
    use sea_orm::Database;

    #[test]
    fn outputs_require_arithmetic_and_interpretation() {
        let valid = vec![
            AnalysisOutputRow {
                kind: "ratio".to_string(),
                label: "Capex / OCF".to_string(),
                value: Some(1.2),
                unit: Some("ratio".to_string()),
                period_end: None,
                formula: None,
                text: None,
            },
            AnalysisOutputRow {
                kind: "interpretation".to_string(),
                label: "Read".to_string(),
                value: None,
                unit: None,
                period_end: None,
                formula: None,
                text: Some("Binding constraint".to_string()),
            },
        ];
        assert!(outputs_include_arithmetic_and_interpretation(&valid));
    }

    #[tokio::test]
    async fn persists_crux_and_supporting_metric() {
        let path = std::env::temp_dir().join(format!(
            "analogues-fin-analysis-store-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");

        let store = FinancialAnalysisStore::new(&db);
        let output = CruxTriageOutput {
            cruxes: vec![CruxCandidateInput {
                crux_key: "rpo_conversion".to_string(),
                title: "RPO conversion".to_string(),
                statement: "Backlog must convert fast enough to fund capex.".to_string(),
                bridge_archetype: Some("backlog_to_cash_conversion".to_string()),
                narrative_side: Some("bear".to_string()),
                watch_condition: "RPO/revenue trend".to_string(),
                confirming_signal: "OCF lags capex".to_string(),
                breaking_signal: "OCF keeps pace with capex".to_string(),
                disposition: "promoted".to_string(),
                rationale: "Core mechanic".to_string(),
                limitations: None,
                cluster_members: vec![ClusterMemberInput {
                    taxonomy: "us-gaap".to_string(),
                    concept_name: "RevenueRemainingPerformanceObligation".to_string(),
                    unit: "USD".to_string(),
                    role: "driver".to_string(),
                    dominant_period_shape: Some("instant".to_string()),
                }],
                linked_claim_ids: vec![],
            }],
            supporting_metrics: vec![SupportingMetricPromotion {
                selection_scope: "crux_support".to_string(),
                crux_key: Some("rpo_conversion".to_string()),
                taxonomy: "us-gaap".to_string(),
                concept_name: "RevenueRemainingPerformanceObligation".to_string(),
                unit: "USD".to_string(),
                label: None,
                rationale: "Backlog driver".to_string(),
                period_basis: Some("instant".to_string()),
                quality_status: Some("ok".to_string()),
            }],
            quality_flags: vec![],
            open_questions: vec![],
        };

        store
            .persist_crux_triage(&output, "test", "2026-06-09T00:00:00Z", None)
            .await
            .expect("persist");

        assert_eq!(store.count_promoted_cruxes().await.expect("count"), 1);
        db.close().await.ok();
    }
}
