use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    blackboard::BlackboardState,
    entry::EntryKind,
    ids::{DocumentId, EntryId, SignalId},
    signal::Priority,
};

#[async_trait]
pub trait Orchestrator: Send + Sync {
    async fn plan_next(&self, board: &BlackboardState) -> anyhow::Result<OrchestratorDecision>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OrchestratorDecision {
    DispatchWorkers(Vec<WorkerTask>),
    Converge {
        reasoning: String,
        remaining_gaps: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerTask {
    pub id: String,
    pub description: String,
    pub reads_from_entries: Vec<EntryId>,
    pub reads_from_documents: Vec<DocumentReadRequest>,
    pub expected_output: EntryKind,
    pub priority: Priority,
    pub addresses_signals: Vec<SignalId>,
    pub domain: serde_json::Value,
}

impl WorkerTask {
    pub fn new(description: impl Into<String>, expected_output: EntryKind) -> Self {
        Self {
            id: crate::id_gen::new_id("worker-task"),
            description: description.into(),
            reads_from_entries: Vec::new(),
            reads_from_documents: Vec::new(),
            expected_output,
            priority: Priority::Medium,
            addresses_signals: Vec::new(),
            domain: serde_json::Value::Object(Default::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentReadRequest {
    pub document_id: DocumentId,
    pub sections: Vec<String>,
}

/// Converges when no open signals remain above the minimum priority threshold.
pub struct SignalDrainedOrchestrator {
    pub min_priority: Priority,
}

#[async_trait]
impl Orchestrator for SignalDrainedOrchestrator {
    async fn plan_next(&self, board: &BlackboardState) -> anyhow::Result<OrchestratorDecision> {
        let open: Vec<_> = board
            .signals
            .iter()
            .filter(|signal| signal.status == crate::signal::SignalStatus::Open)
            .filter(|signal| signal.priority >= self.min_priority)
            .collect();

        if open.is_empty() {
            return Ok(OrchestratorDecision::Converge {
                reasoning: "no open signals above minimum priority".to_string(),
                remaining_gaps: Vec::new(),
            });
        }

        let tasks = open
            .into_iter()
            .map(|signal| {
                WorkerTask {
                    id: crate::id_gen::new_id("worker-task"),
                    description: signal.content.clone(),
                    reads_from_entries: Vec::new(),
                    reads_from_documents: Vec::new(),
                    expected_output: EntryKind::Analysis,
                    priority: signal.priority.clone(),
                    addresses_signals: vec![signal.id.clone()],
                    domain: signal.domain.clone(),
                }
            })
            .collect();

        Ok(OrchestratorDecision::DispatchWorkers(tasks))
    }
}
