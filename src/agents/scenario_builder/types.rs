use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioBuilderMode {
    Blueprint,
    Detail,
}

impl ScenarioBuilderMode {
    pub fn worker_lane(self) -> &'static str {
        match self {
            Self::Blueprint => "scenario_generation",
            Self::Detail => "scenario_generation",
        }
    }

    pub fn submit_tool_name(self) -> &'static str {
        match self {
            Self::Blueprint => "submit_scenario_blueprint",
            Self::Detail => "submit_scenario_detail",
        }
    }

    pub fn mode_label(self) -> &'static str {
        match self {
            Self::Blueprint => "scenario_blueprint",
            Self::Detail => "scenario_detail",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioProjectionCalendarSpec {
    pub forward_quarters: usize,
    #[serde(default)]
    pub historical_quarters: Option<usize>,
}

impl ScenarioProjectionCalendarSpec {
    pub fn historical_quarters(&self) -> usize {
        self.historical_quarters
            .unwrap_or(HISTORICAL_QUARTERS_TARGET)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioBlueprintOutput {
    pub scenarios: Vec<ScenarioBlueprint>,
    pub projection_calendar: ScenarioProjectionCalendarSpec,
    #[serde(default)]
    pub projection_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioBlueprint {
    pub scenario_key: String,
    pub name: String,
    pub stance: String,
    pub probability: f64,
    pub description: String,
    #[serde(default)]
    pub crux_resolution_summary: String,
    #[serde(default)]
    pub linked_crux_keys: Vec<String>,
    #[serde(default)]
    pub linked_experiment_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDetailOutput {
    pub scenario_key: String,
    pub assumption_summary: String,
    #[serde(default)]
    pub crux_assumptions: Vec<ScenarioCruxAssumptionInput>,
    #[serde(default)]
    pub sensitivities: Vec<String>,
    #[serde(default)]
    pub confirming_signals: Vec<String>,
    #[serde(default)]
    pub breaking_signals: Vec<String>,
    pub periods: Vec<ScenarioPeriodInput>,
    #[serde(default)]
    pub per_worker: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioCruxAssumptionInput {
    pub crux_key: String,
    pub crux: String,
    pub assumption: String,
    #[serde(default)]
    pub impact: Option<String>,
    #[serde(default)]
    pub experiment_key: Option<String>,
    #[serde(default)]
    pub source_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioPeriodInput {
    pub period_order: i64,
    pub label: String,
    pub period_end: String,
    #[serde(default = "default_quarter")]
    pub period_type: String,
    #[serde(default)]
    pub revenue: Option<f64>,
    #[serde(default)]
    pub revenue_growth: Option<f64>,
    #[serde(default)]
    pub diluted_shares: Option<f64>,
    #[serde(default)]
    pub gross_margin: Option<f64>,
    #[serde(default)]
    pub operating_margin: Option<f64>,
    #[serde(default)]
    pub net_margin: Option<f64>,
    #[serde(default)]
    pub net_income: Option<f64>,
    #[serde(default)]
    pub eps: Option<f64>,
    #[serde(default)]
    pub ps_low: Option<f64>,
    #[serde(default)]
    pub ps_median: Option<f64>,
    #[serde(default)]
    pub ps_high: Option<f64>,
    #[serde(default)]
    pub pe_low: Option<f64>,
    #[serde(default)]
    pub pe_median: Option<f64>,
    #[serde(default)]
    pub pe_high: Option<f64>,
    #[serde(default = "default_blend_weight")]
    pub blend_ps_weight: f64,
    #[serde(default = "default_blend_weight")]
    pub blend_pe_weight: f64,
    #[serde(default)]
    pub source_note: Option<String>,
}

fn default_quarter() -> String {
    "quarter".to_string()
}

fn default_blend_weight() -> f64 {
    0.5
}

pub const SCENARIO_BLUEPRINT_MIN: usize = 4;
pub const SCENARIO_BLUEPRINT_MAX: usize = 6;
pub const SCENARIOS_TARGET: usize = 5;
pub const HISTORICAL_QUARTERS_TARGET: usize = 4;
pub const FORWARD_QUARTERS_MIN: usize = 12;
pub const FORWARD_QUARTERS_MAX: usize = 20;
pub const MIN_TOTAL_QUARTERLY_PERIODS: usize = 16;
