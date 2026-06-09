pub mod concept_review_submit;
pub mod fundamentals_lookup;
pub mod sql_query;
pub mod web_search;

use crate::services::openrouter_chat::{
    ClientToolExecuteResult, ClientToolHandler, CompletionTool,
};
use concept_review_submit::TOOL_NAME as CONCEPT_REVIEW_SUBMIT_TOOL_NAME;
use fundamentals_lookup::TOOL_NAME as FUNDAMENTALS_LOOKUP_TOOL_NAME;
use sql_query::TOOL_NAME as SQL_QUERY_TOOL_NAME;
use std::{path::PathBuf, sync::Arc};
pub use web_search::WebSearchConfig;

#[derive(Debug, Clone)]
pub enum SharedTool {
    SqlQuery,
    WebSearch(WebSearchConfig),
    FundamentalsLookup,
    ConceptReviewSubmit,
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

    pub fn needs_client_loop(&self) -> bool {
        self.tools.iter().any(|tool| match tool {
            SharedTool::SqlQuery
            | SharedTool::FundamentalsLookup
            | SharedTool::ConceptReviewSubmit => true,
            SharedTool::WebSearch(_) => false,
        })
    }

    pub fn completion_tools(&self) -> Vec<CompletionTool> {
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
            })
            .collect()
    }

    pub fn client_handler(&self) -> Option<Arc<dyn ClientToolHandler>> {
        if !self.tools.iter().any(|tool| {
            matches!(
                tool,
                SharedTool::SqlQuery
                    | SharedTool::FundamentalsLookup
                    | SharedTool::ConceptReviewSubmit
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
}
