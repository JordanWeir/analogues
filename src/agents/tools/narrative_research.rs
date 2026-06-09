use crate::{
    agents::narrative_researcher::types::{
        CaptureClaimInput, CaptureNarrativeItemsInput, CaptureNarrativeSideInput,
        CaptureOrientationInput, CaptureResearchGapInput, CaptureSectionInput, CaptureSourceInput,
    },
    services::{
        narrative_research_store::NarrativeResearchStore,
        openrouter_chat::ClientToolExecuteResult,
        workspace_store,
    },
};
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_CAPTURE_SOURCES: &str = "capture_sources";
pub const TOOL_CAPTURE_CLAIMS: &str = "capture_claims";
pub const TOOL_CAPTURE_NARRATIVE_SIDE: &str = "capture_narrative_side";
pub const TOOL_CAPTURE_NARRATIVE_ITEMS: &str = "capture_narrative_items";
pub const TOOL_CAPTURE_ORIENTATION: &str = "capture_orientation";
pub const TOOL_CAPTURE_SECTION: &str = "capture_section";
pub const TOOL_CAPTURE_RESEARCH_GAP: &str = "capture_research_gap";
pub const TOOL_FINALIZE: &str = "finalize_narrative_research";

pub const NARRATIVE_TOOL_NAMES: &[&str] = &[
    TOOL_CAPTURE_SOURCES,
    TOOL_CAPTURE_CLAIMS,
    TOOL_CAPTURE_NARRATIVE_SIDE,
    TOOL_CAPTURE_NARRATIVE_ITEMS,
    TOOL_CAPTURE_ORIENTATION,
    TOOL_CAPTURE_SECTION,
    TOOL_CAPTURE_RESEARCH_GAP,
    TOOL_FINALIZE,
];

pub fn completion_tools() -> Vec<Tool> {
    vec![
        tool_capture_sources(),
        tool_capture_claims(),
        tool_capture_narrative_side(),
        tool_capture_narrative_items(),
        tool_capture_orientation(),
        tool_capture_section(),
        tool_capture_research_gap(),
        tool_finalize(),
    ]
}

pub async fn execute(
    sqlite_path: &PathBuf,
    tool_name: &str,
    arguments: &str,
) -> Result<ClientToolExecuteResult> {
    let db = NarrativeResearchStore::connect(sqlite_path).await?;
    let payload = match tool_name {
        TOOL_CAPTURE_SOURCES => {
            let input: SourcesPayload = parse_args(arguments)?;
            let result = NarrativeResearchStore::capture_sources(&db, input.sources).await?;
            serde_json::to_string(&result).map_err(map_serialize_err)?
        }
        TOOL_CAPTURE_CLAIMS => {
            let input: ClaimsPayload = parse_args(arguments)?;
            let result = NarrativeResearchStore::capture_claims(&db, input.claims).await?;
            serde_json::to_string(&result).map_err(map_serialize_err)?
        }
        TOOL_CAPTURE_NARRATIVE_SIDE => {
            let input: CaptureNarrativeSideInput = parse_args(arguments)?;
            let result = NarrativeResearchStore::capture_narrative_side(&db, input).await?;
            serde_json::to_string(&result).map_err(map_serialize_err)?
        }
        TOOL_CAPTURE_NARRATIVE_ITEMS => {
            let input: CaptureNarrativeItemsInput = parse_args(arguments)?;
            let result = NarrativeResearchStore::capture_narrative_items(&db, input).await?;
            serde_json::to_string(&result).map_err(map_serialize_err)?
        }
        TOOL_CAPTURE_ORIENTATION => {
            let input: CaptureOrientationInput = parse_args(arguments)?;
            let result = NarrativeResearchStore::capture_orientation(&db, input).await?;
            serde_json::to_string(&result).map_err(map_serialize_err)?
        }
        TOOL_CAPTURE_SECTION => {
            let input: CaptureSectionInput = parse_args(arguments)?;
            let result = NarrativeResearchStore::capture_section(&db, input).await?;
            serde_json::to_string(&result).map_err(map_serialize_err)?
        }
        TOOL_CAPTURE_RESEARCH_GAP => {
            let input: CaptureResearchGapInput = parse_args(arguments)?;
            let result = NarrativeResearchStore::capture_research_gap(&db, input).await?;
            serde_json::to_string(&result).map_err(map_serialize_err)?
        }
        TOOL_FINALIZE => {
            let result = NarrativeResearchStore::finalize(&db).await?;
            let text = serde_json::to_string(&result).map_err(map_serialize_err)?;
            return Ok(ClientToolExecuteResult::Complete(text));
        }
        other => {
            return Err(Error::string(&format!("unknown narrative tool: {other}")));
        }
    };

    Ok(ClientToolExecuteResult::Response(payload))
}

