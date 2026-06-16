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
        narrative_research_store::NarrativeResearchStore, workspace_store::WorkspaceHandle,
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
        let store = NarrativeResearchStore::new(workspace.connection());
        let existing_context = store.load_existing_context().await?;

        let ticker = scalar_ticker(workspace.connection()).await?;
        let company_name = load_company_name(workspace.connection()).await?;
        let fundamentals_summary = fundamentals_summary(workspace).await?;

        let mut tools = ToolRegistry::new()
            .with_sql_query(workspace_sqlite.clone())
            .with_narrative_research();
        if self.config.enable_web_search {
            tools = tools.with_web_search(WebSearchConfig::concept_validation_defaults());
        }

        // @TODO: Add a custom `prepare_step` hook that injects a task-status checklist (finished vs pending
        // capture_* tools) using `ctx.steps` and workspace snapshot counts.
        let response = ToolLoopAgent::default()
            .run(ToolLoopRequest {
                worker_name: WORKER_NAME.to_string(),
                model: self.config.model.clone(),
                preamble: AGENT_PREAMBLE.to_string(),
                prompt: build_user_prompt(
                    &ticker,
                    company_name.as_deref(),
                    &fundamentals_summary,
                    &existing_context,
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
                prepare_step: None,
                stop_when: None,
            })
            .await?;

        let outcome = store.finalize().await.map_err(|err| {
            let preview: String = response.text.chars().take(300).collect();
            Error::string(&format!(
                "{err}; agent finalize incomplete (preview: {preview})"
            ))
        })?;

        Ok((
            NarrativeResearchRunResult {
                worker_run_id: response.worker_run_id,
                source_count: outcome.snapshot.source_count,
                claim_count: outcome.snapshot.claim_count,
                crux_count: outcome.snapshot.crux_count,
            },
            response,
        ))
    }
}

