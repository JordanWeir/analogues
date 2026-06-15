use crate::{
    agents::scenario_builder::types::{ScenarioBlueprintOutput, ScenarioDetailOutput},
    services::workspace_sql::{execute_sql, scalar_i64, sql_i64, sql_literal, sql_number, sql_value},
};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement, TransactionTrait};
use std::collections::HashSet;

pub struct ScenarioStore<'a> {
    db: &'a sea_orm::DatabaseConnection,
}

impl<'a> ScenarioStore<'a> {
    pub fn new(db: &'a sea_orm::DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn clear_scenario_workspace(&self) -> Result<()> {
        for sql in [
            "DELETE FROM scenario_signals",
            "DELETE FROM scenario_sensitivities",
            "DELETE FROM scenario_crux_assumptions",
            "DELETE FROM scenario_periods",
            "DELETE FROM scenario_assumptions",
            "DELETE FROM monte_carlo_histogram_bins",
            "DELETE FROM monte_carlo_scenario_probabilities",
            "DELETE FROM monte_carlo_summary",
        ] {
            execute_sql(self.db, sql).await?;
        }
        Ok(())
    }

    pub async fn count_scenarios(&self) -> Result<i64> {
        scalar_i64(
            self.db,
            "SELECT COUNT(*) AS count FROM scenario_assumptions",
        )
        .await
    }

    pub async fn count_scenarios_with_periods(&self) -> Result<i64> {
        scalar_i64(
            self.db,
            "SELECT COUNT(DISTINCT scenario_id) AS count FROM scenario_periods",
        )
        .await
    }

