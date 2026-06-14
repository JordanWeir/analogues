pub mod analysis_draft_run;
pub mod analysis_finalize;
pub mod concept_review_submit;
pub mod crux_triage_submit;
pub mod fundamentals_lookup;
pub mod mechanics_complete;
pub mod narrative_research;
pub mod scenario_blueprint_submit;
pub mod scenario_detail_submit;
pub mod sql_query;
pub mod web_search;

use crate::services::openrouter_chat::{
    ClientToolExecuteResult, ClientToolHandler, CompletionTool,
};
use analysis_draft_run::TOOL_NAME as ANALYSIS_DRAFT_TOOL_NAME;
use analysis_finalize::TOOL_NAME as ANALYSIS_FINALIZE_TOOL_NAME;
use concept_review_submit::TOOL_NAME as CONCEPT_REVIEW_SUBMIT_TOOL_NAME;
use crux_triage_submit::TOOL_NAME as CRUX_TRIAGE_SUBMIT_TOOL_NAME;
use fundamentals_lookup::TOOL_NAME as FUNDAMENTALS_LOOKUP_TOOL_NAME;
use scenario_blueprint_submit::TOOL_NAME as SCENARIO_BLUEPRINT_SUBMIT_TOOL_NAME;
use scenario_detail_submit::TOOL_NAME as SCENARIO_DETAIL_SUBMIT_TOOL_NAME;
use mechanics_complete::TOOL_NAME as MECHANICS_COMPLETE_TOOL_NAME;
use narrative_research::NARRATIVE_TOOL_NAMES;
use sql_query::TOOL_NAME as SQL_QUERY_TOOL_NAME;

pub use analysis_draft_run::TOOL_NAME as ANALYSIS_DRAFT_TOOL;
pub use analysis_finalize::TOOL_NAME as ANALYSIS_FINALIZE_TOOL;
pub use crux_triage_submit::TOOL_NAME as CRUX_TRIAGE_SUBMIT_TOOL;
pub use scenario_blueprint_submit::TOOL_NAME as SCENARIO_BLUEPRINT_SUBMIT_TOOL;
pub use scenario_detail_submit::TOOL_NAME as SCENARIO_DETAIL_SUBMIT_TOOL;
use std::{path::PathBuf, sync::Arc};
pub use web_search::WebSearchConfig;

