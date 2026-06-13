pub mod agent;
pub mod config;
pub mod explorer_context;
pub mod golden_path;
pub mod prepare_step;
pub mod types;

pub use agent::{
    FinancialModelExplorerAgent, CRUX_TRIAGE_PREAMBLE, MECHANICS_EXPERIMENT_PREAMBLE,
};
pub use config::FinancialModelExplorerConfig;
pub use golden_path::{
    crux_triage_golden_path, explorer_schema_hint, mechanics_experiment_golden_path,
};
pub use types::{
    AnalysisAssumption, AnalysisBridge, AnalysisDraftInput, AnalysisExperimentInput,
    AnalysisFinalizeInput, AnalysisInputRef, AnalysisOutputRow, ClusterMemberInput,
    CruxCandidateInput, CruxTriageOutput, DataGapInput, ExplorerMode, MechanicsExperimentsComplete,
    QualityFlagInput, SupportingMetricPromotion,
};

pub const WORKER_NAME: &str = "financial_model_explorer";
