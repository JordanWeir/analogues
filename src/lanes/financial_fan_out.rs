use super::{
    context::LaneContext,
    financial_mechanics_experiments::gate::financial_mechanics_experiments_gates,
    gate::Gate,
    identify_crux_candidates::gate::identify_crux_candidates_gates,
    lane::Lane,
    result::{LaneResult, LaneStatus, LaneWritesSummary},
};
use crate::{
    agents::{
        financial_mechanics_validator::{
            FinancialMechanicsValidatorAgent, FinancialMechanicsValidatorConfig,
        },
        financial_model_explorer::{
            explorer_context::MIN_PROMOTED_EXPERIMENTS, FinancialModelExplorerAgent,
            FinancialModelExplorerConfig,
        },
    },
    services::{
        financial_analysis_store::FinancialAnalysisStore,
        mechanics_review::{
            MechanicsReviewRecord, MechanicsReviewService, MAX_MECHANICS_REMEDIATION_ROUNDS,
            VERDICT_APPROVED, VERDICT_CHANGES_REQUESTED,
        },
        workspace_sql::scalar_i64,
    },
};
use async_trait::async_trait;
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::sync::Arc;

const SCOUT_PREFIX: &str = "SCOUT: Find mechanics the narrative board did not model but SEC data \
supports (dilution, issuance, obligation stacks). Submit at most 2 promoted cruxes. Skip work \
already covered by per-crux agents.\n\n";

const MECHANICS_SCOUT_PREFIX: &str = "SCOUT: Run mechanics experiments for promoted crux_candidates \
that still lack a promoted analysis_experiment. Cover scout triage cruxes (dilution, issuance, \
obligation stacks). Prefer sensitivity or forward_projection when claims include guidance. \
Finish with submit_mechanics_experiments, per_worker true, and scout true.\n\n";

const VALIDATOR_CRUX_PREFIX: &str = "FOCUS: Review promoted mechanics experiments for crux `{crux_key}` only. \
Re-run SQL, verify arithmetic against sec_raw_facts and claims, and check SEC staleness notes. \
Stamp approved or changes_requested via submit_mechanics_review with per_worker true and crux_key \
\"{crux_key}\".\n\n";

const VALIDATOR_SCOUT_PREFIX: &str = "FOCUS: Review scout mechanics experiments for promoted cruxes \
that still lacked dedicated experiment coverage. Stamp approved or changes_requested via \
submit_mechanics_review with per_worker true and scout true.\n\n";

pub struct FinancialFanOutLane {
    #[cfg(test)]
    fixture: bool,
}

impl FinancialFanOutLane {
    pub fn new() -> Self {
        Self {
            #[cfg(test)]
            fixture: false,
        }
    }
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self { fixture: true }
    }
}

impl Default for FinancialFanOutLane {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Lane for FinancialFanOutLane {
    fn name(&self) -> &'static str {
        "financial_fan_out"
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        let mut gates = identify_crux_candidates_gates();
        gates.extend(financial_mechanics_experiments_gates());
        gates.extend(super::financial_mechanics_experiments::gate::mechanics_reviews_approved_gates());
        gates
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        let db = ctx.workspace.connection();
        let store = FinancialAnalysisStore::new(db);
        if scalar_i64(db, "SELECT COUNT(*) AS count FROM concept_catalog_entries").await? == 0 {
            return Ok(LaneResult::skipped(self.name(), "concept catalog empty"));
        }
        if !store.narrative_context_present().await? {
            return Ok(LaneResult::skipped(self.name(), "narrative context empty"));
        }

        #[cfg(test)]
        if self.fixture {
            super::identify_crux_candidates::fixtures::persist_fixture_cruxes(ctx).await?;
            super::financial_mechanics_experiments::fixtures::persist_fixture_experiment(ctx)
                .await?;
            return Ok(ok(self.name()));
        }

        let sqlite = ctx.workspace.paths.sqlite_path.clone();
        let ticker = ctx.ticker().to_string();
        let model = FinancialModelExplorerConfig::crux_triage().model;
        let mut tasks: Vec<(String, tokio::task::JoinHandle<Result<(String, Option<i64>)>>)> =
            Vec::new();

