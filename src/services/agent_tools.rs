use crate::services::{
    openrouter_chat::ClientToolHandler,
    workspace_query::{execute_workspace_query_json, workspace_sql_tool_definition},
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

pub struct WorkspaceAgentTools {
    pub sqlite_path: PathBuf,
}

#[async_trait::async_trait]
impl ClientToolHandler for WorkspaceAgentTools {
    async fn execute(&self, tool_name: &str, arguments: &str) -> Result<String> {
        match tool_name {
            "workspace_sql" => {
                let args: Value = serde_json::from_str(arguments).map_err(|err| {
                    Error::string(&format!("workspace_sql arguments were not JSON: {err}"))
                })?;
                let query = args
                    .get("query")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::string("workspace_sql requires a query argument"))?;
                execute_workspace_query_json(&self.sqlite_path, query).await
            }
            other => Err(Error::string(&format!("unknown client tool: {other}"))),
        }
    }
}

pub fn workspace_agent_tools(sqlite_path: PathBuf) -> Arc<dyn ClientToolHandler> {
    Arc::new(WorkspaceAgentTools { sqlite_path })
}

pub fn workspace_sql_tool() -> Tool {
    let definition = workspace_sql_tool_definition();
    let function = definition
        .get("function")
        .cloned()
        .expect("workspace_sql tool definition should include function metadata");

    Tool::builder()
        .name(
            function
                .get("name")
                .and_then(Value::as_str)
                .expect("workspace_sql tool should have a name"),
        )
        .description(
            function
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .parameters(
            function
                .get("parameters")
                .cloned()
                .expect("workspace_sql tool should have parameters"),
        )
        .build()
        .expect("workspace_sql tool definition should be valid")
}
