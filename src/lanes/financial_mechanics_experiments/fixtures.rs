#![cfg(test)]

use crate::{
    lanes::context::LaneContext,
    agents::financial_model_explorer::types::{
        AnalysisExperimentInput, AnalysisInputRef, AnalysisOutputRow,
    },
    services::financial_analysis_store::FinancialAnalysisStore,
};
use loco_rs::prelude::*;

pub async fn persist_fixture_experiment(ctx: &LaneContext) -> Result<()> {
    let store = FinancialAnalysisStore::new(ctx.workspace.connection());
    store
        .insert_analysis_run(
            "fixture_run",
            store
                .load_crux_id_by_key("rpo_conversion")
                .await?
                .or(Some(1)),
            "Does capex outpace operating cash flow?",
            "SELECT 1",
            "FY2025",
            "success",
            Some(1),
            None,
            "[]",
            "[]",
            r#"[{"input_type":"concept","taxonomy":"us-gaap","concept_name":"PaymentsToAcquirePropertyPlantAndEquipment","unit":"USD"}]"#,
            "2026-06-09T00:00:00Z",
            None,
        )
        .await?;

    let experiment = AnalysisExperimentInput {
        experiment_key: "capex_ocf_pressure".to_string(),
        question: "Does capex outpace operating cash flow?".to_string(),
        purpose: "historical_investigation".to_string(),
        crux_key: Some("rpo_conversion".to_string()),
        sql_body: "SELECT 1".to_string(),
        period_basis: "FY2025".to_string(),
        disposition: "promoted".to_string(),
        rejection_reason: None,
        source_note: None,
        rationale: Some("Fixture experiment".to_string()),
        assumptions: vec![],
        inputs: vec![AnalysisInputRef {
            input_type: "concept".to_string(),
            taxonomy: Some("us-gaap".to_string()),
            concept_name: Some("PaymentsToAcquirePropertyPlantAndEquipment".to_string()),
            unit: Some("USD".to_string()),
            canonical_key: None,
            note: None,
        }],
        outputs: vec![
            AnalysisOutputRow {
                kind: "ratio".to_string(),
                label: "Capex / OCF".to_string(),
                value: Some(1.2),
                unit: Some("ratio".to_string()),
                period_end: None,
                formula: None,
                text: None,
            },
            AnalysisOutputRow {
                kind: "interpretation".to_string(),
                label: "Read".to_string(),
                value: None,
                unit: None,
                period_end: None,
                formula: None,
                text: Some("Binding constraint".to_string()),
            },
        ],
        bridge: None,
    };

    store
        .finalize_analysis_run(
            "fixture_run",
            &experiment,
            "fixture",
            "2026-06-09T00:00:00Z",
            None,
        )
        .await?;
    Ok(())
}
