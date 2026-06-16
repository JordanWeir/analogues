#![cfg(test)]

use crate::{
    agents::financial_model_explorer::types::{
        AnalysisExperimentInput, AnalysisInputRef, AnalysisOutputRow,
    },
    lanes::context::LaneContext,
    services::financial_analysis_store::FinancialAnalysisStore,
};
use loco_rs::prelude::*;

async fn persist_promoted_experiment(
    store: &FinancialAnalysisStore<'_>,
    run_key: &str,
    experiment_key: &str,
    purpose: &str,
    crux_key: &str,
    question: &str,
) -> Result<()> {
    store
        .insert_analysis_run(
            run_key,
            store.load_crux_id_by_key(crux_key).await?.or(Some(1)),
            question,
            "SELECT 1",
            "annual",
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
        experiment_key: experiment_key.to_string(),
        question: question.to_string(),
        purpose: purpose.to_string(),
        crux_key: Some(crux_key.to_string()),
        sql_body: "SELECT 1".to_string(),
        period_basis: "annual".to_string(),
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
                label: "Fixture ratio".to_string(),
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
            run_key,
            &experiment,
            "fixture",
            "2026-06-09T00:00:00Z",
            None,
        )
        .await?;
    Ok(())
}

pub async fn persist_fixture_experiment(ctx: &LaneContext) -> Result<()> {
    let store = FinancialAnalysisStore::new(ctx.workspace.connection());
    persist_promoted_experiment(
        &store,
        "fixture_run_historical",
        "capex_ocf_pressure",
        "historical_investigation",
        "rpo_conversion",
        "Does capex outpace operating cash flow?",
    )
    .await?;
    persist_promoted_experiment(
        &store,
        "fixture_run_sensitivity",
        "rpo_conversion_sensitivity",
        "sensitivity",
        "capex_funding",
        "What conversion rate funds guided capex?",
    )
    .await?;
    Ok(())
}