        for (order, body) in load_cruxes(db).await? {
            let (sqlite, ticker) = (sqlite.clone(), ticker.clone());
            let label = format!("crux_{order}");
            let prefix = format!(
                "FOCUS: Analyze only narrative crux #{order}:\n{body}\n\n\
                 Submit exactly one crux_candidate with at least one supporting_metric.\n\n"
            );
            let handle = tokio::spawn(async move {
                FinancialModelExplorerAgent::new(
                    FinancialModelExplorerConfig::crux_triage().with_prompt_prefix(prefix),
                )
                .with_company_label(&ticker)
                .run(sqlite, &ticker)
                .await
            });
            tasks.push((label, handle));
        }

        let (scout_sqlite, scout_ticker) = (sqlite.clone(), ticker.clone());
        tasks.push((
            "scout".to_string(),
            tokio::spawn(async move {
                FinancialModelExplorerAgent::new(
                    FinancialModelExplorerConfig::crux_triage().with_prompt_prefix(SCOUT_PREFIX),
                )
                .with_company_label(&scout_ticker)
                .run(scout_sqlite, &scout_ticker)
                .await
            }),
        ));

        let mut persisted = 0usize;
        let mut failed = 0usize;
        for (label, task) in tasks {
            match task.await {
                Ok(Ok((text, run_id))) => match FinancialModelExplorerAgent::parse_crux_triage_output(&text) {
                    Ok(output) => {
                        if let Err(err) = FinancialModelExplorerAgent::persist_crux_triage(
                            db,
                            &output,
                            &model,
                            run_id,
                        )
                        .await
                        {
                            failed += 1;
                            tracing::warn!(
                                worker = %label,
                                error = %err,
                                "financial_fan_out worker persist failed"
                            );
                        } else {
                            persisted += 1;
                        }
                    }
                    Err(err) => {
                        failed += 1;
                        tracing::warn!(
                            worker = %label,
                            error = %err,
                            "financial_fan_out worker returned invalid crux triage output"
                        );
                    }
                },
                Ok(Err(err)) => {
                    failed += 1;
                    tracing::warn!(
                        worker = %label,
                        error = %err,
                        "financial_fan_out worker agent run failed"
                    );
                }
                Err(err) => {
                    failed += 1;
                    tracing::warn!(
                        worker = %label,
                        error = %err,
                        "financial_fan_out worker task join failed"
                    );
                }
            }
        }

        if persisted == 0 {
            return Err(Error::string(&format!(
                "all {failed} fan-out workers failed; no crux triage results persisted"
            )));
        }
        if failed > 0 {
            tracing::warn!(
                persisted,
                failed,
                "financial_fan_out triage phase completed with worker failures"
            );
        }

        let promoted_cruxes = load_promoted_cruxes(db).await?;
        if promoted_cruxes.is_empty() {
            return Err(Error::string(
                "no promoted crux_candidates after triage fan-out",
            ));
        }

        let mut mech_tasks: Vec<(String, tokio::task::JoinHandle<Result<(String, Option<i64>)>>)> =
            Vec::new();
        for (crux_key, title, statement) in &promoted_cruxes {
            let (sqlite, ticker) = (sqlite.clone(), ticker.clone());
            let label = format!("mech_{crux_key}");
            let prefix = format!(
                "FOCUS: Run 1–2 mechanics experiments for promoted crux `{crux_key}` only.\n\
                 Title: {title}\nStatement: {statement}\n\n\
                 Use run_analysis_draft and finalize_analysis; set crux_key on each experiment. \
                 Prefer sensitivity or forward_projection when this crux involves guidance, funding \
                 gaps, or SEC staleness. Finish with submit_mechanics_experiments, per_worker true, \
                 and crux_key \"{crux_key}\".\n\n"
            );
            let focus_crux_key = crux_key.clone();
            mech_tasks.push((
                label,
                tokio::spawn(async move {
                    FinancialModelExplorerAgent::new(
                        FinancialModelExplorerConfig::mechanics_experiment()
                            .with_prompt_prefix(prefix)
                            .with_focus_crux_key(focus_crux_key),
                    )
                    .with_company_label(&ticker)
                    .run(sqlite, &ticker)
                    .await
                }),
            ));
        }

