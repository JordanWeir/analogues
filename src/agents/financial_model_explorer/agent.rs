use super::{
    config::FinancialModelExplorerConfig,
    explorer_context::{
        format_explorer_context_section, load_explorer_context, validate_crux_triage_with_context,
        validate_mechanics_complete_with_context,
    },
    golden_path::{
        crux_triage_golden_path, crux_triage_submit_example, explorer_schema_hint,
        mechanics_experiment_golden_path, mechanics_finalize_example,
        mechanics_per_worker_submit_example, mechanics_scout_submit_example,
    },
    prepare_step::mechanics_prepare_step_chain,
    types::{
        AnalysisExperimentInput, CruxTriageOutput, ExplorerMode, MechanicsExperimentsComplete,
    },
    WORKER_NAME,
};
use crate::{
    agents::{
        tool_loop_agent::{ToolLoopAgent, ToolLoopRequest},
        tools::ToolRegistry,
    },
    services::{
        financial_analysis_store::{outputs_include_arithmetic_and_interpretation, FinancialAnalysisStore},
        model_client::extract_json_blob,
    },
};
use chrono::Utc;
use loco_rs::prelude::*;
use std::{collections::BTreeMap, path::PathBuf};

pub const CRUX_TRIAGE_PREAMBLE: &str = "You are the Financial Model Explorer in crux-triage mode. Connect SEC concept catalogs to the narrative map and identify falsifiable crux candidates — one per narrative crux or a justified cluster. Do not collapse the full narrative board into a single crux. Use workspace_sql following the golden path. Search concept_catalog_entries before sec_raw_facts. Persist at least two supporting_metrics with rationale from catalog search. When SEC facts lag claims or data_gaps, record quality_flags. When finished, call submit_crux_triage — do not end with a plain assistant message. Fix validation errors and resubmit. You have a limited step budget; after each tool round the user message reports steps remaining. On the penultimate turn, call submit_crux_triage so the last turn can fix validation gaps.";

pub const MECHANICS_EXPERIMENT_PREAMBLE: &str = "You are the Financial Model Explorer in mechanics-experiment mode. Test how crux mechanics affect revenue, margins, cash flow, and funding pressure using focused SQLite calculations. Use run_analysis_draft to execute SQL and inspect results before judging them. Use finalize_analysis to promote, reject, or background an experiment based on the draft results. Separate arithmetic outputs from interpretation outputs. Promote at least two experiments across distinct questions; include sensitivity or forward_projection when claims carry guidance. Note SEC staleness when facts lag narrative claims. On the penultimate turn, finalize any pending drafts — call submit_mechanics_experiments only on the final turn when validation requirements are met. Fix validation errors and resubmit. You have a limited step budget; after each tool round the user message reports steps remaining.";

#[derive(Debug, Clone)]
pub struct FinancialModelExplorerAgent {
    config: FinancialModelExplorerConfig,
    company_label: Option<String>,
}

impl FinancialModelExplorerAgent {
    pub fn new(config: FinancialModelExplorerConfig) -> Self {
        Self {
            config,
            company_label: None,
        }
    }

    pub fn with_company_label(mut self, label: impl Into<String>) -> Self {
        self.company_label = Some(label.into());
        self
    }

    pub fn config(&self) -> &FinancialModelExplorerConfig {
        &self.config
    }

    pub async fn run(&self, workspace_sqlite: PathBuf, ticker: &str) -> Result<(String, Option<i64>)> {
        let tools = self.build_tool_registry(&workspace_sqlite);
        let prepare_step = match self.config.mode {
            ExplorerMode::CruxTriage => None,
            ExplorerMode::MechanicsExperiment => Some(mechanics_prepare_step_chain(
                workspace_sqlite.clone(),
                self.config.focus_crux_key.clone(),
                self.config.scout_worker,
            )),
        };

        let response = ToolLoopAgent::default()
            .run(ToolLoopRequest {
                worker_name: WORKER_NAME.to_string(),
                model: self.config.model.clone(),
                preamble: self.preamble().to_string(),
                prompt: self.agent_prompt(&workspace_sqlite).await?,
                json_mode: false,
                tools,
                metadata: BTreeMap::from([
                    ("lane".to_string(), self.config.mode.worker_lane().to_string()),
                    ("ticker".to_string(), ticker.to_string()),
                    ("mode".to_string(), self.config.mode.mode_label().to_string()),
                ]),
                workspace_sqlite: Some(workspace_sqlite),
                client_tools: None,
                max_agent_rounds: Some(self.config.max_agent_rounds),
                submit_tool_name: Some(self.config.mode.submit_tool_name().to_string()),
                prepare_step,
                stop_when: None,
            })
            .await?;

        Ok((response.text, response.worker_run_id))
    }

