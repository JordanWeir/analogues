use crate::{
    agents::financial_model_explorer::types::{CruxTriageOutput, MechanicsExperimentsComplete},
    services::{
        financial_analysis_store::{AnalysisDraftSummary, FinancialAnalysisStore, MechanicsDraftScope},
        workspace_store::sqlite_uri,
    },
};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

pub const MIN_SUPPORTING_METRICS: usize = 2;
pub const MIN_PROMOTED_EXPERIMENTS: i64 = 2;
pub const MIN_PROMOTED_CRUXES_WHEN_NARRATIVE_RICH: usize = 2;
pub const NARRATIVE_CRUX_COUNT_FOR_STRICT_TRIAGE: i64 = 3;

#[derive(Debug, Clone)]
pub struct ExplorerWorkspaceContext {
    pub narrative_crux_count: i64,
    pub open_gaps: Vec<String>,
    pub narrative_cruxes_summary: String,
    pub sec_freshness_summary: String,
    pub claims_guidance_present: bool,
}

pub async fn load_explorer_context(sqlite_path: &std::path::Path) -> Result<ExplorerWorkspaceContext> {
    let db = sea_orm::Database::connect(sqlite_uri(sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open workspace db: {err}")))?;

    let narrative_crux_count = scalar_i64(
        &db,
        "SELECT COUNT(*) AS count FROM narrative_map_items WHERE item_type = 'crux'",
    )
    .await?;

    let gap_rows = query_all(
        &db,
        "SELECT gap_key, description FROM data_gaps WHERE status = 'open' ORDER BY id",
    )
    .await?;
    let open_gaps: Vec<String> = gap_rows
        .iter()
        .map(|row| row_string(row, 0).unwrap_or_default())
        .collect();

    let crux_rows = query_all(
        &db,
        "SELECT item_order, body FROM narrative_map_items WHERE item_type = 'crux' ORDER BY item_order",
    )
    .await?;
    let narrative_cruxes_summary = if crux_rows.is_empty() {
        "No narrative crux items in narrative_map_items.".to_string()
    } else {
        let mut lines = vec!["Narrative cruxes (map each to a crux_candidate or explain rejection):".to_string()];
        for row in &crux_rows {
            let order = row_i64(row, 0).unwrap_or(0);
            let body = row_string(row, 1).unwrap_or_default();
            let preview = if body.len() > 120 {
                format!("{}…", &body[..120])
            } else {
                body
            };
            lines.push(format!("- #{order}: {preview}"));
        }
        lines.join("\n")
    };

    let sec_rows = query_all(
        &db,
        "SELECT concept_name, MAX(period_end) AS latest_period_end, MAX(filed_at) AS latest_filed_at
         FROM sec_raw_facts
         WHERE concept_name IN (
           'RevenueRemainingPerformanceObligation',
           'PaymentsToAcquirePropertyPlantAndEquipment',
           'NetCashProvidedByUsedInOperatingActivities',
           'RevenueFromContractWithCustomerExcludingAssessedTax'
         )
         GROUP BY concept_name
         ORDER BY concept_name",
    )
    .await?;
    let sec_freshness_summary = if sec_rows.is_empty() {
        "No key SEC facts found.".to_string()
    } else {
        let mut lines = vec![
            "SEC fact freshness (compare to claims before treating as current):".to_string(),
        ];
        for row in &sec_rows {
            let concept = row_string(row, 0).unwrap_or_default();
            let period_end = row_string(row, 1).unwrap_or_else(|_| "n/a".to_string());
            let filed_at = row_string(row, 2).unwrap_or_else(|_| "n/a".to_string());
            lines.push(format!("- {concept}: latest period_end {period_end}, filed {filed_at}"));
        }
        lines.join("\n")
    };

    let claims_guidance_present = scalar_i64(
        &db,
        "SELECT COUNT(*) AS count FROM claims
         WHERE LOWER(claim) LIKE '%guidance%'
            OR LOWER(claim) LIKE '%expects%'
            OR LOWER(claim) LIKE '%outlook%'
            OR LOWER(claim) LIKE '%fy20%'
            OR LOWER(claim) LIKE '%fiscal 20%'",
    )
    .await?
        > 0;

    db.close().await.ok();

    Ok(ExplorerWorkspaceContext {
        narrative_crux_count,
        open_gaps,
        narrative_cruxes_summary,
        sec_freshness_summary,
        claims_guidance_present,
    })
}

pub fn format_explorer_context_section(ctx: &ExplorerWorkspaceContext) -> String {
    let mut sections = vec![
        ctx.narrative_cruxes_summary.clone(),
        ctx.sec_freshness_summary.clone(),
    ];
    if !ctx.open_gaps.is_empty() {
        let mut lines = vec!["Open data_gaps (SEC or fundamentals may lag narrative claims):".to_string()];
        for gap in &ctx.open_gaps {
            lines.push(format!("- {gap}"));
        }
        sections.push(lines.join("\n"));
    }
    if ctx.claims_guidance_present {
        sections.push(
            "Claims include forward guidance — at least one mechanics experiment should use \
             purpose sensitivity or forward_projection, with staleness noted when SEC lags claims."
                .to_string(),
        );
    }
    sections.join("\n\n")
}

pub fn validate_crux_triage_with_context(
    output: &CruxTriageOutput,
    ctx: &ExplorerWorkspaceContext,
) -> Result<()> {
    let promoted_cruxes = output
        .cruxes
        .iter()
        .filter(|crux| crux.disposition == "promoted")
        .count();

    if output.cruxes.len() == 1 {
        if promoted_cruxes == 1 && output.supporting_metrics.is_empty() {
            return Err(Error::string(
                "submit_crux_triage requires at least one supporting_metric when promoting a crux",
            ));
        }
        return Ok(());
    }

    if ctx.narrative_crux_count >= NARRATIVE_CRUX_COUNT_FOR_STRICT_TRIAGE
        && promoted_cruxes < MIN_PROMOTED_CRUXES_WHEN_NARRATIVE_RICH
    {
        return Err(Error::string(&format!(
            "submit_crux_triage requires at least {MIN_PROMOTED_CRUXES_WHEN_NARRATIVE_RICH} promoted cruxes \
             when narrative_map_items has {NARRATIVE_CRUX_COUNT_FOR_STRICT_TRIAGE}+ crux items \
             (have {promoted_cruxes}, narrative count {narrative_crux_count})",
            narrative_crux_count = ctx.narrative_crux_count
        )));
    }

    if promoted_cruxes > 0 && output.supporting_metrics.len() < MIN_SUPPORTING_METRICS {
        return Err(Error::string(&format!(
            "submit_crux_triage requires at least {MIN_SUPPORTING_METRICS} supporting_metrics \
             with rationale when promoting cruxes (have {})",
            output.supporting_metrics.len()
        )));
    }

    Ok(())
}

pub fn validate_mechanics_complete_with_context(
    output: &MechanicsExperimentsComplete,
    promoted_count: i64,
    non_historical_purpose_count: i64,
    ctx: &ExplorerWorkspaceContext,
) -> Result<()> {
    if output.per_worker {
        return Ok(());
    }

    if promoted_count < MIN_PROMOTED_EXPERIMENTS {
        return Err(Error::string(&format!(
            "submit_mechanics_experiments requires at least {MIN_PROMOTED_EXPERIMENTS} promoted \
             analysis_experiments (have {promoted_count})"
        )));
    }

    if ctx.claims_guidance_present && non_historical_purpose_count == 0 {
        return Err(Error::string(
            "claims include forward guidance; promote at least one experiment with purpose \
             sensitivity or forward_projection (not historical_investigation only)",
        ));
    }

    Ok(())
}

pub fn mechanics_draft_scope(output: &MechanicsExperimentsComplete) -> Result<MechanicsDraftScope<'_>> {
    if output.per_worker {
        if output.scout {
            return Ok(MechanicsDraftScope::ScoutGaps);
        }
        let crux_key = output.crux_key.as_deref().filter(|key| !key.trim().is_empty()).ok_or_else(|| {
            Error::string(
                "submit_mechanics_experiments with per_worker requires crux_key for your assigned crux \
                 (or scout true for the scout worker)",
            )
        })?;
        return Ok(MechanicsDraftScope::CruxKey(crux_key));
    }
    Ok(MechanicsDraftScope::Workspace)
}

pub fn format_blocking_drafts_error(drafts: &[AnalysisDraftSummary]) -> String {
    let mut lines = vec![
        "submit_mechanics_experiments rejected: unfinalized analysis drafts remain. \
         Call finalize_analysis (promote, background, or rejected) for each draft before submitting."
            .to_string(),
    ];
    for draft in drafts {
        let crux = draft
            .crux_key
            .as_deref()
            .map(|key| format!(", crux_key={key}"))
            .unwrap_or_default();
        lines.push(format!(
            "- run_key={} ({}){}{}",
            draft.run_key,
            draft.execution_status,
            crux,
            if draft.question.len() > 80 {
                format!(" — {}", &draft.question[..80])
            } else if draft.question.is_empty() {
                String::new()
            } else {
                format!(" — {}", draft.question)
            }
        ));
    }
    lines.join("\n")
}

pub async fn enforce_mechanics_draft_hygiene(
    store: &FinancialAnalysisStore<'_>,
    output: &MechanicsExperimentsComplete,
) -> Result<()> {
    let scope = mechanics_draft_scope(output)?;
    store.discard_error_draft_runs(scope).await?;
    let blocking = store.load_blocking_draft_runs(scope).await?;
    if blocking.is_empty() {
        return Ok(());
    }
    Err(Error::string(&format_blocking_drafts_error(&blocking)))
}

pub fn mechanics_draft_scope_for_prepare(
    focus_crux_key: Option<&str>,
    scout_worker: bool,
) -> MechanicsDraftScope<'_> {
    if scout_worker {
        MechanicsDraftScope::ScoutGaps
    } else if let Some(crux_key) = focus_crux_key.filter(|key| !key.trim().is_empty()) {
        MechanicsDraftScope::CruxKey(crux_key)
    } else {
        MechanicsDraftScope::Workspace
    }
}

