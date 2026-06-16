use crate::{ids::EntryId, signal::Priority};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Obligation {
    pub id: crate::ids::ObligationId,
    pub summary: String,
    pub required_entries: Vec<EntryId>,
    pub target_file: Option<String>,
    pub satisfaction_conditions: Vec<String>,
    pub priority: Priority,
    pub status: ObligationStatus,
    pub domain: serde_json::Value,
}

impl Obligation {
    pub fn builder(summary: impl Into<String>) -> ObligationBuilder {
        ObligationBuilder::new(summary)
    }
}

pub struct ObligationBuilder {
    obligation: Obligation,
}

impl ObligationBuilder {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            obligation: Obligation {
                id: crate::ids::ObligationId::new(),
                summary: summary.into(),
                required_entries: Vec::new(),
                target_file: None,
                satisfaction_conditions: Vec::new(),
                priority: Priority::Medium,
                status: ObligationStatus::Open,
                domain: serde_json::Value::Object(Default::default()),
            },
        }
    }

    pub fn required_entry(mut self, entry_id: EntryId) -> Self {
        self.obligation.required_entries.push(entry_id);
        self
    }

    pub fn target_file(mut self, path: impl Into<String>) -> Self {
        self.obligation.target_file = Some(path.into());
        self
    }

    pub fn satisfaction_condition(mut self, condition: impl Into<String>) -> Self {
        self.obligation
            .satisfaction_conditions
            .push(condition.into());
        self
    }

    pub fn build(self) -> Obligation {
        self.obligation
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObligationStatus {
    Open,
    Satisfied,
    Waived,
}
