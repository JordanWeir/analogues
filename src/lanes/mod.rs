pub mod build_catalog;
pub mod build_narrative_map;
pub mod context;
pub mod financial_fan_out;
pub mod financial_mechanics_experiments;
pub mod gate;
pub mod identify_crux_candidates;
pub mod init_pipeline;
pub mod init_workspace;
pub mod research_pipeline;
pub mod scenario_artifacts;
pub mod scenario_generation;
pub mod lane;
pub mod result;
pub mod runner;

pub use context::{LaneConfig, LaneContext};
pub use gate::{Gate, GateResult, GateStatus};
pub use init_pipeline::lanes_for_request;
pub use research_pipeline::financial_analysis_lanes;
pub use scenario_artifacts::ScenarioArtifactsLane;
pub use scenario_generation::ScenarioGenerationLane;
pub use lane::Lane;
pub use result::{LaneResult, LaneStatus, LaneWritesSummary, LinearRunReport};
pub use runner::LinearRunner;

#[cfg(test)]
mod tests;
