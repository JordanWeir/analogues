use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundamentalObservation {
    pub canonical_key: Option<String>,
    pub metric_key: String,
    pub metric_label: String,
    pub statement_type: String,
    pub period_type: String,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub as_of_date: Option<String>,
    pub filed_at: Option<String>,
    pub fiscal_year: Option<i64>,
    pub fiscal_period: Option<String>,
    pub value: f64,
    pub unit: Option<String>,
    pub source_type: String,
    pub source_note: Option<String>,
    pub concept_name: Option<String>,
    pub form: Option<String>,
    pub accession: Option<String>,
    pub quality: Option<String>,
    pub is_derived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecRawFact {
    pub taxonomy: String,
    pub concept_name: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub unit: String,
    pub form: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub filed: Option<String>,
    pub fiscal_year: Option<i64>,
    pub fiscal_period: Option<String>,
    pub accession: Option<String>,
    pub frame: Option<String>,
    pub value: f64,
    pub raw_json: String,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMapping {
    pub canonical_key: String,
    pub metric_key: String,
    pub metric_label: String,
    pub statement_type: String,
    pub taxonomy: String,
    pub concept_name: String,
    pub unit: String,
    pub confidence: String,
    pub rationale: String,
    pub selected_by: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptCatalogEntry {
    pub taxonomy: String,
    pub concept_name: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub unit: String,
    pub fact_count: i64,
    pub earliest_period_end: Option<String>,
    pub latest_period_end: Option<String>,
    pub latest_filed_at: Option<String>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub period_shape_counts: BTreeMap<String, i64>,
    pub dominant_period_shape: String,
    pub series_usability: String,
    pub plot_readiness: String,
    pub narrative_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePaths {
    pub run_slug: String,
    pub workspace_dir: PathBuf,
    pub sqlite_path: PathBuf,
    pub generated_dir: PathBuf,
}
