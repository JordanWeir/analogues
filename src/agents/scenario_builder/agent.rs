use super::{
    config::ScenarioBuilderConfig,
    context::{format_scenario_context_section, load_scenario_context},
    golden_path::{
        scenario_blueprint_golden_path, scenario_blueprint_submit_example, scenario_detail_golden_path,
        scenario_detail_submit_example, scenario_schema_hint,
    },
    types::{
        ScenarioBlueprintOutput, ScenarioBuilderMode, ScenarioDetailOutput, SCENARIO_BLUEPRINT_MAX,
        SCENARIO_BLUEPRINT_MIN,
    },
    WORKER_NAME,
};
use crate::{
    agents::{
        tool_loop_agent::{ToolLoopAgent, ToolLoopRequest},
        tools::{ToolRegistry, WebSearchConfig},
    },
    services::{
        model_client::extract_json_blob,
        scenario_store::ScenarioStore,
    },
};
use loco_rs::prelude::*;
use sea_orm::ConnectionTrait;
use std::{collections::BTreeMap, path::PathBuf};

pub const SCENARIO_BLUEPRINT_PREAMBLE: &str = "You are the Scenario Builder in blueprint mode. Design 4–6 company-specific conditional scenarios from the narrative map, promoted cruxes, and financial experiments. Use AlphaVantage quarterly data (av_raw_facts) as the primary time-series source for understanding recent trajectory; use SEC and experiments for crux bridges only. Use workspace_sql and web_search when claims contradict AV or SEC. Finish with submit_scenario_blueprint — do not end with plain prose. Fix validation errors and resubmit.";

pub const SCENARIO_DETAIL_PREAMBLE: &str = "You are the Scenario Builder in detail mode. Build quarterly projection paths for ONE assigned scenario. Anchor ~4 historical quarters on AlphaVantage actuals (av_raw_facts, report_type='quarterly'). Project 12–20 forward quarters on a quarterly cadence. Use analysis_experiments and claims to shape forward assumptions; interpolate where needed. Set valuation bands on the terminal forward quarter only. Reuse existing sources.id values when citing crux assumptions — do not invent source ids. Use workspace_sql and web_search for contradictions. Finish with submit_scenario_detail and per_worker true.";

#[derive(Debug, Clone)]
pub struct ScenarioBuilderAgent {
    config: ScenarioBuilderConfig,
    company_label: Option<String>,
}

impl ScenarioBuilderAgent {
    pub fn new(config: ScenarioBuilderConfig) -> Self {
        Self {
            config,
            company_label: None,
        }
    }

    pub fn with_company_label(mut self, label: impl Into<String>) -> Self {
        self.company_label = Some(label.into());
        self
    }

