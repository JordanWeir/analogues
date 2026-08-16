use crate::{
    agents::financial_model_explorer::types::MechanicsReviewOutput,
    services::{
        financial_analysis_store::{FinancialAnalysisStore, MechanicsDraftScope},
        model_client::extract_json_blob,
        workspace_sql::sql_value,
    },
};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const VERDICT_APPROVED: &str = "approved";
pub const VERDICT_CHANGES_REQUESTED: &str = "changes_requested";
pub const MAX_MECHANICS_REMEDIATION_ROUNDS: usize = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicsReviewFinding {
    pub category: String,
    pub severity: String,
    pub description: String,
    #[serde(default)]
    pub experiment_key: Option<String>,
    #[serde(default)]
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicsReviewRecord {
    pub review_scope_key: String,
    pub scope_type: String,
    pub crux_key: Option<String>,
    pub verdict: String,
    pub summary: String,
    pub findings: Vec<MechanicsReviewFinding>,
    pub experiments_reviewed: Vec<String>,
    pub review_round: i64,
}

pub struct MechanicsReviewService;

impl MechanicsReviewService {
    pub fn parse_output(text: &str) -> Result<MechanicsReviewOutput> {
        let json_text = extract_json_blob(text).ok_or_else(|| {
            Error::string(
                "mechanics review response did not contain JSON; call submit_mechanics_review with valid output",
            )
        })?;
        let output: MechanicsReviewOutput = serde_json::from_str(json_text).map_err(|err| {
            Error::string(&format!("invalid mechanics review JSON: {err}"))
        })?;
        Self::validate_output_shape(&output)?;
        Ok(output)
    }

    pub fn validate_output_shape(output: &MechanicsReviewOutput) -> Result<()> {
        if output.summary.trim().is_empty() {
            return Err(Error::string("submit_mechanics_review requires a non-empty summary"));
        }

        match output.verdict.as_str() {
            VERDICT_APPROVED | VERDICT_CHANGES_REQUESTED => {}
            other => {
                return Err(Error::string(&format!(
                    "invalid verdict: {other}; use approved or changes_requested"
                )));
            }
        }

        if output.per_worker {
            if output.scout {
                // scout scope
            } else if output
                .crux_key
                .as_deref()
                .is_none_or(str::is_empty)
            {
                return Err(Error::string(
                    "submit_mechanics_review with per_worker requires crux_key or scout true",
                ));
            }
        }

        if output.verdict == VERDICT_CHANGES_REQUESTED && output.findings.is_empty() {
            return Err(Error::string(
                "changes_requested verdict requires at least one finding with remediation guidance",
            ));
        }

        for finding in &output.findings {
            if finding.description.trim().is_empty() {
                return Err(Error::string("findings require non-empty description"));
            }
            if finding.category.trim().is_empty() {
                return Err(Error::string("findings require non-empty category"));
            }
            if finding.severity.trim().is_empty() {
                return Err(Error::string("findings require non-empty severity"));
            }
            if output.verdict == VERDICT_CHANGES_REQUESTED
                && finding.remediation.as_deref().is_none_or(str::is_empty)
            {
                return Err(Error::string(
                    "changes_requested findings require non-empty remediation guidance",
                ));
            }
        }

        Ok(())
    }

    pub async fn validate_with_workspace(
        db: &sea_orm::DatabaseConnection,
        output: &MechanicsReviewOutput,
        review_round: i64,
    ) -> Result<()> {
        Self::validate_output_shape(output)?;
        let scope = review_scope_from_output(output)?;
        let draft_scope = scope.to_draft_scope();
        let store = FinancialAnalysisStore::new(db);
        let promoted_keys = store
            .load_promoted_experiment_keys_for_scope(draft_scope)
            .await?;

        if promoted_keys.is_empty() {
            return Err(Error::string(&format!(
                "no promoted experiments found for review scope {}",
                scope.review_scope_key()
            )));
        }

        let reviewed: HashSet<&str> = output
            .experiments_reviewed
            .iter()
            .map(String::as_str)
            .collect();
        let missing: Vec<&str> = promoted_keys
            .iter()
            .map(String::as_str)
            .filter(|key| !reviewed.contains(key))
            .collect();
        if !missing.is_empty() {
            return Err(Error::string(&format!(
                "submit_mechanics_review must list every promoted experiment in scope; missing: {}",
                missing.join(", ")
            )));
        }

        let blocking = Self::deterministic_blocking_findings(db, scope).await?;
        if output.verdict == VERDICT_APPROVED && !blocking.is_empty() {
            let lines: Vec<String> = blocking
                .iter()
                .map(|finding| format!("- [{}] {}", finding.category, finding.description))
                .collect();
            return Err(Error::string(&format!(
                "cannot stamp approved while deterministic blocking issues remain:\n{}",
                lines.join("\n")
            )));
        }

        if output.verdict == VERDICT_CHANGES_REQUESTED {
            let has_blocking = output
                .findings
                .iter()
                .any(|finding| finding.severity == "blocking");
            if !has_blocking && blocking.is_empty() {
                return Err(Error::string(
                    "changes_requested requires at least one blocking finding or deterministic issue",
                ));
            }
        }

        let _ = review_round;
        Ok(())
    }

    pub async fn deterministic_blocking_findings(
        db: &sea_orm::DatabaseConnection,
        scope: MechanicsReviewScope<'_>,
    ) -> Result<Vec<MechanicsReviewFinding>> {
        let store = FinancialAnalysisStore::new(db);
        let mut findings = Vec::new();

        let draft_scope = scope.to_draft_scope();
        let blocking_drafts = store.load_blocking_draft_runs(draft_scope).await?;
        for draft in blocking_drafts {
            findings.push(MechanicsReviewFinding {
                category: "orphan_draft".to_string(),
                severity: "blocking".to_string(),
                description: format!(
                    "Unfinalized analysis draft {} ({}) remains in scope",
                    draft.run_key, draft.execution_status
                ),
                experiment_key: None,
                remediation: Some(
                    "Finalize, reject, or discard each orphan draft before approval.".to_string(),
                ),
            });
        }

        let promoted = store
            .load_promoted_experiments_for_scope(scope.to_draft_scope())
            .await?;
        for experiment in promoted {
            if !outputs_include_arithmetic_and_interpretation(&experiment.outputs_json) {
                findings.push(MechanicsReviewFinding {
                    category: "output_shape".to_string(),
                    severity: "blocking".to_string(),
                    description: format!(
                        "Promoted experiment {} lacks arithmetic/ratio and interpretation outputs",
                        experiment.experiment_key
                    ),
                    experiment_key: Some(experiment.experiment_key.clone()),
                    remediation: Some(
                        "Re-run finalize_analysis with both arithmetic and interpretation outputs."
                            .to_string(),
                    ),
                });
            }
        }

        Ok(findings)
    }

    pub async fn persist_review(
        db: &sea_orm::DatabaseConnection,
        output: &MechanicsReviewOutput,
        review_round: i64,
        worker_run_id: Option<&str>,
        created_at: &str,
    ) -> Result<()> {
        let scope = review_scope_from_output(output)?;
        let findings_json = serde_json::to_string(&output.findings)
            .map_err(|err| Error::string(&format!("serialize findings: {err}")))?;
        let experiments_json = serde_json::to_string(&output.experiments_reviewed)
            .map_err(|err| Error::string(&format!("serialize experiments_reviewed: {err}")))?;

        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "INSERT INTO mechanics_reviews (
                    review_scope_key, scope_type, crux_key, verdict, summary,
                    findings_json, experiments_reviewed_json, review_round,
                    worker_run_id, created_at
                ) VALUES (
                    {scope_key}, {scope_type}, {crux_key}, {verdict}, {summary},
                    {findings}, {experiments}, {round},
                    {worker_run}, {created_at}
                )
                ON CONFLICT(review_scope_key, review_round) DO UPDATE SET
                    scope_type = excluded.scope_type,
                    crux_key = excluded.crux_key,
                    verdict = excluded.verdict,
                    summary = excluded.summary,
                    findings_json = excluded.findings_json,
                    experiments_reviewed_json = excluded.experiments_reviewed_json,
                    worker_run_id = excluded.worker_run_id,
                    created_at = excluded.created_at",
                scope_key = sql_value(Some(scope.review_scope_key().as_str())),
                scope_type = sql_value(Some(scope.scope_type())),
                crux_key = sql_value(scope.crux_key()),
                verdict = sql_value(Some(output.verdict.as_str())),
                summary = sql_value(Some(output.summary.as_str())),
                findings = sql_value(Some(findings_json.as_str())),
                experiments = sql_value(Some(experiments_json.as_str())),
                round = review_round,
                worker_run = sql_value(worker_run_id),
                created_at = sql_value(Some(created_at)),
            ),
        ))
        .await
        .map_err(|err| Error::string(&format!("failed to persist mechanics review: {err}")))?;

        Ok(())
    }

    pub async fn load_latest_reviews(
        db: &sea_orm::DatabaseConnection,
    ) -> Result<Vec<MechanicsReviewRecord>> {
        let rows = query_all(
            db,
            "SELECT review_scope_key, scope_type, crux_key, verdict, summary,
                    findings_json, experiments_reviewed_json, review_round
             FROM mechanics_reviews mr
             WHERE review_round = (
               SELECT MAX(review_round) FROM mechanics_reviews mr2
               WHERE mr2.review_scope_key = mr.review_scope_key
             )
             ORDER BY review_scope_key",
        )
        .await?;

        rows.into_iter()
            .map(|row| {
                let findings_json = row_string(&row, 5)?;
                let experiments_json = row_string(&row, 6)?;
                let findings: Vec<MechanicsReviewFinding> =
                    serde_json::from_str(&findings_json).map_err(|err| {
                        Error::string(&format!("invalid findings_json: {err}"))
                    })?;
                let experiments_reviewed: Vec<String> =
                    serde_json::from_str(&experiments_json).map_err(|err| {
                        Error::string(&format!("invalid experiments_reviewed_json: {err}"))
                    })?;
                Ok(MechanicsReviewRecord {
                    review_scope_key: row_string(&row, 0)?,
                    scope_type: row_string(&row, 1)?,
                    crux_key: row.try_get_by_index::<Option<String>>(2).ok().flatten(),
                    verdict: row_string(&row, 3)?,
                    summary: row_string(&row, 4)?,
                    findings,
                    experiments_reviewed,
                    review_round: row_i64(&row, 7)?,
                })
            })
            .collect()
    }

    pub fn format_remediation_prefix(record: &MechanicsReviewRecord) -> String {
        let mut lines = vec![format!(
            "CHANGES REQUESTED — remediate mechanics for scope `{}` and resubmit for review.\n\
             Prior review summary: {}\n",
            record.review_scope_key, record.summary
        )];
        for finding in &record.findings {
            let experiment = finding
                .experiment_key
                .as_deref()
                .map(|key| format!(" (experiment {key})"))
                .unwrap_or_default();
            lines.push(format!(
                "- [{}] {}{}: {}",
                finding.category,
                finding.description,
                experiment,
                finding.remediation.as_deref().unwrap_or("Fix and re-validate.")
            ));
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MechanicsReviewScope<'a> {
    CruxKey(&'a str),
    Scout,
}

impl<'a> MechanicsReviewScope<'a> {
    pub fn review_scope_key(&self) -> String {
        match self {
            Self::CruxKey(key) => format!("crux:{key}"),
            Self::Scout => "scout".to_string(),
        }
    }

    pub fn scope_type(&self) -> &'static str {
        match self {
            Self::CruxKey(_) => "crux_key",
            Self::Scout => "scout",
        }
    }

    pub fn crux_key(&self) -> Option<&str> {
        match self {
            Self::CruxKey(key) => Some(key),
            Self::Scout => None,
        }
    }

    pub fn to_draft_scope(self) -> MechanicsDraftScope<'a> {
        match self {
            Self::CruxKey(key) => MechanicsDraftScope::CruxKey(key),
            Self::Scout => MechanicsDraftScope::ScoutGaps,
        }
    }
}