pub async fn load_draft_summaries_for_prepare(
    sqlite_path: &std::path::Path,
    focus_crux_key: Option<&str>,
    scout_worker: bool,
) -> Vec<AnalysisDraftSummary> {
    let db = match sea_orm::Database::connect(sqlite_uri(sqlite_path)).await {
        Ok(db) => db,
        Err(_) => return vec![],
    };
    let store = FinancialAnalysisStore::new(&db);
    let scope = mechanics_draft_scope_for_prepare(focus_crux_key, scout_worker);
    let drafts = store.load_draft_runs(scope).await.unwrap_or_default();
    db.close().await.ok();
    drafts
}

pub fn format_draft_hygiene_prepare_message(drafts: &[AnalysisDraftSummary]) -> String {
    let mut lines = vec![
        "[Draft hygiene] Unfinalized analysis drafts remain — call finalize_analysis before submit_mechanics_experiments:"
            .to_string(),
    ];
    for draft in drafts {
        lines.push(format!(
            "- {} ({})",
            draft.run_key, draft.execution_status
        ));
    }
    lines.join("\n")
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
    use crate::agents::financial_model_explorer::types::CruxCandidateInput;

    fn sample_crux() -> CruxCandidateInput {
        CruxCandidateInput {
            crux_key: "test_crux".to_string(),
            title: "Test".to_string(),
            statement: "Statement".to_string(),
            bridge_archetype: None,
            narrative_side: None,
            watch_condition: "Watch".to_string(),
            confirming_signal: "Confirm".to_string(),
            breaking_signal: "Break".to_string(),
            disposition: "promoted".to_string(),
            rationale: "Because".to_string(),
            limitations: None,
            cluster_members: vec![],
            linked_claim_ids: vec![],
        }
    }

    #[test]
    fn requires_two_cruxes_when_narrative_is_rich() {
        let mut background = sample_crux();
        background.crux_key = "background_crux".to_string();
        background.disposition = "background".to_string();
        let output = CruxTriageOutput {
            cruxes: vec![sample_crux(), background],
            supporting_metrics: vec![],
            quality_flags: vec![],
            open_questions: vec![],
        };
        let ctx = ExplorerWorkspaceContext {
            narrative_crux_count: 5,
            open_gaps: vec![],
            narrative_cruxes_summary: String::new(),
            sec_freshness_summary: String::new(),
            claims_guidance_present: false,
        };
        let err = validate_crux_triage_with_context(&output, &ctx).expect_err("should fail");
        assert!(err.to_string().contains("at least 2 promoted cruxes"));
    }

    #[test]
    fn single_crux_focus_requires_supporting_metric() {
        let output = CruxTriageOutput {
            cruxes: vec![sample_crux()],
            supporting_metrics: vec![],
            quality_flags: vec![],
            open_questions: vec![],
        };
        let ctx = ExplorerWorkspaceContext {
            narrative_crux_count: 5,
            open_gaps: vec![],
            narrative_cruxes_summary: String::new(),
            sec_freshness_summary: String::new(),
            claims_guidance_present: false,
        };
        let err = validate_crux_triage_with_context(&output, &ctx).expect_err("should fail");
        assert!(err.to_string().contains("supporting_metric"));
    }

    #[test]
    fn per_worker_mechanics_submit_skips_lane_minimums() {
        use crate::agents::financial_model_explorer::types::MechanicsExperimentsComplete;

        let output = MechanicsExperimentsComplete {
            summary: String::new(),
            per_worker: true,
            crux_key: Some("test_crux".to_string()),
            scout: false,
        };
        let ctx = ExplorerWorkspaceContext {
            narrative_crux_count: 5,
            open_gaps: vec![],
            narrative_cruxes_summary: String::new(),
            sec_freshness_summary: String::new(),
            claims_guidance_present: true,
        };
        validate_mechanics_complete_with_context(&output, 0, 0, &ctx).expect("per_worker skips");
    }

    #[test]
    fn per_worker_submit_requires_crux_key_or_scout() {
        use crate::agents::financial_model_explorer::types::MechanicsExperimentsComplete;

        let output = MechanicsExperimentsComplete {
            summary: String::new(),
            per_worker: true,
            crux_key: None,
            scout: false,
        };
        let err = mechanics_draft_scope(&output).expect_err("should require crux_key");
        assert!(err.to_string().contains("crux_key"));

        let scout = MechanicsExperimentsComplete {
            scout: true,
            ..output
        };
        assert_eq!(mechanics_draft_scope(&scout).unwrap(), MechanicsDraftScope::ScoutGaps);
    }
}
