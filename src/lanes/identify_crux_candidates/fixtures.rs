#![cfg(test)]

use crate::{
    agents::financial_model_explorer::types::{
        ClusterMemberInput, CruxCandidateInput, CruxTriageOutput, SupportingMetricPromotion,
    },
    lanes::context::LaneContext,
    services::financial_analysis_store::FinancialAnalysisStore,
};
use loco_rs::prelude::*;

pub async fn persist_fixture_cruxes(ctx: &LaneContext) -> Result<()> {
    let store = FinancialAnalysisStore::new(ctx.workspace.connection());
    let output = CruxTriageOutput {
        cruxes: vec![
            CruxCandidateInput {
                crux_key: "rpo_conversion".to_string(),
                title: "RPO conversion".to_string(),
                statement: "Backlog must convert fast enough to fund capex.".to_string(),
                bridge_archetype: Some("backlog_to_cash_conversion".to_string()),
                narrative_side: Some("bear".to_string()),
                watch_condition: "RPO/revenue trend".to_string(),
                confirming_signal: "OCF lags capex".to_string(),
                breaking_signal: "OCF keeps pace with capex".to_string(),
                disposition: "promoted".to_string(),
                rationale: "Core mechanic".to_string(),
                limitations: None,
                cluster_members: vec![ClusterMemberInput {
                    taxonomy: "us-gaap".to_string(),
                    concept_name: "RevenueRemainingPerformanceObligation".to_string(),
                    unit: "USD".to_string(),
                    role: "driver".to_string(),
                    dominant_period_shape: Some("instant".to_string()),
                }],
                linked_claim_ids: vec![],
            },
            CruxCandidateInput {
                crux_key: "capex_funding".to_string(),
                title: "Capex funding pressure".to_string(),
                statement: "Capex may exceed internal cash generation.".to_string(),
                bridge_archetype: Some("capex_to_funding_pressure".to_string()),
                narrative_side: Some("bear".to_string()),
                watch_condition: "Capex/OCF ratio".to_string(),
                confirming_signal: "Negative FCF persists".to_string(),
                breaking_signal: "FCF turns positive".to_string(),
                disposition: "promoted".to_string(),
                rationale: "Funding bridge".to_string(),
                limitations: None,
                cluster_members: vec![ClusterMemberInput {
                    taxonomy: "us-gaap".to_string(),
                    concept_name: "PaymentsToAcquirePropertyPlantAndEquipment".to_string(),
                    unit: "USD".to_string(),
                    role: "driver".to_string(),
                    dominant_period_shape: Some("annual".to_string()),
                }],
                linked_claim_ids: vec![],
            },
        ],
        supporting_metrics: vec![
            SupportingMetricPromotion {
                selection_scope: "crux_support".to_string(),
                crux_key: Some("rpo_conversion".to_string()),
                taxonomy: "us-gaap".to_string(),
                concept_name: "RevenueRemainingPerformanceObligation".to_string(),
                unit: "USD".to_string(),
                label: None,
                rationale: "Backlog driver".to_string(),
                period_basis: Some("instant".to_string()),
                quality_status: Some("ok".to_string()),
            },
            SupportingMetricPromotion {
                selection_scope: "crux_support".to_string(),
                crux_key: Some("capex_funding".to_string()),
                taxonomy: "us-gaap".to_string(),
                concept_name: "PaymentsToAcquirePropertyPlantAndEquipment".to_string(),
                unit: "USD".to_string(),
                label: None,
                rationale: "Capex driver".to_string(),
                period_basis: Some("annual".to_string()),
                quality_status: Some("ok".to_string()),
            },
        ],
        quality_flags: vec![],
        open_questions: vec![],
    };
    store
        .persist_crux_triage(&output, "fixture", "2026-06-09T00:00:00Z", None)
        .await?;
    Ok(())
}