        let (mech_scout_sqlite, mech_scout_ticker) = (sqlite.clone(), ticker.clone());
        mech_tasks.push((
            "mech_scout".to_string(),
            tokio::spawn(async move {
                FinancialModelExplorerAgent::new(
                    FinancialModelExplorerConfig::mechanics_experiment()
                        .with_prompt_prefix(MECHANICS_SCOUT_PREFIX)
                        .with_scout_worker(),
                )
                .with_company_label(&mech_scout_ticker)
                .run(mech_scout_sqlite, &mech_scout_ticker)
                .await
            }),
        ));

        let mut mech_ok = 0usize;
        let mut mech_failed = 0usize;
        for (label, task) in mech_tasks {
            match task.await {
                Ok(Ok(_)) => mech_ok += 1,
                Ok(Err(err)) => {
                    mech_failed += 1;
                    tracing::warn!(
                        worker = %label,
                        error = %err,
                        "financial_fan_out mechanics worker failed"
                    );
                }
                Err(err) => {
                    mech_failed += 1;
                    tracing::warn!(
                        worker = %label,
                        error = %err,
                        "financial_fan_out mechanics task join failed"
                    );
                }
            }
        }

        if mech_ok == 0 {
            return Err(Error::string(&format!(
                "all {mech_failed} mechanics fan-out workers failed"
            )));
        }
        if mech_failed > 0 {
            tracing::warn!(
                mech_ok,
                mech_failed,
                "financial_fan_out mechanics phase completed with worker failures"
            );
        }

        #[cfg(test)]
        if self.fixture {
            return Ok(ok(self.name()));
        }

        let review_round = 1i64;
        let mut reviews = run_validation_fan_out(
            db,
            &sqlite,
            &ticker,
            &promoted_cruxes,
            &store,
            review_round,
        )
        .await?;

        let mut remediation_round = 0usize;
        while remediation_round < MAX_MECHANICS_REMEDIATION_ROUNDS {
            let changes_requested: Vec<MechanicsReviewRecord> = reviews
                .iter()
                .filter(|review| review.verdict == VERDICT_CHANGES_REQUESTED)
                .cloned()
                .collect();
            if changes_requested.is_empty() {
                break;
            }

            remediation_round += 1;
            tracing::info!(
                remediation_round,
                scopes = changes_requested.len(),
                "financial_fan_out running mechanics remediation for changes_requested reviews"
            );
            run_mechanics_remediation(
                &sqlite,
                &ticker,
                &promoted_cruxes,
                &changes_requested,
            )
            .await?;

            let next_review_round = review_round + remediation_round as i64;
            reviews = run_validation_fan_out(
                db,
                &sqlite,
                &ticker,
                &promoted_cruxes,
                &store,
                next_review_round,
            )
            .await?;
        }

        if reviews
            .iter()
            .any(|review| review.verdict == VERDICT_CHANGES_REQUESTED)
        {
            let scopes: Vec<String> = reviews
                .iter()
                .filter(|review| review.verdict == VERDICT_CHANGES_REQUESTED)
                .map(|review| review.review_scope_key.clone())
                .collect();
            return Err(Error::string(&format!(
                "mechanics validation still has changes_requested after remediation: {}",
                scopes.join(", ")
            )));
        }

        if reviews.iter().all(|review| review.verdict != VERDICT_APPROVED) {
            return Err(Error::string(
                "no approved mechanics reviews were persisted after validation fan-out",
            ));
        }

        if store.count_promoted_experiments().await? < MIN_PROMOTED_EXPERIMENTS {
            return Err(Error::string(&format!(
                "need at least {MIN_PROMOTED_EXPERIMENTS} promoted experiments"
            )));
        }
        Ok(ok(self.name()))
    }
}

