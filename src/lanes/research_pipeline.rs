use super::{
    financial_fan_out::FinancialFanOutLane, scenario_artifacts::ScenarioArtifactsLane,
    scenario_generation::ScenarioGenerationLane,
};
use std::sync::Arc;

pub fn financial_analysis_lanes() -> Vec<Arc<dyn super::lane::Lane>> {
    vec![Arc::new(FinancialFanOutLane::new())]
}

pub fn scenario_lanes() -> Vec<Arc<dyn super::lane::Lane>> {
    vec![
        Arc::new(ScenarioGenerationLane::new()),
        Arc::new(ScenarioArtifactsLane::new()),
    ]
}
