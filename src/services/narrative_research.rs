use crate::{
    agents::{
        narrative_researcher::{
            AGENT_PREAMBLE, narrative_research_golden_path, workspace_schema_hint,
        },
        tools::WebSearchConfig,
    },
    services::{
        model_client::{ModelClient, ModelRequest},
        narrative_research_store::NarrativeResearchStore,
        workspace_store::WorkspaceHandle,
    },
};
use loco_rs::prelude::*;
use std::{collections::BTreeMap, path::PathBuf};

pub const DEFAULT_NARRATIVE_MODEL: &str = "deepseek/deepseek-v4-flash";

#[derive(Debug, Clone)]
pub struct NarrativeResearchService {
    pub model: String,
    pub enable_web_search: bool,
    pub workspace_sqlite: PathBuf,
    pub company_label: Option<String>,
}

impl Default for NarrativeResearchService {
    fn default() -> Self {
        Self {
            model: DEFAULT_NARRATIVE_MODEL.to_string(),
            enable_web_search: true,
            workspace_sqlite: PathBuf::new(),
            company_label: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NarrativeResearchRunResult {
    pub worker_run_id: Option<i64>,
    pub source_count: i64,
    pub claim_count: i64,
    pub crux_count: i64,
}

impl NarrativeResearchService {
    pub fn agent_defaults(workspace_sqlite: PathBuf) -> Self {
        Self {
            workspace_sqlite,
            ..Self::default()
        }
    }

    pub async fn run_on_workspace(
        &self,
        client: &dyn ModelClient,
        workspace: &WorkspaceHandle,
    ) -> Result<NarrativeResearchRunResult> {
        NarrativeResearchStore::clear_narrative_state(workspace.connection()).await?;

        let ticker = scalar_ticker(workspace.connection()).await?;
        let company_name = load_company_name(workspace.connection()).await?;
        let fundamentals_summary = fundamentals_summary(workspace).await?;

        let response = client
            .complete(ModelRequest {
                model: self.model.clone(),
                preamble: AGENT_PREAMBLE.to_string(),
                prompt: self.build_prompt(&ticker, company_name.as_deref(), &fundamentals_summary)?,
                json_mode: false,
                metadata: BTreeMap::from([(
                    "worker_lane".to_string(),
                    "narrative_research".to_string(),
                )]),
                web_search: self
                    .enable_web_search
                    .then(WebSearchConfig::concept_validation_defaults),
                workspace_sqlite: Some(self.workspace_sqlite.clone()),
                client_tools: None,
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
        Ok(NarrativeResearchRunResult {
            worker_run_id: response.worker_run_id,
            source_count: snapshot.source_count,
            claim_count: snapshot.claim_count,
            crux_count: snapshot.crux_count,
        })
    }

    pub fn build_prompt(
        &self,
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
        let service = NarrativeResearchService::default();
        let prompt = service
            .build_prompt("MSFT", Some("Microsoft"), "- revenue_ttm: 100")
            .expect("prompt");
        assert!(prompt.contains("capture_narrative_side"));
        assert!(prompt.contains("finalize_narrative_research"));
    }
}