    pub fn parse_crux_triage_output(text: &str) -> Result<CruxTriageOutput> {
        let json_text = extract_json_blob(text).ok_or_else(|| {
            Error::string(
                "crux triage response did not contain JSON; call submit_crux_triage with valid output",
            )
        })?;
        let output: CruxTriageOutput = serde_json::from_str(json_text).map_err(|err| {
            Error::string(&format!("invalid crux triage JSON: {err}"))
        })?;
        Self::validate_crux_triage_output(&output)?;
        Ok(output)
    }

    pub fn validate_crux_triage_output(output: &CruxTriageOutput) -> Result<()> {
        if output.cruxes.is_empty() {
            return Err(Error::string("submit_crux_triage requires at least one crux"));
        }

        let mut seen = std::collections::HashSet::new();
        for crux in &output.cruxes {
            if crux.crux_key.trim().is_empty() {
                return Err(Error::string("crux_key cannot be empty"));
            }
            if !seen.insert(crux.crux_key.clone()) {
                return Err(Error::string(&format!(
                    "duplicate crux_key: {}",
                    crux.crux_key
                )));
            }
            Self::validate_disposition(&crux.disposition, "crux")?;
            for field in [
                ("statement", crux.statement.as_str()),
                ("watch_condition", crux.watch_condition.as_str()),
                ("confirming_signal", crux.confirming_signal.as_str()),
                ("breaking_signal", crux.breaking_signal.as_str()),
                ("rationale", crux.rationale.as_str()),
            ] {
                if field.1.trim().is_empty() {
                    return Err(Error::string(&format!(
                        "{} requires non-empty {}",
                        crux.crux_key, field.0
                    )));
                }
            }
        }

        for metric in &output.supporting_metrics {
            if metric.rationale.trim().is_empty() {
                return Err(Error::string(
                    "supporting_metrics entries require non-empty rationale",
                ));
            }
        }

        Ok(())
    }

    pub fn validate_mechanics_complete(
        output: &MechanicsExperimentsComplete,
        promoted_count: i64,
        non_historical_purpose_count: i64,
        ctx: &super::explorer_context::ExplorerWorkspaceContext,
    ) -> Result<()> {
        validate_mechanics_complete_with_context(
            output,
            promoted_count,
            non_historical_purpose_count,
            ctx,
        )
    }

    pub fn validate_crux_triage_with_workspace(
        output: &CruxTriageOutput,
        ctx: &super::explorer_context::ExplorerWorkspaceContext,
    ) -> Result<()> {
        Self::validate_crux_triage_output(output)?;
        validate_crux_triage_with_context(output, ctx)
    }

    pub fn validate_experiment_input(experiment: &AnalysisExperimentInput) -> Result<()> {
        if experiment.experiment_key.trim().is_empty() {
            return Err(Error::string("experiment_key cannot be empty"));
        }
        if experiment.question.trim().is_empty() {
            return Err(Error::string("experiment question cannot be empty"));
        }
        if experiment.sql_body.trim().is_empty() {
            return Err(Error::string("experiment sql_body cannot be empty"));
        }
        if experiment.period_basis.trim().is_empty() {
            return Err(Error::string("experiment period_basis cannot be empty"));
        }
        Self::validate_disposition(&experiment.disposition, "experiment")?;
        match experiment.purpose.as_str() {
            "historical_investigation"
            | "sensitivity"
            | "forward_projection"
            | "scenario_validation" => {}
            other => {
                return Err(Error::string(&format!("invalid experiment purpose: {other}")));
            }
        }

        if experiment.disposition == "rejected"
            && experiment
                .rejection_reason
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(Error::string(
                "rejected experiments require rejection_reason",
            ));
        }

        if experiment.disposition == "promoted"
            && !outputs_include_arithmetic_and_interpretation(&experiment.outputs)
        {
            return Err(Error::string(
                "promoted experiments require at least one arithmetic/ratio output and one interpretation output",
            ));
        }

