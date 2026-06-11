use serde::{Deserialize, Serialize};

pub const MIN_NARRATIVE_BODY_LEN: usize = 80;
pub const MIN_SOURCES: usize = 3;
pub const MIN_CLAIMS: usize = 5;
pub const MIN_CRUXES: usize = 2;

pub const NARRATIVE_SIDES: &[&str] =
    &["dominant", "bull", "bear", "consensus", "counter_narrative"];

pub const NARRATIVE_ITEM_TYPES: &[&str] = &["agreement", "crux"];

pub const SOURCE_TYPES: &[&str] = &[
    "Official company source",
    "Filing",
    "Transcript",
    "Financial news",
    "Market commentary",
    "Investor letter",
    "Short-seller / adversarial",
    "Regulatory / legal",
    "Social / retail narrative",
    "Other",
];

pub const CLAIM_TYPES: &[&str] = &[
    "demand",
    "revenue growth",
    "margin",
    "earnings",
    "valuation",
    "competitive position",
    "product",
    "regulatory",
    "credibility",
    "accounting",
    "customer concentration",
    "supplier dependency",
    "macro/sector",
    "management quality",
    "capital allocation",
];

pub const CLAIM_SIDES: &[&str] = &[
    "bull",
    "bear",
    "neutral",
    "consensus",
    "counter-narrative",
    "adversarial",
];

pub const CLAIM_CONFIDENCES: &[&str] = &["high", "medium", "low", "inference"];

pub const SECTION_KEYS: &[&str] = &["business_model", "why_now"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSourceInput {
    pub title: String,
    pub url: Option<String>,
    pub source_type: String,
    pub published_at: Option<String>,
    pub accessed_at: Option<String>,
    pub why_it_matters: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureClaimInput {
    pub claim: String,
    pub source_id: Option<i64>,
    pub source_title: Option<String>,
    pub claim_type: String,
    pub side: String,
    pub confidence: String,
    pub metric: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureNarrativeSideInput {
    pub side: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureNarrativeItemsInput {
    pub item_type: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureOrientationInput {
    pub dominant_question: String,
    pub current_setup: String,
    pub time_horizon: String,
    pub base_rate_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSectionInput {
    pub section_key: String,
    pub title: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResearchGapInput {
    pub gap_key: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NarrativeWorkspaceSnapshot {
    pub source_count: i64,
    pub claim_count: i64,
    pub narrative_sides_captured: Vec<String>,
    pub agreement_count: i64,
    pub crux_count: i64,
    pub orientation_captured: bool,
    pub sections_captured: Vec<String>,
    pub research_gap_count: i64,
}
