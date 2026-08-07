use crate::{
    agents::scenario_builder::types::ScenarioPeriodSubmitInput,
    services::{
        openrouter_chat::ClientToolExecuteResult,
        scenario_projection_calendar::{load_calendar, validate_period_submit},
        scenario_store::ScenarioStore,
        workspace_store,
    },
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use sea_orm::Database;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "submit_scenario_period";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Submit one quarterly projection period for a scenario. Copy period_order and period_end \
             exactly from the full projection calendar in workspace context. Call once per calendar \
             row in period_order sequence. Forward periods must include ps_median.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "scenario_key": { "type": "string" },
                "period": {
                    "type": "object",
                    "properties": {
                        "period_order": { "type": "integer" },
                        "label": { "type": "string" },
                        "period_end": { "type": "string" },
                        "period_type": { "type": "string" },
                        "revenue": { "type": "number" },
                        "revenue_growth": { "type": "number" },
                        "diluted_shares": { "type": "number" },
                        "gross_margin": { "type": "number" },
                        "operating_margin": { "type": "number" },
                        "net_margin": { "type": "number" },
                        "net_income": { "type": "number" },
                        "eps": { "type": "number" },
                        "ps_low": { "type": "number" },
                        "ps_median": { "type": "number" },
                        "ps_high": { "type": "number" },
                        "pe_low": { "type": "number" },
                        "pe_median": { "type": "number" },
                        "pe_high": { "type": "number" },
                        "blend_ps_weight": {
                            "type": "number",
                            "description": "P/S weight in blended implied price."
                        },
                        "blend_pe_weight": {
                            "type": "number",
                            "description": "P/E weight in blended implied price."
                        },
                        "source_note": { "type": "string" }
                    },
                    "required": ["period_order", "label", "period_end", "period_type"]
                }
            },
            "required": ["scenario_key", "period"]
        }))
        .build()
        .expect("submit_scenario_period tool definition should be valid")
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: ScenarioPeriodSubmitInput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_scenario_period arguments were not valid JSON: {err}"
        ))
    })?;
    let db = Database::connect(workspace_store::sqlite_uri(sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open workspace sqlite: {err}")))?;
    let calendar = load_calendar(&db)
        .await?
        .ok_or_else(|| Error::string("projection calendar missing — run blueprint first"))?;
    validate_period_submit(&calendar, &output.period)?;
    let store = ScenarioStore::new(&db);
    let (submitted, required) = store
        .upsert_period(&output.scenario_key, &output.period)
        .await?;
    db.close().await.ok();

    let next_period_order = calendar
        .periods
        .iter()
        .find(|entry| {
            entry.period_order > output.period.period_order
                && submitted < required
        })
        .map(|entry| entry.period_order);

    let next_step = if submitted >= required {
        "complete_scenario_detail".to_string()
    } else if let Some(order) = next_period_order {
        format!("submit_scenario_period for period_order {order}")
    } else {
        "complete_scenario_detail".to_string()
    };

    let response = json!({
        "status": "accepted",
        "scenario_key": output.scenario_key,
        "period_order": output.period.period_order,
        "periods_submitted": submitted,
        "periods_required": required,
        "next_step": next_step
    });
    Ok(ClientToolExecuteResult::Response(
        serde_json::to_string(&response).map_err(|err| {
            Error::string(&format!("failed to serialize period submit response: {err}"))
        })?,
    ))
}
