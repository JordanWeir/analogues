use super::{
    financial_mechanics_experiments::{
        strategy::FinancialMechanicsExperimentsStrategy, FinancialMechanicsExperimentsLane,
    },
    identify_crux_candidates::{
        strategy::IdentifyCruxCandidatesStrategy, IdentifyCruxCandidatesLane,
    },
    lane::Lane,
};
use std::sync::Arc;

/// Financial analysis lanes (worker lanes 4–5) for the linear research path.
pub fn financial_analysis_lanes() -> Vec<Arc<dyn Lane>> {
    vec![
        Arc::new(IdentifyCruxCandidatesLane::new(
            IdentifyCruxCandidatesStrategy::agent_defaults(),
        )),
        Arc::new(FinancialMechanicsExperimentsLane::new(
            FinancialMechanicsExperimentsStrategy::agent_defaults(),
        )),
    ]
}