        Ok(())
    }

    pub async fn persist_crux_triage(
        db: &sea_orm::DatabaseConnection,
        output: &CruxTriageOutput,
        model: &str,
        worker_run_id: Option<i64>,
    ) -> Result<()> {
        let store = FinancialAnalysisStore::new(db);
        let selected_by = format!("financial_model_explorer:{model}");
        let created_at = Utc::now().to_rfc3339();
        let worker_run = worker_run_id.map(|id| id.to_string());
        store
            .persist_crux_triage(
                output,
                &selected_by,
                &created_at,
                worker_run.as_deref(),
            )
            .await?;
        Ok(())
    }

    fn build_tool_registry(&self, workspace_sqlite: &PathBuf) -> ToolRegistry {
        let mut registry = ToolRegistry::new().with_sql_query(workspace_sqlite.clone());
        match self.config.mode {
            ExplorerMode::CruxTriage => registry = registry.with_crux_triage_submit(),
            ExplorerMode::MechanicsExperiment => {
                registry = registry
                    .with_analysis_draft()
                    .with_analysis_finalize()
                    .with_mechanics_complete();
            }
        }
        registry
    }

    fn preamble(&self) -> &'static str {
        match self.config.mode {
            ExplorerMode::CruxTriage => CRUX_TRIAGE_PREAMBLE,
            ExplorerMode::MechanicsExperiment => MECHANICS_EXPERIMENT_PREAMBLE,
        }
    }

    async fn agent_prompt(&self, workspace_sqlite: &std::path::Path) -> Result<String> {
        let company_context = self
            .company_label
            .as_deref()
            .map(|label| format!("Company: {label}\n\n"))
            .unwrap_or_default();
        let focus = self
            .config
            .prompt_prefix
            .as_deref()
            .map(|prefix| format!("{prefix}\n\n"))
            .unwrap_or_default();
        let workspace_context = load_explorer_context(workspace_sqlite)
            .await
            .unwrap_or_else(|_| super::explorer_context::ExplorerWorkspaceContext {
                narrative_crux_count: 0,
                open_gaps: vec![],
                narrative_cruxes_summary: String::new(),
                sec_freshness_summary: String::new(),
                claims_guidance_present: false,
            });
        let context_section = format_explorer_context_section(&workspace_context);
        let golden_path = match self.config.mode {
            ExplorerMode::CruxTriage => crux_triage_golden_path(),
            ExplorerMode::MechanicsExperiment => mechanics_experiment_golden_path(),
        };
        let submit_shape = match self.config.mode {
            ExplorerMode::CruxTriage => crux_triage_submit_example().to_string(),
            ExplorerMode::MechanicsExperiment => {
                if self.config.scout_worker {
                    format!(
                        "{}\n\n{}",
                        mechanics_finalize_example(),
                        mechanics_scout_submit_example()
                    )
                } else if self.config.prompt_prefix.is_some() {
                    format!(
                        "{}\n\n{}",
                        mechanics_finalize_example(),
                        mechanics_per_worker_submit_example()
                    )
                } else {
                    mechanics_finalize_example().to_string()
                }
            }
        };

        Ok(format!(
            r#"{company_context}{focus}{schema}

Workspace context:
{context_section}

{golden_path}

Submit shape:
{submit_shape}"#,
            schema = explorer_schema_hint(),
        ))
    }

    fn validate_disposition(disposition: &str, kind: &str) -> Result<()> {
        let valid = match kind {
            "crux" => matches!(
                disposition,
                "promoted" | "background" | "rejected" | "unresolved"
            ),
            "experiment" => matches!(
                disposition,
                "draft" | "candidate" | "promoted" | "rejected" | "background"
            ),
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::string(&format!(
                "invalid {kind} disposition: {disposition}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::financial_model_explorer::types::{
        CruxCandidateInput, SupportingMetricPromotion,
    };

    #[test]
    fn validates_crux_triage_output() {
        let output = CruxTriageOutput {
            cruxes: vec![CruxCandidateInput {
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
            }],
            supporting_metrics: vec![
                SupportingMetricPromotion {
                    selection_scope: "crux_support".to_string(),
                    crux_key: Some("test_crux".to_string()),
                    taxonomy: "us-gaap".to_string(),
                    concept_name: "RevenueRemainingPerformanceObligation".to_string(),
                    unit: "USD".to_string(),
                    label: None,
                    rationale: "Driver".to_string(),
                    period_basis: None,
                    quality_status: None,
                },
                SupportingMetricPromotion {
                    selection_scope: "crux_support".to_string(),
                    crux_key: Some("test_crux".to_string()),
                    taxonomy: "us-gaap".to_string(),
                    concept_name: "PaymentsToAcquirePropertyPlantAndEquipment".to_string(),
                    unit: "USD".to_string(),
                    label: None,
                    rationale: "Capex".to_string(),
                    period_basis: None,
                    quality_status: None,
                },
            ],
            quality_flags: vec![],
            open_questions: vec![],
        };
        FinancialModelExplorerAgent::validate_crux_triage_output(&output).expect("valid");
    }
}
