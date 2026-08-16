use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::TokenUsage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerRun {
    pub id: String,
    pub run_id: crate::ids::RunId,
    pub worker_id: String,
    pub task_description: String,
    pub status: WorkerRunStatus,
    pub entries_written: u32,
    pub usage: TokenUsage,
    pub model: Option<String>,
    pub latency_ms: u64,
    pub metadata: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl WorkerRun {
    pub fn started(run_id: crate::ids::RunId, worker_id: impl Into<String>, task: impl Into<String>) -> Self {
        Self {
            id: crate::id_gen::new_id("worker-run"),
            run_id,
            worker_id: worker_id.into(),
            task_description: task.into(),
            status: WorkerRunStatus::Running,
            entries_written: 0,
            usage: TokenUsage::default(),
            model: None,
            latency_ms: 0,
            metadata: serde_json::Value::Object(Default::default()),
            started_at: Utc::now(),
            completed_at: None,
        }
    }

    pub fn complete(mut self, entries_written: u32, usage: TokenUsage, model: Option<String>, latency_ms: u64) -> Self {
        self.status = WorkerRunStatus::Completed;
        self.entries_written = entries_written;
        self.usage = usage;
        self.model = model;
        self.latency_ms = latency_ms;
        self.completed_at = Some(Utc::now());
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorkerRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}