#[derive(Debug, Deserialize)]
struct SourcesPayload {
    sources: Vec<CaptureSourceInput>,
}

#[derive(Debug, Deserialize)]
struct ClaimsPayload {
    claims: Vec<CaptureClaimInput>,
}

fn parse_args<T: for<'de> Deserialize<'de>>(arguments: &str) -> Result<T> {
    serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!(
            "tool arguments were not valid JSON: {err}. Pass a JSON object matching the tool schema."
        ))
    })
}

fn map_serialize_err(err: serde_json::Error) -> Error {
    Error::string(&format!("failed to serialize tool result: {err}"))
}

fn tool_capture_sources() -> Tool {
    Tool::builder()
        .name(TOOL_CAPTURE_SOURCES)
        .description(
            "Add 1–3 citeable sources after a web search or filing discovery round. \
             Returns assigned source ids for capture_claims.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "sources": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "url": { "type": "string" },
                            "source_type": { "type": "string" },
                            "published_at": { "type": "string" },
                            "accessed_at": { "type": "string" },
                            "why_it_matters": { "type": "string" },
                            "notes": { "type": "string" }
                        },
                        "required": ["title", "source_type", "why_it_matters"]
                    }
                }
            },
            "required": ["sources"]
        }))
        .build()
        .expect("capture_sources tool should be valid")
}

fn tool_capture_claims() -> Tool {
    Tool::builder()
        .name(TOOL_CAPTURE_CLAIMS)
        .description(
            "Add 1+ extracted claims linked to prior sources. Use source_id from capture_sources responses.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "claims": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "claim": { "type": "string" },
                            "source_id": { "type": "integer" },
                            "source_title": { "type": "string" },
                            "claim_type": { "type": "string" },
                            "side": { "type": "string" },
                            "confidence": { "type": "string" },
                            "metric": { "type": "string" },
                            "notes": { "type": "string" }
                        },
                        "required": ["claim", "claim_type", "side", "confidence"]
                    }
                }
            },
            "required": ["claims"]
        }))
        .build()
        .expect("capture_claims tool should be valid")
}

fn tool_capture_narrative_side() -> Tool {
    Tool::builder()
        .name(TOOL_CAPTURE_NARRATIVE_SIDE)
        .description(
            "Capture one narrative side at a time: bull, bear, dominant, consensus, or counter_narrative. \
             Research the angle first, then call this tool for that side only.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "side": {
                    "type": "string",
                    "enum": ["dominant", "bull", "bear", "consensus", "counter_narrative"]
                },
                "body": { "type": "string" }
            },
            "required": ["side", "body"]
        }))
        .build()
        .expect("capture_narrative_side tool should be valid")
}

fn tool_capture_narrative_items() -> Tool {
    Tool::builder()
        .name(TOOL_CAPTURE_NARRATIVE_ITEMS)
        .description(
            "Add agreement points or cruxes. Call separately for item_type agreement and crux.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "item_type": { "type": "string", "enum": ["agreement", "crux"] },
                "items": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": ["item_type", "items"]
        }))
        .build()
        .expect("capture_narrative_items tool should be valid")
}

fn tool_capture_orientation() -> Tool {
    Tool::builder()
        .name(TOOL_CAPTURE_ORIENTATION)
        .description("Capture the orientation framing for the report.")
        .parameters(json!({
            "type": "object",
            "properties": {
                "dominant_question": { "type": "string" },
                "current_setup": { "type": "string" },
                "time_horizon": { "type": "string" },
                "base_rate_warning": { "type": "string" }
            },
            "required": ["dominant_question", "current_setup", "time_horizon"]
        }))
        .build()
        .expect("capture_orientation tool should be valid")
}

fn tool_capture_section() -> Tool {
    Tool::builder()
        .name(TOOL_CAPTURE_SECTION)
        .description("Capture business_model or why_now section prose.")
        .parameters(json!({
            "type": "object",
            "properties": {
                "section_key": { "type": "string", "enum": ["business_model", "why_now"] },
                "title": { "type": "string" },
                "body": { "type": "string" }
            },
            "required": ["section_key", "body"]
        }))
        .build()
        .expect("capture_section tool should be valid")
}

fn tool_capture_research_gap() -> Tool {
    Tool::builder()
        .name(TOOL_CAPTURE_RESEARCH_GAP)
        .description("Record an unresolved source or research gap.")
        .parameters(json!({
            "type": "object",
            "properties": {
                "gap_key": { "type": "string" },
                "description": { "type": "string" }
            },
            "required": ["gap_key", "description"]
        }))
        .build()
        .expect("capture_research_gap tool should be valid")
}

