use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;

use crate::{
    blackboard::BlackboardState,
    error::{BlackboardError, Result},
    ids::{EntryId, ObligationId, RunId, SignalId},
    model::TokenUsage,
    SCHEMA_VERSION,
};

#[async_trait]
pub trait PersistenceStore: Send + Sync {
    async fn save_snapshot(&self, snapshot: BlackboardSnapshot) -> Result<()>;
    async fn load_snapshot(&self, run_id: &RunId, label: &str) -> Result<BlackboardSnapshot>;
    async fn append_event(&self, event: BlackboardEvent) -> Result<()>;
    async fn list_snapshots(&self, run_id: &RunId) -> Result<Vec<String>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlackboardSnapshot {
    pub run_id: RunId,
    pub label: String,
    pub state: BlackboardState,
    pub created_at: DateTime<Utc>,
    pub schema_version: u32,
}

impl BlackboardSnapshot {
    pub fn new(run_id: RunId, label: impl Into<String>, state: BlackboardState) -> Self {
        Self {
            run_id,
            label: label.into(),
            state,
            created_at: Utc::now(),
            schema_version: SCHEMA_VERSION,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum BlackboardEvent {
    SnapshotSaved { run_id: RunId, label: String },
    EntryAdded { run_id: RunId, entry_id: EntryId },
    SignalAdded { run_id: RunId, signal_id: SignalId },
    SignalAddressed {
        run_id: RunId,
        signal_id: SignalId,
        entry_id: EntryId,
    },
    ObligationAdded {
        run_id: RunId,
        obligation_id: ObligationId,
    },
    WorkerCompleted {
        run_id: RunId,
        worker_id: String,
        usage: TokenUsage,
    },
}

#[derive(Default)]
pub struct MemoryStore {
    inner: Arc<RwLock<MemoryStoreInner>>,
}

#[derive(Default)]
struct MemoryStoreInner {
    snapshots: HashMap<(String, String), BlackboardSnapshot>,
    events: Vec<BlackboardEvent>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PersistenceStore for MemoryStore {
    async fn save_snapshot(&self, snapshot: BlackboardSnapshot) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner
            .snapshots
            .insert((snapshot.run_id.to_string(), snapshot.label.clone()), snapshot);
        Ok(())
    }

    async fn load_snapshot(&self, run_id: &RunId, label: &str) -> Result<BlackboardSnapshot> {
        let inner = self.inner.read().await;
        inner
            .snapshots
            .get(&(run_id.to_string(), label.to_string()))
            .cloned()
            .ok_or_else(|| BlackboardError::RunNotFound(format!("{run_id}/{label}")))
    }

    async fn append_event(&self, event: BlackboardEvent) -> Result<()> {
        self.inner.write().await.events.push(event);
        Ok(())
    }

    async fn list_snapshots(&self, run_id: &RunId) -> Result<Vec<String>> {
        let inner = self.inner.read().await;
        Ok(inner
            .snapshots
            .keys()
            .filter(|(id, _)| id == run_id.as_str())
            .map(|(_, label)| label.clone())
            .collect())
    }
}

pub struct JsonFileStore {
    root: PathBuf,
}

impl JsonFileStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    fn snapshot_path(&self, run_id: &RunId, label: &str) -> PathBuf {
        self.root
            .join(run_id.as_str())
            .join(format!("{label}.json"))
    }

    fn events_path(&self, run_id: &RunId) -> PathBuf {
        self.root.join(run_id.as_str()).join("events.jsonl")
    }
}

#[async_trait]
impl PersistenceStore for JsonFileStore {
    async fn save_snapshot(&self, snapshot: BlackboardSnapshot) -> Result<()> {
        let path = self.snapshot_path(&snapshot.run_id, &snapshot.label);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        }
        let bytes = serde_json::to_vec_pretty(&snapshot)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        tokio::fs::write(&path, bytes)
            .await
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        Ok(())
    }

    async fn load_snapshot(&self, run_id: &RunId, label: &str) -> Result<BlackboardSnapshot> {
        let path = self.snapshot_path(run_id, label);
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|err| BlackboardError::persistence(err.to_string()))
    }

    async fn append_event(&self, event: BlackboardEvent) -> Result<()> {
        let run_id = match &event {
            BlackboardEvent::SnapshotSaved { run_id, .. }
            | BlackboardEvent::EntryAdded { run_id, .. }
            | BlackboardEvent::SignalAdded { run_id, .. }
            | BlackboardEvent::SignalAddressed { run_id, .. }
            | BlackboardEvent::ObligationAdded { run_id, .. }
            | BlackboardEvent::WorkerCompleted { run_id, .. } => run_id.clone(),
        };

        let path = self.events_path(&run_id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        }

        let line = serde_json::to_string(&event)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        file.write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        Ok(())
    }

    async fn list_snapshots(&self, run_id: &RunId) -> Result<Vec<String>> {
        let dir = self.root.join(run_id.as_str());
        let mut labels = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|err| BlackboardError::persistence(err.to_string()))?
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") && name != "events.jsonl" {
                labels.push(name.trim_end_matches(".json").to_string());
            }
        }
        Ok(labels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blackboard::Blackboard;

    #[tokio::test]
    async fn memory_store_roundtrip() {
        let store = MemoryStore::new();
        let board = Blackboard::new(RunId::new(), "test task");
        let snapshot = BlackboardSnapshot::new(board.run_id().clone(), "iter-0", board.into_state());

        store.save_snapshot(snapshot.clone()).await.unwrap();
        let loaded = store.load_snapshot(&snapshot.run_id, "iter-0").await.unwrap();
        assert_eq!(loaded.state.task_instruction, "test task");
    }

    #[tokio::test]
    async fn json_file_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileStore::new(dir.path());
        let board = Blackboard::new(RunId::new(), "persist me");
        let snapshot = BlackboardSnapshot::new(board.run_id().clone(), "final", board.into_state());

        store.save_snapshot(snapshot.clone()).await.unwrap();
        let loaded = store.load_snapshot(&snapshot.run_id, "final").await.unwrap();
        assert_eq!(loaded.state.task_instruction, "persist me");
    }
}
