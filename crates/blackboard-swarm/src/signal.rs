use crate::ids::{EntryId, SignalId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signal {
    pub id: SignalId,
    pub kind: SignalKind,
    pub content: String,
    pub priority: Priority,
    pub status: SignalStatus,
    pub origin_entry: Option<EntryId>,
    pub addressed_by: Option<EntryId>,
    pub iteration_created: u32,
    pub domain: serde_json::Value,
}

impl Signal {
    pub fn builder(kind: SignalKind, content: impl Into<String>) -> SignalBuilder {
        SignalBuilder::new(kind, content)
    }
}

pub struct SignalBuilder {
    signal: Signal,
}

impl SignalBuilder {
    pub fn new(kind: SignalKind, content: impl Into<String>) -> Self {
        Self {
            signal: Signal {
                id: SignalId::new(),
                kind,
                content: content.into(),
                priority: Priority::Medium,
                status: SignalStatus::Open,
                origin_entry: None,
                addressed_by: None,
                iteration_created: 0,
                domain: serde_json::Value::Object(Default::default()),
            },
        }
    }

    pub fn priority(mut self, priority: Priority) -> Self {
        self.signal.priority = priority;
        self
    }

    pub fn origin_entry(mut self, entry_id: EntryId) -> Self {
        self.signal.origin_entry = Some(entry_id);
        self
    }

    pub fn iteration_created(mut self, iteration: u32) -> Self {
        self.signal.iteration_created = iteration;
        self
    }

    pub fn build(self) -> Signal {
        self.signal
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value")]
pub enum SignalKind {
    Question,
    ReadRequest,
    ConvergenceGap,
    ContradictionResolution,
    CoverageGap,
    Investigation,
    Domain(String),
}

impl SignalKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Question => "question",
            Self::ReadRequest => "read_request",
            Self::ConvergenceGap => "convergence_gap",
            Self::ContradictionResolution => "contradiction_resolution",
            Self::CoverageGap => "coverage_gap",
            Self::Investigation => "investigation",
            Self::Domain(name) => name,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalStatus {
    Open,
    Addressed,
    Expired,
    Cancelled,
}