pub fn review_scope_from_output(output: &MechanicsReviewOutput) -> Result<MechanicsReviewScope<'_>> {
    if output.scout {
        return Ok(MechanicsReviewScope::Scout);
    }
    let crux_key = output
        .crux_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            Error::string("submit_mechanics_review requires crux_key or scout true")
        })?;
    Ok(MechanicsReviewScope::CruxKey(crux_key))
}

fn outputs_include_arithmetic_and_interpretation(outputs_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(outputs_json) else {
        return false;
    };
    let Some(rows) = value.as_array() else {
        return false;
    };

    let has_arithmetic = rows.iter().any(|row| {
        row.get("kind")
            .and_then(|v| v.as_str())
            .is_some_and(|kind| {
                matches!(
                    kind,
                    "arithmetic" | "ratio" | "series_point" | "bridge_step"
                )
            })
    });
    let has_interpretation = rows.iter().any(|row| {
        row.get("kind").and_then(|v| v.as_str()) == Some("interpretation")
            && row
                .get("text")
                .and_then(|v| v.as_str())
                .is_some_and(|text| !text.trim().is_empty())
    });
    has_arithmetic && has_interpretation
}

async fn query_all(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
) -> Result<Vec<sea_orm::QueryResult>> {
    db.query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))
}

fn row_string(row: &sea_orm::QueryResult, index: usize) -> Result<String> {
    row.try_get_by_index::<String>(index)
        .map_err(|err| Error::string(&format!("expected text column {index}: {err}")))
}