pub fn build_user_prompt(
    ticker: &str,
    company_name: Option<&str>,
    fundamentals_summary: &str,
    existing_context: &serde_json::Value,
) -> Result<String> {
    let company = company_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(ticker);
    let existing_json = serde_json::to_string_pretty(existing_context).map_err(|err| {
        Error::string(&format!(
            "failed to serialize existing narrative context: {err}"
        ))
    })?;
    let has_existing = existing_context
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|sources| !sources.is_empty())
        || existing_context
            .get("claims")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|claims| !claims.is_empty());

    let existing_guidance = if has_existing {
        "Review what is already on the board. Reuse valid source ids, supersede stale claims when a newer quarter changes headline metrics (note superseded claim ids in capture_claims.notes), update narrative sides for the current catalyst quarter, and add net-new sources/claims/items where gaps remain."
    } else {
        "The narrative board is empty — build the initial source pack and narrative map for the current catalyst quarter."
    };

    Ok(format!(
        r#"Maintain the source pack and narrative map for {company} ({ticker}).

{existing_guidance}

{schema}

{golden_path}

## Existing narrative board (durable state — reconcile stale rows, do not blindly duplicate)
```json
{existing_json}
```

## Workspace fundamentals snapshot (starter TTM + latest observations — prefer observations when newer)
{fundamentals}

## Incremental capture tools
- capture_sources — add NEW sources (duplicates return existing id); use real citeable urls
- capture_claims — add claims for the current quarter; note superseded claim ids in notes when correcting stale metrics
- capture_narrative_side — UPDATE bull/bear/dominant/consensus for the current catalyst
- capture_narrative_items — add agreements and cruxes (minimum gates require several of each)
- capture_orientation — update orientation for the current catalyst quarter
- capture_section — update business_model or why_now when needed
- capture_research_gap — record filing-lag and unresolved questions (required when SEC facts trail the market)
- finalize_narrative_research — validate when the board is ready (≥10 claims, ≥5 cruxes, ≥2 bear claims, ≥5 sources, ≥1 agreement, plus sides and sections)

Each capture tool returns a workspace snapshot. Fix gaps before finalize."#,
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

// @TODO: Pull from catalog manager flagged metrics once that lane exposes them.
async fn fundamentals_summary(workspace: &WorkspaceHandle) -> Result<String> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let conn = workspace.connection();
    let mut sections = Vec::new();

    let fundamentals_sql = "SELECT metric_key, metric_value, period FROM fundamentals
         WHERE metric_key IN (
            'revenue_ttm','net_income_ttm','eps_ttm','current_price','market_cap','net_margin','total_debt'
         )
         ORDER BY metric_key";
    let rows = conn
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            fundamentals_sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("fundamentals query failed: {err}")))?;

    if rows.is_empty() {
        sections.push("(no fundamentals rows yet)".to_string());
    } else {
        let mut lines = Vec::from(["Headline fundamentals (TTM — may lag latest quarter):".to_string()]);
        for row in rows {
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
            lines.push(format!("- {key}: {value} ({period})"));
        }
        sections.push(lines.join("\n"));
    }

    let freshness_sql = "SELECT
            (SELECT MAX(fetched_at) FROM av_raw_facts) AS max_av_fetched_at,
            (SELECT MAX(period_end) FROM av_raw_facts) AS max_av_period_end,
            (SELECT MAX(filed_at) FROM sec_raw_facts) AS max_sec_filed_at,
            (SELECT MAX(period_end) FROM sec_raw_facts) AS max_sec_period_end,
            (SELECT MAX(period_end) FROM fundamental_observations WHERE metric_key = 'revenue_quarter') AS max_revenue_quarter_end,
            (SELECT created_at FROM run_metadata WHERE id = 1) AS run_created_at,
            (SELECT financial_fetch_status FROM run_metadata WHERE id = 1) AS financial_fetch_status";
    if let Some(row) = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            freshness_sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("freshness query failed: {err}")))?
    {
        sections.push(format!(
            "Filing freshness:\n- max_av_fetched_at: {}\n- max_av_period_end: {}\n- max_sec_filed_at: {}\n- max_sec_period_end: {}\n- max_revenue_quarter_end: {}\n- run_created_at: {}\n- financial_fetch_status: {}",
            row.try_get::<String>("", "max_av_fetched_at").unwrap_or_else(|_| "n/a".to_string()),
            row.try_get::<String>("", "max_av_period_end").unwrap_or_else(|_| "n/a".to_string()),
            row.try_get::<String>("", "max_sec_filed_at").unwrap_or_else(|_| "n/a".to_string()),
            row.try_get::<String>("", "max_sec_period_end").unwrap_or_else(|_| "n/a".to_string()),
            row.try_get::<String>("", "max_revenue_quarter_end").unwrap_or_else(|_| "n/a".to_string()),
            row.try_get::<String>("", "run_created_at").unwrap_or_else(|_| "n/a".to_string()),
            row.try_get::<String>("", "financial_fetch_status").unwrap_or_else(|_| "n/a".to_string()),
        ));
    }

    let observations_sql = "SELECT metric_key, metric_value, period_end, filed_at, fiscal_year, fiscal_period
         FROM fundamental_observations
         WHERE metric_key IN (
            'revenue_quarter','net_income_quarter','eps_quarter','cash','debt_current','debt_noncurrent'
         )
         ORDER BY period_end DESC, metric_key
         LIMIT 24";
    let obs_rows = conn
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            observations_sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("observations query failed: {err}")))?;
    if !obs_rows.is_empty() {
        let mut lines = Vec::from(["Latest fundamental_observations (prefer over TTM for claims):".to_string()]);
        for row in obs_rows {
            let key = row
                .try_get::<String>("", "metric_key")
                .unwrap_or_else(|_| "?".to_string());
            let value = row
                .try_get::<f64>("", "metric_value")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "n/a".to_string());
            let period_end = row
                .try_get::<String>("", "period_end")
                .unwrap_or_else(|_| "n/a".to_string());
            let filed_at = row
                .try_get::<String>("", "filed_at")
                .unwrap_or_else(|_| "n/a".to_string());
            lines.push(format!("- {key}: {value} (period_end {period_end}, filed {filed_at})"));
        }
        sections.push(lines.join("\n"));
    }

    let gaps_sql = "SELECT gap_key, status FROM data_gaps WHERE status = 'open' ORDER BY id";
    let gap_rows = conn
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            gaps_sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("data_gaps query failed: {err}")))?;
    if !gap_rows.is_empty() {
        let mut lines = Vec::from(["Open data_gaps:".to_string()]);
        for row in gap_rows {
            let gap_key = row
                .try_get::<String>("", "gap_key")
                .unwrap_or_else(|_| "?".to_string());
            lines.push(format!("- {gap_key}"));
        }
        sections.push(lines.join("\n"));
    }

    Ok(sections.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_existing_board_and_tools() {
        let existing = serde_json::json!({
            "sources": [{"id": 1, "title": "10-K"}],
            "claims": [],
            "narrative_map": {},
            "narrative_map_items": {"agreements": [], "cruxes": []},
            "sections": {},
            "research_gaps": []
        });
        let prompt = build_user_prompt("MSFT", Some("Microsoft"), "- revenue_ttm: 100", &existing)
            .expect("prompt");
        assert!(prompt.contains("Existing narrative board"));
        assert!(prompt.contains("\"id\": 1"));
        assert!(prompt.contains("capture_narrative_side"));
        assert!(prompt.contains("finalize_narrative_research"));
    }
}