async fn run_validation_fan_out(
    db: &sea_orm::DatabaseConnection,
    sqlite: &std::path::Path,
    ticker: &str,
    promoted_cruxes: &[(String, String, String)],
    store: &FinancialAnalysisStore<'_>,
    review_round: i64,
) -> Result<Vec<MechanicsReviewRecord>> {
    let created_at = Utc::now().to_rfc3339();
    let mut tasks: Vec<(String, tokio::task::JoinHandle<Result<(String, Option<i64>)>>)> =
        Vec::new();

    for (crux_key, title, statement) in promoted_cruxes {
        let keys = store
            .load_promoted_experiment_keys_for_scope(
                crate::services::financial_analysis_store::MechanicsDraftScope::CruxKey(crux_key),
            )
            .await?;
        if keys.is_empty() {
            continue;
        }

        let (sqlite, ticker) = (sqlite.to_path_buf(), ticker.to_string());
        let label = format!("review_{crux_key}");
        let prefix = VALIDATOR_CRUX_PREFIX
            .replace("{crux_key}", crux_key)
            .replace("{title}", title)
            .replace("{statement}", statement);
        let focus_crux_key = crux_key.clone();
        tasks.push((
            label,
            tokio::spawn(async move {
                FinancialMechanicsValidatorAgent::new(
                    FinancialMechanicsValidatorConfig::default()
                        .with_prompt_prefix(prefix)
                        .with_focus_crux_key(focus_crux_key)
                        .with_review_round(review_round),
                )
                .with_company_label(&ticker)
                .run(sqlite, &ticker)
                .await
            }),
        ));
    }

    if store.scout_review_scope_has_experiments().await? {
        let (sqlite, ticker) = (sqlite.to_path_buf(), ticker.to_string());
        tasks.push((
            "review_scout".to_string(),
            tokio::spawn(async move {
                FinancialMechanicsValidatorAgent::new(
                    FinancialMechanicsValidatorConfig::default()
                        .with_prompt_prefix(VALIDATOR_SCOUT_PREFIX)
                        .with_scout_worker()
                        .with_review_round(review_round),
                )
                .with_company_label(&ticker)
                .run(sqlite, &ticker)
                .await
            }),
        ));
    }

    if tasks.is_empty() {
        return Err(Error::string(
            "validation fan-out found no promoted experiment scopes to review",
        ));
    }

    let mut persisted = 0usize;
    let mut failed = 0usize;
    for (label, task) in tasks {
        match task.await {
            Ok(Ok((text, run_id))) => {
                match FinancialMechanicsValidatorAgent::parse_review_output(&text) {
                    Ok(output) => {
                        if let Err(err) = MechanicsReviewService::persist_review(
                            db,
                            &output,
                            review_round,
                            run_id.map(|id| id.to_string()).as_deref(),
                            &created_at,
                        )
                        .await
                        {
                            failed += 1;
                            tracing::warn!(
                                worker = %label,
                                error = %err,
                                "financial_fan_out review persist failed"
                            );
                        } else {
                            persisted += 1;
                        }
                    }
                    Err(err) => {
                        failed += 1;
                        tracing::warn!(
                            worker = %label,
                            error = %err,
                            "financial_fan_out review returned invalid output"
                        );
                    }
                }
            }
            Ok(Err(err)) => {
                failed += 1;
                tracing::warn!(
                    worker = %label,
                    error = %err,
                    "financial_fan_out review worker failed"
                );
            }
            Err(err) => {
                failed += 1;
                tracing::warn!(
                    worker = %label,
                    error = %err,
                    "financial_fan_out review task join failed"
                );
            }
        }
    }

    if persisted == 0 {
        return Err(Error::string(&format!(
            "all {failed} mechanics validation workers failed; no reviews persisted"
        )));
    }
    if failed > 0 {
        tracing::warn!(
            persisted,
            failed,
            "financial_fan_out validation phase completed with worker failures"
        );
    }

    MechanicsReviewService::load_latest_reviews(db).await
}