    pub async fn persist_blueprint(&self, output: &ScenarioBlueprintOutput) -> Result<()> {
        for (index, scenario) in output.scenarios.iter().enumerate() {
            execute_sql(
                self.db,
                &format!(
                    "INSERT INTO scenario_assumptions (
                        scenario_order, scenario_key, name, stance, probability,
                        description, assumption_summary
                     ) VALUES (
                        {}, {}, {}, {}, {}, {}, {}
                     )",
                    index as i64 + 1,
                    sql_literal(Some(&scenario.scenario_key)),
                    sql_literal(Some(&scenario.name)),
                    sql_literal(Some(&scenario.stance)),
                    scenario.probability,
                    sql_literal(Some(&scenario.description)),
                    sql_literal(Some(&scenario.crux_resolution_summary)),
                ),
            )
            .await?;
        }
        Ok(())
    }

    pub async fn validate_detail_references(
        db: &impl ConnectionTrait,
        output: &ScenarioDetailOutput,
    ) -> Result<()> {
        let sources = load_source_rows(db).await?;
        let source_ids: HashSet<i64> = sources.iter().map(|(id, _)| *id).collect();
        let experiment_keys = load_experiment_key_set(db).await?;

        for crux in &output.crux_assumptions {
            if let Some(source_id) = crux.source_id {
                if !source_ids.contains(&source_id) {
                    return Err(Error::string(&format!(
                        "invalid source_id {source_id} in crux_assumption crux_key='{}'; \
                         reuse an id from sources or omit source_id. Valid sources: {}",
                        crux.crux_key,
                        format_valid_sources(&sources),
                    )));
                }
            }
            if let Some(key) = crux.experiment_key.as_deref().filter(|key| !key.is_empty()) {
                if !experiment_keys.contains(key) {
                    return Err(Error::string(&format!(
                        "invalid experiment_key '{key}' in crux_assumption crux_key='{}'; \
                         use a promoted analysis_experiments.experiment_key or omit experiment_key",
                        crux.crux_key,
                    )));
                }
            }
        }

        Ok(())
    }

    pub async fn persist_detail(&self, output: &ScenarioDetailOutput) -> Result<()> {
        Self::validate_detail_references(self.db, output).await?;
        let txn = self.db.begin().await.map_err(|err| {
            Error::string(&format!("failed to begin scenario detail transaction: {err}"))
        })?;
        self.persist_detail_in(&txn, output).await?;
        txn.commit().await.map_err(|err| {
            Error::string(&format!("failed to commit scenario detail transaction: {err}"))
        })?;
        Ok(())
    }

    async fn persist_detail_in(
        &self,
        db: &impl ConnectionTrait,
        output: &ScenarioDetailOutput,
    ) -> Result<()> {
        let scenario_id = self.scenario_id_for_key_in(db, &output.scenario_key).await?;
        execute_sql(
            db,
            &format!(
                "UPDATE scenario_assumptions
                 SET assumption_summary = {}
                 WHERE id = {}",
                sql_literal(Some(&output.assumption_summary)),
                scenario_id,
            ),
        )
        .await?;

        for table in [
            "scenario_signals",
            "scenario_sensitivities",
            "scenario_crux_assumptions",
            "scenario_periods",
        ] {
            execute_sql(
                db,
                &format!("DELETE FROM {table} WHERE scenario_id = {scenario_id}"),
            )
            .await?;
        }

        for (index, crux) in output.crux_assumptions.iter().enumerate() {
            execute_sql(
                db,
                &format!(
                    "INSERT INTO scenario_crux_assumptions (
                        scenario_id, crux_order, crux_key, experiment_key, crux, assumption, impact, source_id
                     ) VALUES ({}, {}, {}, {}, {}, {}, {}, {})",
                    scenario_id,
                    index as i64 + 1,
                    sql_literal(Some(&crux.crux_key)),
                    sql_value(crux.experiment_key.as_deref()),
                    sql_literal(Some(&crux.crux)),
                    sql_literal(Some(&crux.assumption)),
                    sql_value(crux.impact.as_deref()),
                    sql_i64(crux.source_id),
                ),
            )
            .await?;
        }

        for (index, body) in output.sensitivities.iter().enumerate() {
            execute_sql(
                db,
                &format!(
                    "INSERT INTO scenario_sensitivities (scenario_id, sensitivity_order, body)
                     VALUES ({}, {}, {})",
                    scenario_id,
                    index as i64 + 1,
                    sql_literal(Some(body)),
                ),
            )
            .await?;
        }

        for (index, body) in output.confirming_signals.iter().enumerate() {
            execute_sql(
                db,
                &format!(
                    "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
                     VALUES ({}, {}, 'confirming', {})",
                    scenario_id,
                    index as i64 + 1,
                    sql_literal(Some(body)),
                ),
            )
            .await?;
        }

        for (index, body) in output.breaking_signals.iter().enumerate() {
            execute_sql(
                db,
                &format!(
                    "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
                     VALUES ({}, {}, 'breaking', {})",
                    scenario_id,
                    index as i64 + 1,
                    sql_literal(Some(body)),
                ),
            )
            .await?;
        }

        for period in &output.periods {
            execute_sql(
                db,
                &format!(
                    "INSERT INTO scenario_periods (
                        scenario_id, period_order, label, period_end, period_type,
                        revenue, revenue_growth, diluted_shares, gross_margin, operating_margin,
                        net_margin, net_income, eps, ps_low, ps_median, ps_high,
                        pe_low, pe_median, pe_high, blend_ps_weight, blend_pe_weight, source_note
                     ) VALUES (
                        {}, {}, {}, {}, {}, {}, {}, {}, {}, {},
                        {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}
                     )",
                    scenario_id,
                    period.period_order,
                    sql_literal(Some(&period.label)),
                    sql_literal(Some(&period.period_end)),
                    sql_literal(Some(&period.period_type)),
                    sql_number(period.revenue),
                    sql_number(period.revenue_growth),
                    sql_number(period.diluted_shares),
                    sql_number(period.gross_margin),
                    sql_number(period.operating_margin),
                    sql_number(period.net_margin),
                    sql_number(period.net_income),
                    sql_number(period.eps),
                    sql_number(period.ps_low),
                    sql_number(period.ps_median),
                    sql_number(period.ps_high),
                    sql_number(period.pe_low),
                    sql_number(period.pe_median),
                    sql_number(period.pe_high),
                    period.blend_ps_weight,
                    period.blend_pe_weight,
                    sql_value(period.source_note.as_deref()),
                ),
            )
            .await?;
        }

        Ok(())
    }

    pub async fn load_scenario_keys(&self) -> Result<Vec<(String, String, String)>> {
        self.load_scenario_rows(
            "SELECT scenario_key, name, description
             FROM scenario_assumptions ORDER BY scenario_order",
        )
        .await
    }

    /// Scenarios from the persisted blueprint that have no `scenario_periods` rows yet.
    pub async fn load_scenarios_needing_detail(&self) -> Result<Vec<(String, String, String)>> {
        self.load_scenario_rows(
            "SELECT sa.scenario_key, sa.name, sa.description
             FROM scenario_assumptions sa
             WHERE NOT EXISTS (
                 SELECT 1 FROM scenario_periods sp WHERE sp.scenario_id = sa.id
             )
             ORDER BY sa.scenario_order",
        )
        .await
    }

    async fn load_scenario_rows(&self, sql: &str) -> Result<Vec<(String, String, String)>> {
        self.db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                sql.to_string(),
            ))
            .await
            .map_err(|e| Error::string(&format!("load scenario rows: {e}")))?
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get_by_index(0)
                        .map_err(|e| Error::string(&e.to_string()))?,
                    row.try_get_by_index(1)
                        .map_err(|e| Error::string(&e.to_string()))?,
                    row.try_get_by_index(2)
                        .map_err(|e| Error::string(&e.to_string()))?,
                ))
            })
            .collect()
    }

    async fn scenario_id_for_key_in(
        &self,
        db: &impl ConnectionTrait,
        scenario_key: &str,
    ) -> Result<i64> {
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT id FROM scenario_assumptions WHERE scenario_key = {}",
                    sql_literal(Some(scenario_key))
                ),
            ))
            .await
            .map_err(|e| Error::string(&format!("scenario id lookup: {e}")))?
            .ok_or_else(|| {
                Error::string(&format!(
                    "unknown scenario_key '{scenario_key}' — run blueprint first"
                ))
            })?;
        row.try_get_by_index(0)
            .map_err(|e| Error::string(&format!("read scenario id: {e}")))
    }
}

