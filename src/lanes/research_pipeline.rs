use super::{
    financial_mechanics_experiments::FinancialMechanicsExperimentsLane,
    identify_crux_candidates::IdentifyCruxCandidatesLane, lane::Lane,
};
use std::sync::Arc;

/// Financial analysis lanes (worker lanes 4–5) for the linear research path.
pub fn financial_analysis_lanes() -> Vec<Arc<dyn Lane>> {
    vec![
        Arc::new(IdentifyCruxCandidatesLane::default()),
        Arc::new(FinancialMechanicsExperimentsLane::default()),
    ]
}