#[derive(Debug, Clone)]
pub enum SharedTool {
    SqlQuery,
    WebSearch(WebSearchConfig),
    FundamentalsLookup,
    ConceptReviewSubmit,
    NarrativeResearch,
    CruxTriageSubmit,
    AnalysisDraft,
    AnalysisFinalize,
    MechanicsComplete,
    ScenarioBlueprintSubmit,
    ScenarioDetailSubmit,
}

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<SharedTool>,
    sqlite_path: Option<PathBuf>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sql_query(mut self, sqlite_path: PathBuf) -> Self {
        self.sqlite_path = Some(sqlite_path);
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::SqlQuery))
        {
            self.tools.push(SharedTool::SqlQuery);
        }
        self
    }

    pub fn with_web_search(mut self, config: WebSearchConfig) -> Self {
        self.tools
            .retain(|tool| !matches!(tool, SharedTool::WebSearch(_)));
        self.tools.push(SharedTool::WebSearch(config));
        self
    }

    pub fn with_narrative_research(mut self) -> Self {
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::NarrativeResearch))
        {
            self.tools.push(SharedTool::NarrativeResearch);
        }
        self
    }

    pub fn with_concept_review_submit(mut self) -> Self {
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::ConceptReviewSubmit))
        {
            self.tools.push(SharedTool::ConceptReviewSubmit);
        }
        self
    }

    pub fn with_fundamentals_lookup(mut self) -> Self {
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::FundamentalsLookup))
        {
            self.tools.push(SharedTool::FundamentalsLookup);
        }
        self
    }

    pub fn with_crux_triage_submit(mut self) -> Self {
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::CruxTriageSubmit))
        {
            self.tools.push(SharedTool::CruxTriageSubmit);
        }
        self
    }

    pub fn with_analysis_draft(mut self) -> Self {
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::AnalysisDraft))
        {
            self.tools.push(SharedTool::AnalysisDraft);
        }
        self
    }

    pub fn with_analysis_finalize(mut self) -> Self {
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::AnalysisFinalize))
        {
            self.tools.push(SharedTool::AnalysisFinalize);
        }
        self
    }

    pub fn with_scenario_blueprint_submit(mut self) -> Self {
        if !self.tools.iter().any(|t| matches!(t, SharedTool::ScenarioBlueprintSubmit)) {
            self.tools.push(SharedTool::ScenarioBlueprintSubmit);
        }
        self
    }

    pub fn with_scenario_detail_submit(mut self) -> Self {
        if !self.tools.iter().any(|t| matches!(t, SharedTool::ScenarioDetailSubmit)) {
            self.tools.push(SharedTool::ScenarioDetailSubmit);
        }
        self
    }

    pub fn with_mechanics_complete(mut self) -> Self {
        if !self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::MechanicsComplete))
        {
            self.tools.push(SharedTool::MechanicsComplete);
        }
        self
    }

    pub fn needs_client_loop(&self) -> bool {
        self.tools.iter().any(|tool| match tool {
            SharedTool::SqlQuery
            | SharedTool::FundamentalsLookup
            | SharedTool::ConceptReviewSubmit
            | SharedTool::NarrativeResearch
            | SharedTool::CruxTriageSubmit
            | SharedTool::AnalysisDraft
            | SharedTool::AnalysisFinalize
            | SharedTool::MechanicsComplete
            | SharedTool::ScenarioBlueprintSubmit
            | SharedTool::ScenarioDetailSubmit => true,
            SharedTool::WebSearch(_) => false,
        })
    }

    pub fn completion_tools(&self) -> Vec<CompletionTool> {
        if self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::NarrativeResearch))
        {
            let mut tools: Vec<CompletionTool> = self
                .narrative_completion_tools()
                .into_iter()
                .map(CompletionTool::Function)
                .collect();
            for tool in &self.tools {
                if let SharedTool::WebSearch(config) = tool {
                    tools.push(config.completion_tool());
                }
            }
            return tools;
        }

        self.tools
            .iter()
            .map(|tool| match tool {
                SharedTool::SqlQuery => CompletionTool::Function(sql_query::openrouter_tool()),
                SharedTool::WebSearch(config) => config.completion_tool(),
                SharedTool::FundamentalsLookup => {
                    CompletionTool::Function(fundamentals_lookup::openrouter_tool())
                }
                SharedTool::ConceptReviewSubmit => {
                    CompletionTool::Function(concept_review_submit::openrouter_tool())
                }
                SharedTool::CruxTriageSubmit => {
                    CompletionTool::Function(crux_triage_submit::openrouter_tool())
                }
                SharedTool::AnalysisDraft => {
                    CompletionTool::Function(analysis_draft_run::openrouter_tool())
                }
                SharedTool::AnalysisFinalize => {
                    CompletionTool::Function(analysis_finalize::openrouter_tool())
                }
                SharedTool::MechanicsComplete => {
                    CompletionTool::Function(mechanics_complete::openrouter_tool())
                }
                SharedTool::ScenarioBlueprintSubmit => {
                    CompletionTool::Function(scenario_blueprint_submit::openrouter_tool())
                }
                SharedTool::ScenarioDetailSubmit => {
                    CompletionTool::Function(scenario_detail_submit::openrouter_tool())
                }
                SharedTool::NarrativeResearch => unreachable!("handled above"),
            })
            .collect()
    }

    fn narrative_completion_tools(&self) -> Vec<openrouter_rs::types::Tool> {
        let mut tools = narrative_research::completion_tools();
        if self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::SqlQuery))
        {
            tools.insert(0, sql_query::openrouter_tool());
        }
        tools
    }

    pub fn client_handler(&self) -> Option<Arc<dyn ClientToolHandler>> {
        if !self.tools.iter().any(|tool| {
            matches!(
                tool,
                SharedTool::SqlQuery
                    | SharedTool::FundamentalsLookup
                    | SharedTool::ConceptReviewSubmit
                    | SharedTool::NarrativeResearch
                    | SharedTool::CruxTriageSubmit
                    | SharedTool::AnalysisDraft
                    | SharedTool::AnalysisFinalize
                    | SharedTool::MechanicsComplete
                    | SharedTool::ScenarioBlueprintSubmit
                    | SharedTool::ScenarioDetailSubmit
            )
        }) {
            return None;
        }
        Some(Arc::new(RegistryClientHandler {
            sqlite_path: self.sqlite_path.clone(),
            tools: self.tools.clone(),
        }))
    }
}

struct RegistryClientHandler {
    sqlite_path: Option<PathBuf>,
    tools: Vec<SharedTool>,
}

