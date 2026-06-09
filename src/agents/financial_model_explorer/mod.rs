pub mod golden_path;
pub mod service;
pub mod types;

pub use golden_path::{
    crux_triage_golden_path, explorer_schema_hint, mechanics_experiment_golden_path,
};
pub use service::{
    FinancialModelExplorerService, CRUX_TRIAGE_PREAMBLE, MECHANICS_EXPERIMENT_PREAMBLE,
};
pub use types::{
    AnalysisAssumption, AnalysisBridge, AnalysisDraftInput, AnalysisExperimentInput,
    AnalysisFinalizeInput, AnalysisInputRef, AnalysisOutputRow, ClusterMemberInput,
    CruxCandidateInput, CruxTriageOutput, DataGapInput, ExplorerMode, MechanicsExperimentsComplete,
    QualityFlagInput, SupportingMetricPromotion,
};