async fn run_mechanics_remediation(
    sqlite: &std::path::Path,
    ticker: &str,
    promoted_cruxes: &[(String, String, String)],
    changes_requested: &[MechanicsReviewRecord],
) -> Result<()> {
    let mut tasks: Vec<(String, tokio::task::JoinHandle<Result<(String, Option<i64>)>>)> =
        Vec::new();

    for review in changes_requested {
        let remediation_prefix = MechanicsReviewService::format_remediation_prefix(review);
        if review.review_scope_key == "scout" {
            let (sqlite, ticker) = (sqlite.to_path_buf(), ticker.to_string());
            tasks.push((
                "mech_scout_remediate".to_string(),
                tokio::spawn(async move {
                    FinancialModelExplorerAgent::new(
                        FinancialModelExplorerConfig::mechanics_experiment()
                            .with_prompt_prefix(format!(
                                "{remediation_prefix}{MECHANICS_SCOUT_PREFIX}"
                            ))
                            .with_scout_worker(),
                    )
                    .with_company_label(&ticker)
                    .run(sqlite, &ticker)
                    .await
                }),
            ));
            continue;
        }

        let crux_key = review
            .crux_key
            .clone()
            .or_else(|| review.review_scope_key.strip_prefix("crux:").map(str::to_string))
            .ok_or_else(|| {
                Error::string(&format!(
                    "cannot remediate review scope {}",
                    review.review_scope_key
                ))
            })?;
        let (_, title, statement) = promoted_cruxes
            .iter()
            .find(|(key, _, _)| key == &crux_key)
            .ok_or_else(|| Error::string(&format!("unknown crux_key for remediation: {crux_key}")))?;
        let (sqlite, ticker) = (sqlite.to_path_buf(), ticker.to_string());
        let label = format!("mech_remediate_{crux_key}");
        let prefix = format!(
            "{remediation_prefix}FOCUS: Run 1–2 mechanics experiments for promoted crux `{crux_key}` only.\n\
             Title: {title}\nStatement: {statement}\n\n\
             Use run_analysis_draft and finalize_analysis; set crux_key on each experiment. \
             Prefer sensitivity or forward_projection when this crux involves guidance, funding \
             gaps, or SEC staleness. Finish with submit_mechanics_experiments, per_worker true, \
             and crux_key \"{crux_key}\".\n\n"
        );
        let focus_crux_key = crux_key.clone();
        tasks.push((
            label,
            tokio::spawn(async move {
                FinancialModelExplorerAgent::new(
                    FinancialModelExplorerConfig::mechanics_experiment()
                        .with_prompt_prefix(prefix)
                        .with_focus_crux_key(focus_crux_key),
                )
                .with_company_label(&ticker)
                .run(sqlite, &ticker)
                .await
            }),
        ));
    }

    let mut ok_count = 0usize;
    let mut failed = 0usize;
    for (label, task) in tasks {
        match task.await {
            Ok(Ok(_)) => ok_count += 1,
            Ok(Err(err)) => {
                failed += 1;
                tracing::warn!(
                    worker = %label,
                    error = %err,
                    "financial_fan_out remediation worker failed"
                );
            }
            Err(err) => {
                failed += 1;
                tracing::warn!(
                    worker = %label,
                    error = %err,
                    "financial_fan_out remediation task join failed"
                );
            }
        }
    }

    if ok_count == 0 {
        return Err(Error::string(&format!(
            "all {failed} mechanics remediation workers failed"
        )));
    }
    Ok(())
}

async fn load_cruxes(db: &sea_orm::DatabaseConnection) -> Result<Vec<(i64, String)>> {
    db.query_all(Statement::from_string(
        DatabaseBackend::Sqlite,
        "SELECT item_order, body FROM narrative_map_items WHERE item_type = 'crux' ORDER BY item_order"
            .to_string(),
    ))
    .await
    .map_err(|e| Error::string(&format!("load cruxes: {e}")))?
    .into_iter()
    .map(|row| {
        Ok((
            row.try_get_by_index(0).map_err(|e| Error::string(&e.to_string()))?,
            row.try_get_by_index(1).map_err(|e| Error::string(&e.to_string()))?,
        ))
    })
    .collect()
}

async fn load_promoted_cruxes(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<(String, String, String)>> {
    db.query_all(Statement::from_string(
        DatabaseBackend::Sqlite,
        "SELECT crux_key, title, statement FROM crux_candidates
         WHERE disposition = 'promoted' AND status = 'active'
         ORDER BY id"
            .to_string(),
    ))
    .await
    .map_err(|e| Error::string(&format!("load promoted cruxes: {e}")))?
    .into_iter()
    .map(|row| {
        Ok((
            row.try_get_by_index(0).map_err(|e| Error::string(&e.to_string()))?,
            row.try_get_by_index(1).map_err(|e| Error::string(&e.to_string()))?,
            row.try_get_by_index(2).map_err(|e| Error::string(&e.to_string()))?,
        ))
    })
    .collect()
}

fn ok(name: &str) -> LaneResult {
    LaneResult {
        lane_name: name.to_string(),
        status: LaneStatus::Success,
        writes: LaneWritesSummary::default()
            .read("narrative_map_items")
            .wrote("crux_candidates")
            .wrote("analysis_experiments")
            .wrote("mechanics_reviews"),
        gate_results: Vec::new(),
        error_message: None,
    }
}
