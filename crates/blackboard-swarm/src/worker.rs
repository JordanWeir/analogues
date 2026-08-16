use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    entry::Entry,
    ids::DocumentId,
    model::TokenUsage,
    orchestrator::{DocumentReadRequest, WorkerTask},
    signal::Signal,
};

#[async_trait]
pub trait Worker: Send + Sync {
    async fn run(&self, task: WorkerTask, ctx: WorkerContext) -> anyhow::Result<WorkerOutput>;
}

#[derive(Clone, Debug)]
pub struct WorkerContext {
    pub task_instruction: String,
    pub entries: Vec<Entry>,
    pub signals: Vec<Signal>,
    pub document_sections: Vec<DocumentSection>,
}

#[derive(Clone, Debug)]
pub struct DocumentSection {
    pub document_id: DocumentId,
    pub document_name: String,
    pub section: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerOutput {
    pub worker_id: String,
    pub task: WorkerTask,
    pub entries: Vec<Entry>,
    pub sections_read: Vec<DocumentReadRequest>,
    pub usage: TokenUsage,
    pub model: Option<String>,
}

/// Echo worker for tests: writes a single analysis entry mirroring the task description.
pub struct EchoWorker {
    pub worker_id: String,
}

#[async_trait]
impl Worker for EchoWorker {
    async fn run(&self, task: WorkerTask, _ctx: WorkerContext) -> anyhow::Result<WorkerOutput> {
        use crate::entry::{Entry, EntryKind, WorkerRecord};

        let mut builder = Entry::builder(EntryKind::Analysis, task.description.clone())
            .created_by(WorkerRecord::new(
                self.worker_id.clone(),
                task.description.clone(),
                0,
            ));
        if let Some(signal_id) = task.addresses_signals.first() {
            builder = builder.addresses_signal(signal_id.clone());
        }
        let entry = builder.build();

        Ok(WorkerOutput {
            worker_id: self.worker_id.clone(),
            task,
            entries: vec![entry],
            sections_read: Vec::new(),
            usage: TokenUsage::default(),
            model: None,
        })
    }
}
