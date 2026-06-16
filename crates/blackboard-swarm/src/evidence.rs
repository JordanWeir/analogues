use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::DocumentId;

/// Reference to evidence in a canonical data store or calculation artifact.
///
/// Entries should reference query results rather than duplicating time series data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EvidenceRef {
    pub id: EvidenceRefId,
    pub kind: EvidenceKind,
    pub ref_id: String,
    pub query_hash: Option<String>,
    pub excerpt_or_summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl EvidenceRef {
    pub fn new(kind: EvidenceKind, ref_id: impl Into<String>) -> Self {
        Self {
            id: EvidenceRefId::new(),
            kind,
            ref_id: ref_id.into(),
            query_hash: None,
            excerpt_or_summary: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_query_hash(mut self, hash: impl Into<String>) -> Self {
        self.query_hash = Some(hash.into());
        self
    }

    pub fn with_excerpt(mut self, excerpt: impl Into<String>) -> Self {
        self.excerpt_or_summary = Some(excerpt.into());
        self
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EvidenceRefId(pub String);

impl EvidenceRefId {
    pub fn new() -> Self {
        Self(crate::id_gen::new_id("evidence"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value")]
pub enum EvidenceKind {
    SecFactObservation,
    AlphaVantageMetric,
    PriceSeriesWindow,
    ValuationQueryResult,
    WebDocumentSpan { document_id: DocumentId },
    CalculationResult,
    Custom(String),
}

impl EvidenceKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::SecFactObservation => "sec_fact_observation",
            Self::AlphaVantageMetric => "alpha_vantage_metric",
            Self::PriceSeriesWindow => "price_series_window",
            Self::ValuationQueryResult => "valuation_query_result",
            Self::WebDocumentSpan { .. } => "web_document_span",
            Self::CalculationResult => "calculation_result",
            Self::Custom(name) => name,
        }
    }
}
