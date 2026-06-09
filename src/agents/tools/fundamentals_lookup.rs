use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;

pub const TOOL_NAME: &str = "fundamentals_lookup";

/// Placeholder for a future freely-available fundamentals API tool.
pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Look up headline fundamental metrics for a public company from a curated fundamentals source",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "ticker": {
                    "type": "string",
                    "description": "Stock ticker symbol"
                },
                "metrics": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Requested metric keys such as revenue, gross_margin, or net_debt"
                }
            },
            "required": ["ticker", "metrics"]
        }))
        .build()
        .expect("fundamentals_lookup tool definition should be valid")
}

pub async fn execute(_arguments: &str) -> Result<String> {
    Err(Error::string("fundamentals_lookup is not implemented yet"))
}
