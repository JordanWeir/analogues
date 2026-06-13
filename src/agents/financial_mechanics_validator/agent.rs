use super::{config::FinancialMechanicsValidatorConfig, prompt, WORKER_NAME};
use crate::{
    agents::{
        tool_loop_agent::{ToolLoopAgent, ToolLoopRequest},
        tools::ToolRegistry,
    },
    services::mechanics_review::MechanicsReviewService,
};
use loco_rs::prelude::*;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone)]
pub struct FinancialMechanicsValidatorAgent {
    config: FinancialMechanicsValidatorConfig,
    company_label: Option<String>,
}

impl FinancialMechanicsValidatorAgent {
    pub fn new(config: FinancialMechanicsValidatorConfig) -> Self {
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
        let tools = ToolRegistry::new()
            .with_sql_query(workspace_sqlite.clone())
            .with_mechanics_review_submit(self.config.review_round);

        let response = ToolLoopAgent::default()
            .run(ToolLoopRequest {
                worker_name: WORKER_NAME.to_string(),
                model: self.config.model.clone(),
                preamble: prompt::PREAMBLE.to_string(),
                prompt: prompt::build_user_prompt(
                    self.company_label.as_deref(),
                    self.config.prompt_prefix.as_deref(),
                    self.config.review_round,
                ),
                json_mode: false,
                tools,
                metadata: BTreeMap::from([
                    ("lane".to_string(), "financial_mechanics_validation".to_string()),
                    ("ticker".to_string(), ticker.to_string()),
                    ("review_round".to_string(), self.config.review_round.to_string()),
                    (
                        "scope".to_string(),
                        if self.config.scout_worker {
                            "scout".to_string()
                        } else {
                            self.config
                                .focus_crux_key
                                .clone()
                                .unwrap_or_else(|| "workspace".to_string())
                        },
                    ),
                ]),
                workspace_sqlite: Some(workspace_sqlite),
                client_tools: None,
                max_agent_rounds: Some(self.config.max_agent_rounds),
                submit_tool_name: Some("submit_mechanics_review".to_string()),
                prepare_step: None,
                stop_when: None,
            })
            .await?;

        Ok((response.text, response.worker_run_id))
    }

    pub fn parse_review_output(text: &str) -> Result<crate::agents::financial_model_explorer::types::MechanicsReviewOutput> {
        MechanicsReviewService::parse_output(text)
    }
}