fn row_i64(row: &sea_orm::QueryResult, index: usize) -> Result<i64> {
    row.try_get_by_index::<i64>(index)
        .map_err(|err| Error::string(&format!("expected integer column {index}: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::financial_model_explorer::types::MechanicsReviewOutput;

    #[test]
    fn changes_requested_requires_findings() {
        let output = MechanicsReviewOutput {
            summary: "Needs work".to_string(),
            per_worker: true,
            crux_key: Some("test_crux".to_string()),
            scout: false,
            verdict: VERDICT_CHANGES_REQUESTED.to_string(),
            findings: vec![],
            experiments_reviewed: vec!["exp_a".to_string()],
        };
        let err = MechanicsReviewService::validate_output_shape(&output).expect_err("should fail");
        assert!(err.to_string().contains("at least one finding"));
    }

    #[test]
    fn approved_rejects_empty_summary() {
        let output = MechanicsReviewOutput {
            summary: "   ".to_string(),
            per_worker: true,
            crux_key: Some("test_crux".to_string()),
            scout: false,
            verdict: VERDICT_APPROVED.to_string(),
            findings: vec![],
            experiments_reviewed: vec!["exp_a".to_string()],
        };
        assert!(MechanicsReviewService::validate_output_shape(&output).is_err());
    }
}
