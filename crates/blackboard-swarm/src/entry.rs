use crate::{
    document::TextSpan,
    evidence::EvidenceRef,
    ids::{EntryId, SignalId},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub kind: EntryKind,
    pub content: String,
    pub sources: Vec<SourceRef>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub epistemic: EpistemicStatus,
    pub confidence: f32,
    pub status: EntryStatus,
    pub tags: Vec<String>,
    pub created_by: WorkerRecord,
    pub relations: EntryRelations,
    pub addresses_signals: Vec<SignalId>,
    pub domain: serde_json::Value,
}

impl Entry {
    pub fn builder(kind: EntryKind, content: impl Into<String>) -> EntryBuilder {
        EntryBuilder::new(kind, content)
    }
}

pub struct EntryBuilder {
    entry: Entry,
}

impl EntryBuilder {
    pub fn new(kind: EntryKind, content: impl Into<String>) -> Self {
        Self {
            entry: Entry {
                id: EntryId::new(),
                kind,
                content: content.into(),
                sources: Vec::new(),
                evidence_refs: Vec::new(),
                epistemic: EpistemicStatus::default(),
                confidence: 0.5,
                status: EntryStatus::Active,
                tags: Vec::new(),
                created_by: WorkerRecord::system(),
                relations: EntryRelations::default(),
                addresses_signals: Vec::new(),
                domain: serde_json::Value::Object(Default::default()),
            },
        }
    }

    pub fn id(mut self, id: EntryId) -> Self {
        self.entry.id = id;
        self
    }

    pub fn source(mut self, source: SourceRef) -> Self {
        self.entry.sources.push(source);
        self
    }

    pub fn evidence_ref(mut self, evidence_ref: EvidenceRef) -> Self {
        self.entry.evidence_refs.push(evidence_ref);
        self
    }

    pub fn epistemic(mut self, epistemic: EpistemicStatus) -> Self {
        self.entry.epistemic = epistemic;
        self
    }

    pub fn confidence(mut self, confidence: f32) -> Self {
        self.entry.confidence = confidence;
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.entry.tags.push(tag.into());
        self
    }

    pub fn created_by(mut self, worker: WorkerRecord) -> Self {
        self.entry.created_by = worker;
        self
    }

    pub fn addresses_signal(mut self, signal_id: SignalId) -> Self {
        self.entry.addresses_signals.push(signal_id);
        self
    }

    pub fn build(self) -> Entry {
        self.entry
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value")]
pub enum EntryKind {
    Observation,
    Analysis,
    Calculation,
    Strategy,
    Gap,
    Contradiction,
    Question,
    Domain(String),
}

impl EntryKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Observation => "observation",
            Self::Analysis => "analysis",
            Self::Calculation => "calculation",
            Self::Strategy => "strategy",
            Self::Gap => "gap",
            Self::Contradiction => "contradiction",
            Self::Question => "question",
            Self::Domain(name) => name,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntryStatus {
    Active,
    Disputed,
    Superseded,
    Quarantined,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceRef {
    pub document_id: crate::ids::DocumentId,
    pub document_name: Option<String>,
    pub section: Option<String>,
    pub evidence: Option<String>,
    pub span: Option<TextSpan>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EpistemicStatus {
    pub classification: EpistemicClass,
    pub source_credibility: SourceCredibility,
    pub motivation: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum EpistemicClass {
    #[default]
    Unknown,
    Fact,
    Inference,
    Calculation,
    ExpertOpinion,
    AdversarialClaim,
    Strategic,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceCredibility {
    #[default]
    Unknown,
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub worker_id: String,
    pub description: String,
    pub iteration: u32,
}

impl WorkerRecord {
    pub fn system() -> Self {
        Self {
            worker_id: "system".to_string(),
            description: "system bootstrap".to_string(),
            iteration: 0,
        }
    }

    pub fn new(worker_id: impl Into<String>, description: impl Into<String>, iteration: u32) -> Self {
        Self {
            worker_id: worker_id.into(),
            description: description.into(),
            iteration,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EntryRelations {
    pub supports: Vec<EntryId>,
    pub contradicts: Vec<EntryId>,
    pub supersedes: Vec<EntryId>,
    pub derived_from: Vec<EntryId>,
}
