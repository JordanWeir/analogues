use crate::{
    agents::scenario_builder::{ScenarioBuilderAgent, types::ScenarioBlueprintOutput},
    services::openrouter_chat::ClientToolExecuteResult,
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "submit_scenario_blueprint";

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Submit 4–6 company-specific scenario blueprints after workspace exploration. \
             Probabilities should sum to ~1.0. Include bullish, neutral, and bearish stances. \
             Link crux_keys and experiment_keys. Include projection_calendar.forward_quarters (12–20); \
             the system aligns historical/terminal period_end dates from AV before detail workers run.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "scenarios": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "scenario_key": { "type": "string" },
                            "name": { "type": "string" },
                            "stance": { "type": "string" },
                            "probability": { "type": "number" },
                            "description": { "type": "string" },
                            "crux_resolution_summary": { "type": "string" },
                            "linked_crux_keys": { "type": "array", "items": { "type": "string" } },
                            "linked_experiment_keys": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["scenario_key", "name", "stance", "probability", "description"]
                    }
                },
                "projection_calendar": {
                    "type": "object",
                    "properties": {
                        "forward_quarters": {
                            "type": "integer",
                            "description": "Forward quarterly periods to project (12–20). Historical anchor comes from AV."
                        },
                        "historical_quarters": {
                            "type": "integer",
                            "description": "Optional trailing historical quarters to anchor (default 4)."
                        }
                    },
                    "required": ["forward_quarters"]
                },
                "projection_notes": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": ["scenarios", "projection_calendar"]
        }))
        .build()
        .expect("submit_scenario_blueprint tool definition should be valid")
}

pub async fn execute(_sqlite_path: &PathBuf, arguments: &str) -> Result<ClientToolExecuteResult> {
    let output: ScenarioBlueprintOutput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "submit_scenario_blueprint arguments were not valid JSON: {err}"
        ))
    })?;
    ScenarioBuilderAgent::validate_blueprint_output(&output)?;
    let text = serde_json::to_string(&output).map_err(|err| {
        Error::string(&format!("failed to serialize accepted blueprint output: {err}"))
    })?;
    Ok(ClientToolExecuteResult::Complete(text))
}
