use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct NarrativeBoard {
    pub sources: Vec<SourceRow>,
    pub claims: Vec<ClaimRow>,
    pub map: NarrativeMapFields,
    pub agreements: Vec<NarrativeItemRow>,
    pub cruxes: Vec<NarrativeItemRow>,
    pub sections: HashMap<String, SectionRow>,
    pub gaps: Vec<GapRow>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NarrativeMapFields {
    pub dominant: Option<String>,
    pub bull: Option<String>,
    pub bear: Option<String>,
    pub consensus: Option<String>,
    pub counter_narrative: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceRow {
    pub id: i64,
    pub title: Option<String>,
    pub url: Option<String>,
    pub source_type: Option<String>,
    pub published_at: Option<String>,
    pub why_it_matters: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaimRow {
    pub id: i64,
    pub claim: String,
    pub source_id: Option<i64>,
    pub claim_type: Option<String>,
    pub side: Option<String>,
    pub confidence: Option<String>,
    pub metric: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct NarrativeItemRow {
    pub id: i64,
    pub item_order: i64,
    pub body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SectionRow {
    pub status: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GapRow {
    pub gap_key: String,
    pub description: String,
    pub status: Option<String>,
}
