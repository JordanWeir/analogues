use super::{
    analysis_draft_run, analysis_finalize, crux_triage_submit, mechanics_complete, sql_query,
    ANALYSIS_DRAFT_TOOL_NAME, ANALYSIS_FINALIZE_TOOL_NAME, CRUX_TRIAGE_SUBMIT_TOOL_NAME,
    MECHANICS_COMPLETE_TOOL_NAME,
};
use crate::{
    agents::financial_model_explorer::types::ExplorerMode,
    services::openrouter_chat::{ClientToolExecuteResult, ClientToolHandler},
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::path::PathBuf;

pub struct FinancialExplorerHandler {
    sqlite_path: PathBuf,
    mode: ExplorerMode,
}

impl FinancialExplorerHandler {
    pub fn new(sqlite_path: PathBuf, mode: ExplorerMode) -> Self {
        Self { sqlite_path, mode }
    }
}

#[async_trait]
impl ClientToolHandler for FinancialExplorerHandler {
    async fn execute(
        &self,
        tool_name: &str,
        arguments: &str,
    ) -> Result<ClientToolExecuteResult> {
        if tool_name == sql_query::TOOL_NAME {
            let result = sql_query::execute(&self.sqlite_path, arguments).await?;
            return Ok(ClientToolExecuteResult::Response(result));
        }

        match self.mode {
            ExplorerMode::CruxTriage if tool_name == CRUX_TRIAGE_SUBMIT_TOOL_NAME => {
                return crux_triage_submit::execute(arguments);
            }
            ExplorerMode::MechanicsExperiment => match tool_name {
                ANALYSIS_DRAFT_TOOL_NAME => {
                    return analysis_draft_run::execute_sync_for_handler(
                        &self.sqlite_path,
                        arguments,
                    );
                }
                ANALYSIS_FINALIZE_TOOL_NAME => {
                    return analysis_finalize::execute_sync_for_handler(
                        &self.sqlite_path,
                        arguments,
                    );
                }
                MECHANICS_COMPLETE_TOOL_NAME => {
                    return mechanics_complete::execute_sync_for_handler(
                        &self.sqlite_path,
                        arguments,
                    );
                }
                _ => {}
            },
            _ => {}
        }

        Err(Error::string(&format!(
            "unknown or disabled financial explorer tool: {tool_name}"
        )))
    }
}
