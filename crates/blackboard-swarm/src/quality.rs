use crate::{blackboard::BlackboardState, entry::Entry};

#[derive(Clone, Debug)]
pub enum GateResult {
    Pass,
    Warn(String),
    Reject(String),
    Quarantine(String),
}

impl GateResult {
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::Reject(_) | Self::Quarantine(_))
    }
}

#[derive(Clone, Debug)]
pub struct GateOutcome {
    pub gate_name: String,
    pub result: GateResult,
}

pub trait QualityGate: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, entry: &Entry, board: &BlackboardState) -> GateResult;
}

/// Entry must cite at least one source or evidence ref.
pub struct SourceRequiredGate;

impl QualityGate for SourceRequiredGate {
    fn name(&self) -> &'static str {
        "source_required"
    }

    fn check(&self, entry: &Entry, _board: &BlackboardState) -> GateResult {
        if entry.sources.is_empty() && entry.evidence_refs.is_empty() {
            GateResult::Reject("observation requires a source or evidence ref".to_string())
        } else {
            GateResult::Pass
        }
    }
}

/// Contradiction entries must reference both sides via relations.
pub struct ContradictionReferencesGate;

impl QualityGate for ContradictionReferencesGate {
    fn name(&self) -> &'static str {
        "contradiction_references"
    }

    fn check(&self, entry: &Entry, _board: &BlackboardState) -> GateResult {
        use crate::entry::EntryKind;

        if !matches!(entry.kind, EntryKind::Contradiction) {
            return GateResult::Pass;
        }

        if entry.relations.contradicts.len() < 2 {
            GateResult::Reject(
                "contradiction entry must reference at least two conflicting entries".to_string(),
            )
        } else {
            GateResult::Pass
        }
    }
}

/// Warn when document sources lack section or evidence text.
pub struct DocumentSourceDetailGate;

impl QualityGate for DocumentSourceDetailGate {
    fn name(&self) -> &'static str {
        "document_source_detail"
    }

    fn check(&self, entry: &Entry, _board: &BlackboardState) -> GateResult {
        for source in &entry.sources {
            if source.section.is_none() && source.evidence.is_none() {
                return GateResult::Warn(format!(
                    "source for document {} lacks section or evidence excerpt",
                    source.document_id
                ));
            }
        }
        GateResult::Pass
    }
}

/// Evidence refs should include a query hash or excerpt for auditability.
pub struct EvidenceRefAuditGate;

impl QualityGate for EvidenceRefAuditGate {
    fn name(&self) -> &'static str {
        "evidence_ref_audit"
    }

    fn check(&self, entry: &Entry, _board: &BlackboardState) -> GateResult {
        for evidence in &entry.evidence_refs {
            if evidence.query_hash.is_none() && evidence.excerpt_or_summary.is_none() {
                return GateResult::Warn(format!(
                    "evidence ref {} ({}) lacks query_hash and excerpt",
                    evidence.id.0,
                    evidence.kind.as_str()
                ));
            }
        }
        GateResult::Pass
    }
}

/// Placeholder for domain-specific period/unit validation on calculation entries.
pub struct CalculationMetadataGate {
    pub required_domain_keys: Vec<String>,
}

impl QualityGate for CalculationMetadataGate {
    fn name(&self) -> &'static str {
        "calculation_metadata"
    }

    fn check(&self, entry: &Entry, _board: &BlackboardState) -> GateResult {
        use crate::entry::EntryKind;

        if !matches!(entry.kind, EntryKind::Calculation) {
            return GateResult::Pass;
        }

        for key in &self.required_domain_keys {
            if entry.domain.get(key).is_none() {
                return GateResult::Reject(format!(
                    "calculation entry missing required domain key: {key}"
                ));
            }
        }
        GateResult::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        entry::EntryKind,
        evidence::{EvidenceKind, EvidenceRef},
    };

    #[test]
    fn source_required_rejects_bare_observation() {
        let gate = SourceRequiredGate;
        let entry = crate::entry::Entry::builder(EntryKind::Observation, "no source").build();
        assert!(gate.check(&entry, &BlackboardState::new(crate::ids::RunId::new(), "")).is_blocking());
    }

    #[test]
    fn evidence_ref_satisfies_source_required() {
        let gate = SourceRequiredGate;
        let entry = crate::entry::Entry::builder(EntryKind::Observation, "with evidence")
            .evidence_ref(EvidenceRef::new(EvidenceKind::AlphaVantageMetric, "gross_margin_q1"))
            .build();
        assert!(!gate.check(&entry, &BlackboardState::new(crate::ids::RunId::new(), "")).is_blocking());
    }
}