    pub async fn run(&self, workspace_sqlite: PathBuf, ticker: &str) -> Result<(String, Option<i64>)> {
        let tools = self.build_tool_registry(&workspace_sqlite);
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
                prepare_step: None,
                stop_when: None,
            })
            .await?;

        Ok((response.text, response.worker_run_id))
    }

    pub fn parse_blueprint_output(text: &str) -> Result<ScenarioBlueprintOutput> {
        let json_text = extract_json_blob(text).ok_or_else(|| {
            Error::string(
                "scenario blueprint response did not contain JSON; call submit_scenario_blueprint",
            )
        })?;
        let output: ScenarioBlueprintOutput =
            serde_json::from_str(json_text).map_err(|err| {
                Error::string(&format!("invalid scenario blueprint JSON: {err}"))
            })?;
        Self::validate_blueprint_output(&output)?;
        Ok(output)
    }

    pub fn parse_detail_output(text: &str) -> Result<ScenarioDetailOutput> {
        let json_text = extract_json_blob(text).ok_or_else(|| {
            Error::string(
                "scenario detail response did not contain JSON; call submit_scenario_detail",
            )
        })?;
        let output: ScenarioDetailOutput = serde_json::from_str(json_text).map_err(|err| {
            Error::string(&format!("invalid scenario detail JSON: {err}"))
        })?;
        Self::validate_detail_output(&output)?;
        Ok(output)
    }

    pub fn validate_blueprint_output(output: &ScenarioBlueprintOutput) -> Result<()> {
        let count = output.scenarios.len();
        if count < SCENARIO_BLUEPRINT_MIN || count > SCENARIO_BLUEPRINT_MAX {
            return Err(Error::string(&format!(
                "blueprint requires {SCENARIO_BLUEPRINT_MIN}–{SCENARIO_BLUEPRINT_MAX} scenarios, got {count}"
            )));
        }

        let mut keys = std::collections::HashSet::new();
        let mut stances = std::collections::HashSet::new();
        let mut prob_sum = 0.0;
        for scenario in &output.scenarios {
            if scenario.scenario_key.trim().is_empty() {
                return Err(Error::string("scenario_key cannot be empty"));
            }
            if !keys.insert(scenario.scenario_key.clone()) {
                return Err(Error::string(&format!(
                    "duplicate scenario_key: {}",
                    scenario.scenario_key
                )));
            }
            Self::validate_stance(&scenario.stance)?;
            stances.insert(scenario.stance.clone());
            if scenario.name.trim().is_empty() || scenario.description.trim().is_empty() {
                return Err(Error::string(&format!(
                    "scenario '{}' needs name and description",
                    scenario.scenario_key
                )));
            }
            if scenario.probability < 0.0 {
                return Err(Error::string(&format!(
                    "scenario '{}' probability cannot be negative",
                    scenario.scenario_key
                )));
            }
            prob_sum += scenario.probability;
        }

        for required in ["bullish", "neutral", "bearish"] {
            if !stances.contains(required) {
                return Err(Error::string(&format!(
                    "blueprint must include at least one {required} scenario"
                )));
            }
        }

        if prob_sum < 0.85 || prob_sum > 1.15 {
            return Err(Error::string(&format!(
                "scenario probabilities should sum to ~1.0 before normalization, got {prob_sum:.3}"
            )));
        }

        Ok(())
    }

    pub fn validate_detail_output(output: &ScenarioDetailOutput) -> Result<()> {
        if output.scenario_key.trim().is_empty() {
            return Err(Error::string("scenario_key cannot be empty"));
        }
        if output.assumption_summary.trim().is_empty() {
            return Err(Error::string("assumption_summary cannot be empty"));
        }
        if output.periods.is_empty() {
            return Err(Error::string("scenario detail requires at least one period"));
        }

        let mut orders = std::collections::HashSet::new();
        let mut has_terminal_multiples = false;
        for period in &output.periods {
            if !orders.insert(period.period_order) {
                return Err(Error::string(&format!(
                    "duplicate period_order {}",
                    period.period_order
                )));
            }
            if period.label.trim().is_empty() || period.period_end.trim().is_empty() {
                return Err(Error::string("each period needs label and period_end"));
            }
            if period.period_type != "quarter" {
                return Err(Error::string(&format!(
                    "period '{}' must use period_type quarter",
                    period.label
                )));
            }
            if period.revenue.is_none() && period.revenue_growth.is_none() {
                return Err(Error::string(&format!(
                    "period '{}' needs revenue or revenue_growth",
                    period.label
                )));
            }
            if period.ps_median.is_some() {
                has_terminal_multiples = true;
            }
        }

        let max_order = output.periods.iter().map(|p| p.period_order).max().unwrap_or(0);
        let terminal = output
            .periods
            .iter()
            .find(|p| p.period_order == max_order)
            .ok_or_else(|| Error::string("terminal period missing"))?;
        if terminal.ps_median.is_none() {
            return Err(Error::string(
                "terminal period must include ps_median valuation band",
            ));
        }
        if !has_terminal_multiples {
            return Err(Error::string("terminal period needs valuation multiples"));
        }

        if output.crux_assumptions.is_empty() {
            return Err(Error::string("scenario detail needs at least one crux_assumption"));
        }
        if output.sensitivities.is_empty() {
            return Err(Error::string("scenario detail needs at least one sensitivity"));
        }
        if output.confirming_signals.is_empty() || output.breaking_signals.is_empty() {
            return Err(Error::string(
                "scenario detail needs confirming and breaking signals",
            ));
        }

        Ok(())
    }

    pub async fn validate_detail_output_for_workspace(
        db: &impl ConnectionTrait,
        output: &ScenarioDetailOutput,
    ) -> Result<()> {
        Self::validate_detail_output(output)?;
        ScenarioStore::validate_detail_references(db, output).await
    }

    fn build_tool_registry(&self, workspace_sqlite: &PathBuf) -> ToolRegistry {
        let mut registry = ToolRegistry::new()
            .with_sql_query(workspace_sqlite.clone())
            .with_web_search(WebSearchConfig::concept_validation_defaults());
        match self.config.mode {
            ScenarioBuilderMode::Blueprint => registry.with_scenario_blueprint_submit(),
            ScenarioBuilderMode::Detail => registry.with_scenario_detail_submit(),
        }
    }

    fn preamble(&self) -> &'static str {
        match self.config.mode {
            ScenarioBuilderMode::Blueprint => SCENARIO_BLUEPRINT_PREAMBLE,
            ScenarioBuilderMode::Detail => SCENARIO_DETAIL_PREAMBLE,
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
        let workspace_context = load_scenario_context(workspace_sqlite)
            .await
            .unwrap_or_else(|_| super::context::ScenarioWorkspaceContext {
                promoted_crux_count: 0,
                promoted_experiment_count: 0,
                av_quarter_count: 0,
                claims_count: 0,
                crux_summary: String::new(),
                experiment_summary: String::new(),
                av_coverage_summary: String::new(),
                blueprint_summary: String::new(),
                sources_summary: String::new(),
            });
        let context_section = format_scenario_context_section(&workspace_context);
        let (golden_path, submit_shape) = match self.config.mode {
            ScenarioBuilderMode::Blueprint => (
                scenario_blueprint_golden_path(),
                scenario_blueprint_submit_example(),
            ),
            ScenarioBuilderMode::Detail => (
                scenario_detail_golden_path(),
                scenario_detail_submit_example(),
            ),
        };

        Ok(format!(
            r#"{company_context}{focus}{schema}

Workspace context:
{context_section}

{golden_path}

Submit shape:
{submit_shape}"#,
            schema = scenario_schema_hint(),
        ))
    }

    fn validate_stance(stance: &str) -> Result<()> {
        match stance {
            "bullish" | "neutral" | "bearish" | "mixed" => Ok(()),
            other => Err(Error::string(&format!("invalid stance: {other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::scenario_builder::types::ScenarioBlueprint;

    #[test]
    fn validates_blueprint_stance_coverage() {
        let output = ScenarioBlueprintOutput {
            scenarios: vec![
                blueprint("bull", "bullish", 0.4),
                blueprint("base", "neutral", 0.35),
                blueprint("bear", "bearish", 0.25),
                blueprint("mixed", "mixed", 0.0),
            ],
            projection_notes: vec![],
        };
        ScenarioBuilderAgent::validate_blueprint_output(&output).expect("valid");
    }

    fn blueprint(key: &str, stance: &str, prob: f64) -> ScenarioBlueprint {
        ScenarioBlueprint {
            scenario_key: key.to_string(),
            name: key.to_string(),
            stance: stance.to_string(),
            probability: prob,
            description: "desc".to_string(),
            crux_resolution_summary: "resolves crux".to_string(),
            linked_crux_keys: vec!["crux_a".to_string()],
            linked_experiment_keys: vec![],
        }
    }
}
