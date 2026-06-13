use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorerMode {
    CruxTriage,
    MechanicsExperiment,
}

impl ExplorerMode {
    pub fn worker_lane(self) -> &'static str {
        match self {
            Self::CruxTriage => "identify_crux_candidates",
            Self::MechanicsExperiment => "financial_mechanics_experiments",
        }
    }

    pub fn submit_tool_name(self) -> &'static str {
        match self {
            Self::CruxTriage => "submit_crux_triage",
            Self::MechanicsExperiment => "submit_mechanics_experiments",
        }
    }

    pub fn mode_label(self) -> &'static str {
        match self {
            Self::CruxTriage => "crux_triage",
            Self::MechanicsExperiment => "mechanics_experiment",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CruxTriageOutput {
    pub cruxes: Vec<CruxCandidateInput>,
    #[serde(default)]
    pub supporting_metrics: Vec<SupportingMetricPromotion>,
    #[serde(default)]
    pub quality_flags: Vec<QualityFlagInput>,
    #[serde(default)]
    pub open_questions: Vec<DataGapInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CruxCandidateInput {
    pub crux_key: String,
    pub title: String,
    pub statement: String,
    #[serde(default)]
    pub bridge_archetype: Option<String>,
    #[serde(default)]
    pub narrative_side: Option<String>,
    pub watch_condition: String,
    pub confirming_signal: String,
    pub breaking_signal: String,
    pub disposition: String,
    pub rationale: String,
    #[serde(default)]
    pub limitations: Option<Vec<String>>,
    #[serde(default)]
    pub cluster_members: Vec<ClusterMemberInput>,
    #[serde(default)]
    pub linked_claim_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMemberInput {
    pub taxonomy: String,
    pub concept_name: String,
    pub unit: String,
    pub role: String,
    #[serde(default)]
    pub dominant_period_shape: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportingMetricPromotion {
    pub selection_scope: String,
    #[serde(default)]
    pub crux_key: Option<String>,
    pub taxonomy: String,
    pub concept_name: String,
    pub unit: String,
    #[serde(default)]
    pub label: Option<String>,
    pub rationale: String,
    #[serde(default)]
    pub period_basis: Option<String>,
    #[serde(default)]
    pub quality_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityFlagInput {
    pub flag_key: String,
    pub severity: String,
    pub description: String,
    #[serde(default)]
    pub metric_key: Option<String>,
    #[serde(default)]
    pub period: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataGapInput {
    pub gap_key: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MechanicsExperimentsComplete {
    #[serde(default)]
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisDraftInput {
    pub run_key: String,
    pub question: String,
    pub sql_body: String,
    pub period_basis: String,
    #[serde(default)]
    pub crux_key: Option<String>,
    #[serde(default)]
    pub assumptions: Vec<AnalysisAssumption>,
    #[serde(default)]
    pub inputs: Vec<AnalysisInputRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisFinalizeInput {
    pub run_key: String,
    pub experiment: AnalysisExperimentInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisExperimentInput {
    pub experiment_key: String,
    pub question: String,
    pub purpose: String,
    #[serde(default)]
    pub crux_key: Option<String>,
    #[serde(default)]
    pub sql_body: String,
    #[serde(default)]
    pub period_basis: String,
    pub disposition: String,
    #[serde(default)]
    pub rejection_reason: Option<String>,
    #[serde(default)]
    pub source_note: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub assumptions: Vec<AnalysisAssumption>,
    #[serde(default)]
    pub inputs: Vec<AnalysisInputRef>,
    #[serde(default)]
    pub outputs: Vec<AnalysisOutputRow>,
    #[serde(default)]
    pub bridge: Option<AnalysisBridge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisAssumption {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisInputRef {
    pub input_type: String,
    #[serde(default)]
    pub taxonomy: Option<String>,
    #[serde(default)]
    pub concept_name: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub canonical_key: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisOutputRow {
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub period_end: Option<String>,
    #[serde(default)]
    pub formula: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisBridge {
    pub archetype: String,
    pub driver: String,
    pub mechanism: String,
    pub outcome: String,
    pub conclusion: String,
}
