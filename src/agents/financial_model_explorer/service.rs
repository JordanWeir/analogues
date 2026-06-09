use super::{
    golden_path::{crux_triage_golden_path, explorer_schema_hint, mechanics_experiment_golden_path},
    types::{
        AnalysisExperimentInput, CruxTriageOutput, ExplorerMode, MechanicsExperimentsComplete,
    },
};
use crate::{
    agents::{
        tool_loop_agent::{ToolLoopAgent, ToolLoopRequest},
        tools::{financial_explorer_handler::FinancialExplorerHandler, ToolRegistry},
    },
    services::{
        financial_analysis_store::{outputs_include_arithmetic_and_interpretation, FinancialAnalysisStore},
        model_client::extract_json_blob,
        openrouter_chat::FINANCIAL_EXPLORER_MAX_AGENT_ROUNDS,
    },
};
use chrono::Utc;
use loco_rs::prelude::*;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

pub const CRUX_TRIAGE_PREAMBLE: &str = "You are the Financial Model Explorer in crux-triage mode. Connect SEC concept catalogs to the narrative map and identify a small set of falsifiable crux candidates. Use workspace_sql following the golden path. Search concept_catalog_entries before sec_raw_facts. Promote supporting metrics only when they confirm, complicate, or contradict a narrative. When finished, call submit_crux_triage — do not end with a plain assistant message. Fix validation errors and resubmit.";

pub const MECHANICS_EXPERIMENT_PREAMBLE: &str = "You are the Financial Model Explorer in mechanics-experiment mode. Test how crux mechanics affect revenue, margins, cash flow, and funding pressure using focused SQLite calculations. Use run_analysis_draft to execute SQL and inspect results before judging them. Use finalize_analysis to promote, reject, or background an experiment based on the draft results. Separate arithmetic outputs from interpretation outputs. When at least one promoted experiment exists, call submit_mechanics_experiments to finish.";

#[derive(Debug, Clone)]
pub struct FinancialModelExplorerService {
    pub model: String,
    pub company_label: Option<String>,
    pub workspace_sqlite: Option<PathBuf>,
    pub mode: ExplorerMode,
}

impl FinancialModelExplorerService {
    pub fn crux_triage(workspace_sqlite: PathBuf, ticker: &str) -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            company_label: Some(ticker.to_string()),
            workspace_sqlite: Some(workspace_sqlite),
            mode: ExplorerMode::CruxTriage,
        }
    }

    pub fn mechanics_experiments(workspace_sqlite: PathBuf, ticker: &str) -> Self {
        Self {
            model: "deepseek/deepseek-v4-flash".to_string(),
            company_label: Some(ticker.to_string()),
            workspace_sqlite: Some(workspace_sqlite),
            mode: ExplorerMode::MechanicsExperiment,
        }
    }

    pub async fn run(&self) -> Result<(String, Option<i64>)> {
        let workspace_sqlite = self.workspace_sqlite.clone().ok_or_else(|| {
            Error::string("financial model explorer requires workspace_sqlite")
        })?;

        let tools = self.build_tool_registry(&workspace_sqlite);
        let client_tools = Arc::new(FinancialExplorerHandler::new(
            workspace_sqlite.clone(),
            self.mode,
        ));

        let response = ToolLoopAgent::default()
            .run(ToolLoopRequest {
                worker_name: self.mode.worker_lane().to_string(),
                model: self.model.clone(),
                preamble: self.preamble().to_string(),
                prompt: self.agent_prompt()?,
                json_mode: false,
                tools,
                metadata: BTreeMap::from([
                    ("worker_lane".to_string(), self.mode.worker_lane().to_string()),
                    ("explorer_mode".to_string(), format!("{:?}", self.mode)),
                ]),
                workspace_sqlite: Some(workspace_sqlite),
                client_tools: Some(client_tools),
                max_agent_rounds: Some(FINANCIAL_EXPLORER_MAX_AGENT_ROUNDS),
                submit_tool_name: Some(self.mode.submit_tool_name().to_string()),
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

    pub fn validate_mechanics_complete(_output: &MechanicsExperimentsComplete) -> Result<()> {
        Ok(())
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
        match self.mode {
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
        match self.mode {
            ExplorerMode::CruxTriage => CRUX_TRIAGE_PREAMBLE,
            ExplorerMode::MechanicsExperiment => MECHANICS_EXPERIMENT_PREAMBLE,
        }
    }

    fn agent_prompt(&self) -> Result<String> {
        let company_context = self
            .company_label
            .as_deref()
            .map(|label| format!("Company: {label}\n\n"))
            .unwrap_or_default();
        let golden_path = match self.mode {
            ExplorerMode::CruxTriage => crux_triage_golden_path(),
            ExplorerMode::MechanicsExperiment => mechanics_experiment_golden_path(),
        };
        let submit_shape = match self.mode {
            ExplorerMode::CruxTriage => {
                r#"{"cruxes":[{"crux_key":"rpo_conversion","title":"RPO conversion","statement":"...","bridge_archetype":"backlog_to_cash_conversion","narrative_side":"bear","watch_condition":"...","confirming_signal":"...","breaking_signal":"...","disposition":"promoted","rationale":"...","cluster_members":[{"taxonomy":"us-gaap","concept_name":"RevenueRemainingPerformanceObligation","unit":"USD","role":"driver"}],"linked_claim_ids":[]}],"supporting_metrics":[],"quality_flags":[],"open_questions":[]}"#
            }
            ExplorerMode::MechanicsExperiment => {
                r#"Use run_analysis_draft and finalize_analysis during exploration. Finish with submit_mechanics_experiments: {"summary":"..."}"#
            }
        };

        Ok(format!(
            r#"{company_context}{schema}

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
    use crate::agents::financial_model_explorer::types::CruxCandidateInput;

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
            supporting_metrics: vec![],
            quality_flags: vec![],
            open_questions: vec![],
        };
        FinancialModelExplorerService::validate_crux_triage_output(&output).expect("valid");
    }
}
