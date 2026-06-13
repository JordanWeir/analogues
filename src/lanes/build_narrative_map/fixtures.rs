#![cfg(test)]

use crate::{
    agents::tools::narrative_research::{
        self, TOOL_CAPTURE_CLAIMS, TOOL_CAPTURE_NARRATIVE_ITEMS, TOOL_CAPTURE_NARRATIVE_SIDE,
        TOOL_CAPTURE_ORIENTATION, TOOL_CAPTURE_SECTION, TOOL_CAPTURE_SOURCES, TOOL_FINALIZE,
    },
    lanes::{
        build_catalog::BuildCatalogLane, context::LaneConfig, context::LaneContext, lane::Lane,
    },
    services::{
        workspace_financial_store::{RawIngestPersist, WorkspaceFinancialStore},
        workspace_store::{execute_schema, WorkspaceStore},
    },
    workspace::{seed_database, AvRawFact, InitWorkspaceRequest, WorkspacePaths},
};
use sea_orm::Database;
use serde_json::json;
use std::path::PathBuf;

fn sample_av_facts() -> Vec<AvRawFact> {
    vec![
        AvRawFact {
            endpoint: "INCOME_STATEMENT".to_string(),
            report_type: "annual".to_string(),
            field_name: "totalRevenue".to_string(),
            label: None,
            period_end: "2025-12-31".to_string(),
            period_type: "annual".to_string(),
            unit: "USD".to_string(),
            currency: Some("USD".to_string()),
            value: 100.0,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-09T00:00:00Z".to_string(),
        },
        AvRawFact {
            endpoint: "INCOME_STATEMENT".to_string(),
            report_type: "annual".to_string(),
            field_name: "netIncome".to_string(),
            label: None,
            period_end: "2025-12-31".to_string(),
            period_type: "annual".to_string(),
            unit: "USD".to_string(),
            currency: Some("USD".to_string()),
            value: 10.0,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-09T00:00:00Z".to_string(),
        },
        AvRawFact {
            endpoint: "OVERVIEW".to_string(),
            report_type: "overview".to_string(),
            field_name: "DilutedEPSTTM".to_string(),
            label: None,
            period_end: "2025-12-31".to_string(),
            period_type: "ttm".to_string(),
            unit: "USD".to_string(),
            currency: Some("USD".to_string()),
            value: 1.25,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-09T00:00:00Z".to_string(),
        },
    ]
}

pub async fn catalog_lane_context() -> (LaneContext, PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "analogues-narrative-fixture-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
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

    WorkspaceFinancialStore::new(&db)
        .persist_raw_ingest(&RawIngestPersist {
            fetched_at: "2026-06-09T00:00:00Z",
            company_name: Some("Example Corp"),
            currency: Some("USD"),
            source_note: "fixture",
            raw_av_facts: &sample_av_facts(),
            raw_sec_facts: &[],
        })
        .await
        .expect("persist");
    db.close().await.ok();

    let workspace = WorkspaceStore.open_workspace(&path).await.expect("open");
    let mut ctx = LaneContext::new(workspace, LaneConfig::new("EXMP"));
    BuildCatalogLane::new(crate::lanes::build_catalog::CatalogResolutionStrategy::Deterministic)
        .run(&mut ctx)
        .await
        .expect("catalog");
    ctx.workspace.close().await.ok();

    let workspace = WorkspaceStore.open_workspace(&path).await.expect("reopen");
    (LaneContext::new(workspace, LaneConfig::new("EXMP")), path)
}