async fn load_source_rows(db: &impl ConnectionTrait) -> Result<Vec<(i64, String)>> {
    db.query_all(Statement::from_string(
        DatabaseBackend::Sqlite,
        "SELECT id, title FROM sources ORDER BY id".to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("load sources for validation: {err}")))?
    .into_iter()
    .map(|row| {
        let id: i64 = row
            .try_get_by_index(0)
            .map_err(|e| Error::string(&e.to_string()))?;
        let title: String = row
            .try_get_by_index(1)
            .map_err(|e| Error::string(&e.to_string()))?;
        Ok((id, title))
    })
    .collect()
}

async fn load_experiment_key_set(db: &impl ConnectionTrait) -> Result<HashSet<String>> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT experiment_key FROM analysis_experiments".to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load experiment keys: {err}")))?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.try_get_by_index::<String>(0).ok())
        .collect())
}

fn format_valid_sources(sources: &[(i64, String)]) -> String {
    if sources.is_empty() {
        return "(none — omit source_id or capture sources first)".to_string();
    }
    sources
        .iter()
        .map(|(id, title)| format!("{id}={title}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agents::scenario_builder::types::{
            ScenarioCruxAssumptionInput, ScenarioDetailOutput, ScenarioPeriodInput,
        },
        services::workspace_store::execute_schema,
    };
    use sea_orm::Database;

    fn sample_detail(source_id: Option<i64>) -> ScenarioDetailOutput {
        ScenarioDetailOutput {
            scenario_key: "test_scenario".to_string(),
            assumption_summary: "Summary".to_string(),
            crux_assumptions: vec![ScenarioCruxAssumptionInput {
                crux_key: "test_crux".to_string(),
                crux: "Crux".to_string(),
                assumption: "Assumption".to_string(),
                impact: None,
                experiment_key: None,
                source_id,
            }],
            sensitivities: vec!["Sensitivity".to_string()],
            confirming_signals: vec!["Confirm".to_string()],
            breaking_signals: vec!["Break".to_string()],
            periods: vec![ScenarioPeriodInput {
                period_order: 1,
                label: "Q1".to_string(),
                period_end: "2026-05-31".to_string(),
                period_type: "quarter".to_string(),
                revenue: Some(1.0),
                revenue_growth: None,
                diluted_shares: None,
                gross_margin: None,
                operating_margin: None,
                net_margin: None,
                net_income: None,
                eps: None,
                ps_low: None,
                ps_median: Some(5.0),
                ps_high: None,
                pe_low: None,
                pe_median: None,
                pe_high: None,
                blend_ps_weight: 0.5,
                blend_pe_weight: 0.5,
                source_note: None,
            }],
            per_worker: true,
        }
    }

    #[tokio::test]
    async fn rejects_invalid_source_id_before_persist() {
        let path = std::env::temp_dir().join(format!(
            "analogues-scenario-store-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        execute_sql(
            &db,
            "INSERT INTO sources (title) VALUES ('Oracle earnings release')",
        )
        .await
        .expect("source");
        execute_sql(
            &db,
            "INSERT INTO scenario_assumptions (
                scenario_order, scenario_key, name, stance, probability, description, assumption_summary
             ) VALUES (1, 'test_scenario', 'Test', 'bullish', 1.0, 'desc', 'old')",
        )
        .await
        .expect("scenario");
        execute_sql(
            &db,
            "INSERT INTO scenario_periods (
                scenario_id, period_order, label, period_end, period_type, revenue_growth
             ) VALUES (1, 1, 'Q0', '2025-05-31', 'quarter', 0.1)",
        )
        .await
        .expect("period");

        let store = ScenarioStore::new(&db);
        let err = store
            .persist_detail(&sample_detail(Some(99)))
            .await
            .expect_err("invalid source_id");
        assert!(err.to_string().contains("invalid source_id 99"));
        assert!(err.to_string().contains("1=Oracle earnings release"));

        let periods = scalar_i64(
            &db,
            "SELECT COUNT(*) AS count FROM scenario_periods WHERE scenario_id = 1",
        )
        .await
        .expect("count");
        assert_eq!(periods, 1, "invalid source_id should not delete existing periods");
    }

    #[tokio::test]
    async fn persists_detail_with_valid_source_id() {
        let path = std::env::temp_dir().join(format!(
            "analogues-scenario-store-ok-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        execute_sql(
            &db,
            "INSERT INTO sources (title) VALUES ('Oracle earnings release')",
        )
        .await
        .expect("source");
        execute_sql(
            &db,
            "INSERT INTO scenario_assumptions (
                scenario_order, scenario_key, name, stance, probability, description, assumption_summary
             ) VALUES (1, 'test_scenario', 'Test', 'bullish', 1.0, 'desc', 'old')",
        )
        .await
        .expect("scenario");

        let store = ScenarioStore::new(&db);
        store
            .persist_detail(&sample_detail(Some(1)))
            .await
            .expect("persist");

        let source_id = scalar_i64(
            &db,
            "SELECT source_id AS count FROM scenario_crux_assumptions WHERE scenario_id = 1",
        )
        .await
        .expect("source_id");
        assert_eq!(source_id, 1);
    }
}