fn tool_finalize() -> Tool {
    Tool::builder()
        .name(TOOL_FINALIZE)
        .description(
            "Validate that sources, claims, narratives, cruxes, and early sections are complete. \
             Call when finished capturing. Fix validation errors using other capture tools and call again.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .build()
        .expect("finalize_narrative_research tool should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::workspace_store::execute_schema,
        workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
    };
    use chrono::Utc;
    use sea_orm::Database;
    use std::path::PathBuf;

    async fn test_db() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "analogues-narrative-tools-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        let paths = WorkspacePaths {
            run_slug: "EXMP-2026-06-09-1".to_string(),
            workspace_dir: path.parent().unwrap().to_path_buf(),
            sqlite_path: path.clone(),
            generated_dir: path.parent().unwrap().join("generated"),
        };
        seed_database(
            &db,
            &InitWorkspaceRequest {
                ticker: "EXMP".to_string(),
                date: "2026-06-09".to_string(),
                base_dir: PathBuf::from("reports/stock-narrative-research"),
                fetch_financials: false,
                mapping_strategy: None,
                build_narrative_map: false,
            },
            &paths,
        )
        .await
        .expect("seed");
        db.close().await.ok();
        path
    }

    fn long_text() -> String {
        "x".repeat(90)
    }

    #[tokio::test]
    async fn incremental_capture_tools_finalize_fixture_workspace() {
        let path = test_db().await;

        let sources = json!({
            "sources": [
                {
                    "title": "Example 10-K",
                    "url": "https://example.com/10k",
                    "source_type": "Filing",
                    "why_it_matters": "Primary audited financial disclosure for baseline facts."
                },
                {
                    "title": "Earnings call transcript",
                    "source_type": "Transcript",
                    "why_it_matters": "Management commentary on demand trends and guidance."
                },
                {
                    "title": "Sell-side debate note",
                    "source_type": "Market commentary",
                    "why_it_matters": "Captures bull and bear framing in one place."
                }
            ]
        })
        .to_string();
        let source_result = execute(&path, TOOL_CAPTURE_SOURCES, &sources)
            .await
            .expect("sources");
        assert!(matches!(source_result, ClientToolExecuteResult::Response(_)));

        let claims = json!({
            "claims": [
                {"claim": "Revenue growth re-accelerated in the latest quarter.", "source_id": 1, "claim_type": "revenue growth", "side": "bull", "confidence": "high"},
                {"claim": "Margin pressure from mix shift remains a risk.", "source_id": 2, "claim_type": "margin", "side": "bear", "confidence": "medium"},
                {"claim": "Valuation embeds optimistic AI monetization.", "source_id": 3, "claim_type": "valuation", "side": "bear", "confidence": "medium"},
                {"claim": "Balance sheet supports continued buybacks.", "source_id": 1, "claim_type": "capital allocation", "side": "bull", "confidence": "high"},
                {"claim": "Consensus assumes stable enterprise demand.", "source_id": 3, "claim_type": "demand", "side": "consensus", "confidence": "medium"}
            ]
        })
        .to_string();
        execute(&path, TOOL_CAPTURE_CLAIMS, &claims)
            .await
            .expect("claims");

        let long = long_text();
        for (side, body) in [
            ("bull", format!("{long} Bull case emphasizes durable growth.")),
            ("bear", format!("{long} Bear case emphasizes valuation and risk.")),
            ("dominant", format!("{long} Market is debating growth durability.")),
            ("consensus", format!("{long} Consensus expects steady execution.")),
        ] {
            let args = json!({ "side": side, "body": body }).to_string();
            execute(&path, TOOL_CAPTURE_NARRATIVE_SIDE, &args)
                .await
                .expect(side);
        }

        let cruxes = json!({
            "item_type": "crux",
            "items": [
                "Whether cloud consumption growth re-accelerates through FY26.",
                "Whether margin expansion offsets heavier AI infrastructure spend."
            ]
        })
        .to_string();
        execute(&path, TOOL_CAPTURE_NARRATIVE_ITEMS, &cruxes)
            .await
            .expect("cruxes");

        let orientation = json!({
            "dominant_question": "Is growth re-acceleration already priced in?",
            "current_setup": "Shares trade near recent highs after strong results.",
            "time_horizon": "12-18 months"
        })
        .to_string();
        execute(&path, TOOL_CAPTURE_ORIENTATION, &orientation)
            .await
            .expect("orientation");

        for section_key in ["business_model", "why_now"] {
            let args = json!({
                "section_key": section_key,
                "body": format!("{} section body with enough detail for downstream readers.", section_key)
            })
            .to_string();
            execute(&path, TOOL_CAPTURE_SECTION, &args)
                .await
                .expect(section_key);
        }

        let finalize = execute(&path, TOOL_FINALIZE, "{}")
            .await
            .expect("finalize");
        assert!(matches!(finalize, ClientToolExecuteResult::Complete(_)));
    }
}