pub async fn populate_fixture_narrative(path: &PathBuf) {
    let long = "x".repeat(90);
    let sources = json!({
        "sources": [
            {"title": "Example 10-K", "url": "https://example.com/10k", "source_type": "Filing", "why_it_matters": "Primary audited financial disclosure for baseline facts."},
            {"title": "Earnings call transcript", "source_type": "Transcript", "why_it_matters": "Management commentary on demand trends and guidance."},
            {"title": "Sell-side debate note", "source_type": "Market commentary", "why_it_matters": "Captures bull and bear framing in one place."},
            {"title": "Official Q1 press release", "url": "https://example.com/q1-release", "source_type": "Official company source", "why_it_matters": "Latest-quarter official revenue and guidance figures."},
            {"title": "Financial news recap", "url": "https://example.com/news-recap", "source_type": "Financial news", "why_it_matters": "Summarizes market reaction to the latest earnings print."}
        ]
    })
    .to_string();
    narrative_research::execute(path, TOOL_CAPTURE_SOURCES, &sources)
        .await
        .expect("sources");

    let claims = json!({
        "claims": [
            {"claim": "Revenue growth re-accelerated in the latest quarter.", "source_id": 1, "claim_type": "revenue growth", "side": "bull", "confidence": "high"},
            {"claim": "Margin pressure from mix shift remains a risk.", "source_id": 2, "claim_type": "margin", "side": "bear", "confidence": "medium"},
            {"claim": "Valuation embeds optimistic AI monetization.", "source_id": 3, "claim_type": "valuation", "side": "bear", "confidence": "medium"},
            {"claim": "Balance sheet supports continued buybacks.", "source_id": 1, "claim_type": "capital allocation", "side": "bull", "confidence": "high"},
            {"claim": "Consensus assumes stable enterprise demand.", "source_id": 3, "claim_type": "demand", "side": "consensus", "confidence": "medium"},
            {"claim": "Cloud backlog conversion is accelerating.", "source_id": 4, "claim_type": "demand", "side": "bull", "confidence": "high"},
            {"claim": "Capex intensity may pressure free cash flow.", "source_id": 2, "claim_type": "capital allocation", "side": "bear", "confidence": "medium"},
            {"claim": "Operating margin held steady year over year.", "source_id": 1, "claim_type": "margin", "side": "bull", "confidence": "high"},
            {"claim": "Customer concentration remains elevated.", "source_id": 5, "claim_type": "customer concentration", "side": "bear", "confidence": "inference"},
            {"claim": "Latest quarter EPS beat consensus.", "source_id": 4, "claim_type": "earnings", "side": "bull", "confidence": "high"}
        ]
    })
    .to_string();
    narrative_research::execute(path, TOOL_CAPTURE_CLAIMS, &claims)
        .await
        .expect("claims");

    for (side, suffix) in [
        ("bull", "Bull case emphasizes durable growth."),
        ("bear", "Bear case emphasizes valuation and risk."),
        ("dominant", "Market is debating growth durability."),
        ("consensus", "Consensus expects steady execution."),
    ] {
        let args = json!({ "side": side, "body": format!("{long} {suffix}") }).to_string();
        narrative_research::execute(path, TOOL_CAPTURE_NARRATIVE_SIDE, &args)
            .await
            .expect(side);
    }

    let cruxes = json!({
        "item_type": "crux",
        "items": [
            "Whether cloud consumption growth re-accelerates through FY26.",
            "Whether margin expansion offsets heavier AI infrastructure spend.",
            "Whether backlog converts to revenue on management's timeline.",
            "Whether customer concentration creates binary demand risk.",
            "Whether financing costs stay manageable during the capex ramp."
        ]
    })
    .to_string();
    narrative_research::execute(path, TOOL_CAPTURE_NARRATIVE_ITEMS, &cruxes)
        .await
        .expect("cruxes");

    let agreements = json!({
        "item_type": "agreement",
        "items": [
            "Both sides agree cloud is now the primary growth engine.",
            "Both sides agree capex is rising materially this cycle."
        ]
    })
    .to_string();
    narrative_research::execute(path, TOOL_CAPTURE_NARRATIVE_ITEMS, &agreements)
        .await
        .expect("agreements");

    let orientation = json!({
        "dominant_question": "Is growth re-acceleration already priced in?",
        "current_setup": "Shares trade near recent highs after strong results.",
        "time_horizon": "12-18 months"
    })
    .to_string();
    narrative_research::execute(path, TOOL_CAPTURE_ORIENTATION, &orientation)
        .await
        .expect("orientation");

    for section_key in ["business_model", "why_now"] {
        let args = json!({
            "section_key": section_key,
            "body": format!("{} section body with enough detail for downstream readers.", section_key)
        })
        .to_string();
        narrative_research::execute(path, TOOL_CAPTURE_SECTION, &args)
            .await
            .expect(section_key);
    }

    narrative_research::execute(path, TOOL_FINALIZE, "{}")
        .await
        .expect("finalize");
}
