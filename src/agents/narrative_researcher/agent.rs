use super::{
    config::NarrativeResearcherConfig,
    preamble::AGENT_PREAMBLE,
    research_workspace::{narrative_research_golden_path, workspace_schema_hint},
    WORKER_NAME,
};
use crate::{
    agents::{
        tool_loop_agent::{ToolLoopAgent, ToolLoopRequest, ToolLoopResponse},
        tools::{ToolRegistry, WebSearchConfig},
    },
    services::{
        narrative_research_store::NarrativeResearchStore,
        workspace_store::WorkspaceHandle,
    },
};
use loco_rs::prelude::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct NarrativeResearcherAgent {
    config: NarrativeResearcherConfig,
}

#[derive(Debug, Clone)]
pub struct NarrativeResearchRunResult {
    pub worker_run_id: Option<i64>,
    pub source_count: i64,
    pub claim_count: i64,
    pub crux_count: i64,
}

impl NarrativeResearcherAgent {
    pub fn new(config: NarrativeResearcherConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &NarrativeResearcherConfig {
        &self.config
    }

    pub async fn run_on_workspace(
        &self,
        workspace: &WorkspaceHandle,
    ) -> Result<NarrativeResearchRunResult> {
        self.run_on_workspace_with_telemetry(workspace)
            .await
            .map(|(result, _)| result)
    }

    pub async fn run_on_workspace_with_telemetry(
        &self,
        workspace: &WorkspaceHandle,
    ) -> Result<(NarrativeResearchRunResult, ToolLoopResponse)> {
        let workspace_sqlite = workspace.paths.sqlite_path.clone();
        NarrativeResearchStore::clear_narrative_state(workspace.connection()).await?;

        let ticker = scalar_ticker(workspace.connection()).await?;
        let company_name = load_company_name(workspace.connection()).await?;
        let fundamentals_summary = fundamentals_summary(workspace).await?;

        let mut tools = ToolRegistry::new()
            .with_sql_query(workspace_sqlite.clone())
            .with_narrative_research();
        if self.config.enable_web_search {
            tools = tools.with_web_search(WebSearchConfig::concept_validation_defaults());
        }

        let response = ToolLoopAgent::default()
            .run(ToolLoopRequest {
                worker_name: WORKER_NAME.to_string(),
                model: self.config.model.clone(),
                preamble: AGENT_PREAMBLE.to_string(),
                prompt: build_user_prompt(
                    &ticker,
                    company_name.as_deref(),
                    &fundamentals_summary,
                )?,
                json_mode: false,
                tools,
                metadata: BTreeMap::from([
                    ("lane".to_string(), "build_narrative_map".to_string()),
                    ("ticker".to_string(), ticker.clone()),
                ]),
                workspace_sqlite: Some(workspace_sqlite),
                client_tools: None,
                max_agent_rounds: Some(self.config.max_agent_rounds),
                submit_tool_name: Some("finalize_narrative_research".to_string()),
            })
            .await?;

        NarrativeResearchStore::finalize(workspace.connection())
            .await
            .map_err(|err| {
                let preview: String = response.text.chars().take(300).collect();
                Error::string(&format!(
                    "{err}; agent finalize incomplete (preview: {preview})"
                ))
            })?;

        let snapshot = NarrativeResearchStore::snapshot(workspace.connection()).await?;
        Ok((
            NarrativeResearchRunResult {
                worker_run_id: response.worker_run_id,
                source_count: snapshot.source_count,
                claim_count: snapshot.claim_count,
                crux_count: snapshot.crux_count,
            },
            response,
        ))
    }
}

pub fn build_user_prompt(
    ticker: &str,
    company_name: Option<&str>,
    fundamentals_summary: &str,
) -> Result<String> {
    let company = company_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(ticker);
    Ok(format!(
        r#"Build a source pack and narrative map for {company} ({ticker}).

{schema}

{golden_path}

## Fundamentals snapshot (from workspace)
{fundamentals}

## Incremental capture tools
Use these tools throughout research (not one bulk submit):
- capture_sources — add 1–3 sources per call after each discovery round
- capture_claims — add claims linked to source_id from prior captures
- capture_narrative_side — capture bull, then bear, then dominant/consensus separately
- capture_narrative_items — capture agreements, then cruxes (item_type agreement|crux)
- capture_orientation — dominant_question, current_setup, time_horizon
- capture_section — business_model and why_now
- capture_research_gap — unresolved questions
- finalize_narrative_research — validate and complete when all required pieces exist

Each capture tool returns a workspace snapshot showing progress. Fix gaps before finalize."#,
        schema = workspace_schema_hint(),
        golden_path = narrative_research_golden_path(),
        fundamentals = fundamentals_summary,
    ))
}

async fn scalar_ticker(db: &sea_orm::DatabaseConnection) -> Result<String> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT ticker FROM run_metadata WHERE id = 1".to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("read ticker failed: {err}")))?
        .ok_or_else(|| Error::string("run_metadata missing"))?;
    row.try_get::<String>("", "ticker")
        .map_err(|err| Error::string(&format!("parse ticker: {err}")))
}

async fn load_company_name(db: &sea_orm::DatabaseConnection) -> Result<Option<String>> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT company_name FROM stock_info WHERE id = 1".to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("read stock_info failed: {err}")))?;
    Ok(row.and_then(|row| row.try_get::<String>("", "company_name").ok()))
}

async fn fundamentals_summary(workspace: &WorkspaceHandle) -> Result<String> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
    let rows = workspace
        .connection()
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT metric_key, metric_value, period FROM fundamentals
             WHERE metric_key IN (
                'revenue_ttm','net_income_ttm','eps_ttm','current_price','market_cap','net_margin'
             )
             ORDER BY metric_key"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("fundamentals query failed: {err}")))?;

    if rows.is_empty() {
        return Ok("(no fundamentals rows yet)".to_string());
    }

    let lines: Result<Vec<String>> = rows
        .into_iter()
        .map(|row| {
            let key = row
                .try_get::<String>("", "metric_key")
                .map_err(|err| Error::string(&format!("parse metric_key: {err}")))?;
            let value = row
                .try_get::<f64>("", "metric_value")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "n/a".to_string());
            let period = row
                .try_get::<String>("", "period")
                .unwrap_or_else(|_| "n/a".to_string());
            Ok(format!("- {key}: {value} ({period})"))
        })
        .collect();
    lines.map(|items| items.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_incremental_tool_names() {
        let prompt = build_user_prompt("MSFT", Some("Microsoft"), "- revenue_ttm: 100")
            .expect("prompt");
        assert!(prompt.contains("capture_narrative_side"));
        assert!(prompt.contains("finalize_narrative_research"));
    }
}