#[async_trait::async_trait]
impl ClientToolHandler for RegistryClientHandler {
    async fn execute(
        &self,
        tool_name: &str,
        arguments: &str,
    ) -> loco_rs::prelude::Result<ClientToolExecuteResult> {
        if tool_name == SQL_QUERY_TOOL_NAME {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "workspace_sql requires a workspace sqlite path to be configured",
                )
            })?;
            let result = sql_query::execute(path, arguments).await?;
            return Ok(ClientToolExecuteResult::Response(result));
        }
        if tool_name == CONCEPT_REVIEW_SUBMIT_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::ConceptReviewSubmit))
        {
            return concept_review_submit::execute(arguments);
        }
        if tool_name == FUNDAMENTALS_LOOKUP_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::FundamentalsLookup))
        {
            let result = fundamentals_lookup::execute(arguments).await?;
            return Ok(ClientToolExecuteResult::Response(result));
        }
        if self
            .tools
            .iter()
            .any(|tool| matches!(tool, SharedTool::NarrativeResearch))
            && NARRATIVE_TOOL_NAMES.contains(&tool_name)
        {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "narrative capture tools require a workspace sqlite path to be configured",
                )
            })?;
            return narrative_research::execute(path, tool_name, arguments).await;
        }
        if tool_name == CRUX_TRIAGE_SUBMIT_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::CruxTriageSubmit))
        {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "submit_crux_triage requires a workspace sqlite path to be configured",
                )
            })?;
            return crux_triage_submit::execute(path, arguments).await;
        }
        if tool_name == ANALYSIS_DRAFT_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::AnalysisDraft))
        {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "run_analysis_draft requires a workspace sqlite path to be configured",
                )
            })?;
            let result = analysis_draft_run::execute(path, arguments).await?;
            return Ok(ClientToolExecuteResult::Response(result));
        }
        if tool_name == ANALYSIS_FINALIZE_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::AnalysisFinalize))
        {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "finalize_analysis requires a workspace sqlite path to be configured",
                )
            })?;
            let result = analysis_finalize::execute(path, arguments).await?;
            return Ok(ClientToolExecuteResult::Response(result));
        }
        if tool_name == MECHANICS_COMPLETE_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::MechanicsComplete))
        {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "submit_mechanics_experiments requires a workspace sqlite path to be configured",
                )
            })?;
            return mechanics_complete::execute(path, arguments).await;
        }
        if tool_name == SCENARIO_BLUEPRINT_SUBMIT_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::ScenarioBlueprintSubmit))
        {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "submit_scenario_blueprint requires a workspace sqlite path",
                )
            })?;
            return scenario_blueprint_submit::execute(path, arguments).await;
        }
        if tool_name == SCENARIO_DETAIL_SUBMIT_TOOL_NAME
            && self
                .tools
                .iter()
                .any(|tool| matches!(tool, SharedTool::ScenarioDetailSubmit))
        {
            let path = self.sqlite_path.as_ref().ok_or_else(|| {
                loco_rs::prelude::Error::string(
                    "submit_scenario_detail requires a workspace sqlite path",
                )
            })?;
            return scenario_detail_submit::execute(path, arguments).await;
        }

        Err(loco_rs::prelude::Error::string(&format!(
            "unknown or disabled client tool: {tool_name}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_builds_sql_and_web_search_tools() {
        let registry = ToolRegistry::new()
            .with_sql_query(PathBuf::from("/tmp/test.sqlite"))
            .with_web_search(WebSearchConfig::concept_validation_defaults());

        assert!(registry.needs_client_loop());
        let tools = registry.completion_tools();
        assert_eq!(tools.len(), 2);

        let values: Vec<_> = tools
            .iter()
            .map(|tool| serde_json::to_value(tool).expect("tool should serialize"))
            .collect();
        assert_eq!(values[0]["type"], "function");
        assert_eq!(values[0]["function"]["name"], "workspace_sql");
        assert_eq!(values[1]["type"], "openrouter:web_search");
    }

    #[test]
    fn concept_review_registry_includes_sql_and_submit_tools() {
        let registry = ToolRegistry::new()
            .with_sql_query(PathBuf::from("/tmp/test.sqlite"))
            .with_concept_review_submit();

        assert!(registry.needs_client_loop());
        let tools = registry.completion_tools();
        assert_eq!(tools.len(), 2);

        let values: Vec<_> = tools
            .iter()
            .map(|tool| serde_json::to_value(tool).expect("tool should serialize"))
            .collect();
        assert_eq!(values[0]["function"]["name"], "workspace_sql");
        assert_eq!(values[1]["function"]["name"], "submit_concept_review");
    }

    #[test]
    fn web_search_alone_does_not_need_client_loop() {
        let registry = ToolRegistry::new().with_web_search(WebSearchConfig::default());
        assert!(!registry.needs_client_loop());
        assert_eq!(registry.completion_tools().len(), 1);
        assert_eq!(
            serde_json::to_value(&registry.completion_tools()[0]).expect("serialize"),
            json!({"type": "openrouter:web_search"})
        );
    }

    #[test]
    fn financial_explorer_registry_includes_mode_tools() {
        let registry = ToolRegistry::new()
            .with_sql_query(PathBuf::from("/tmp/test.sqlite"))
            .with_analysis_draft()
            .with_analysis_finalize()
            .with_mechanics_complete();

        let names: Vec<_> = registry
            .completion_tools()
            .iter()
            .map(|tool| {
                serde_json::to_value(tool).expect("serialize")["function"]["name"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(names, vec![
            "workspace_sql",
            "run_analysis_draft",
            "finalize_analysis",
            "submit_mechanics_experiments"
        ]);
    }
}
