use super::financial_fan_out::FinancialFanOutLane;
use std::sync::Arc;

pub fn financial_analysis_lanes() -> Vec<Arc<dyn super::lane::Lane>> {
    vec![Arc::new(FinancialFanOutLane::new())]
}
