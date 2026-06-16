use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    blackboard::BlackboardState,
    entry::Entry,
    error::{BlackboardError, Result},
    ids::{EntryId, ObligationId, RunId, SignalId},
    obligation::Obligation,
    signal::Signal,
    store::BoardStore,
    worker_run::WorkerRun,
};

#[derive(Default)]
struct RunRecords {
    state: Option<BlackboardState>,
    worker_runs: Vec<WorkerRun>,
}

#[derive(Default)]
pub struct InMemoryBoardStore {
    runs: Arc<RwLock<HashMap<String, RunRecords>>>,
}

impl InMemoryBoardStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl BoardStore for InMemoryBoardStore {
    async fn init_run(&self, state: &BlackboardState) -> Result<()> {
        let mut runs = self.runs.write().await;
        if runs.contains_key(state.run_id.as_str()) {
            return Err(BlackboardError::DuplicateId(state.run_id.to_string()));
        }
        runs.insert(
            state.run_id.to_string(),
            RunRecords {
                state: Some(state.clone()),
                worker_runs: Vec::new(),
            },
        );
        Ok(())
    }

    async fn load_run(&self, run_id: &RunId) -> Result<BlackboardState> {
        let runs = self.runs.read().await;
        runs.get(run_id.as_str())
            .and_then(|records| records.state.clone())
            .ok_or_else(|| BlackboardError::RunNotFound(run_id.to_string()))
    }

    async fn upsert_entry(&self, run_id: &RunId, entry: &Entry) -> Result<()> {
        let mut runs = self.runs.write().await;
        let records = runs
            .get_mut(run_id.as_str())
            .ok_or_else(|| BlackboardError::RunNotFound(run_id.to_string()))?;
        let state = records
            .state
            .as_mut()
            .ok_or_else(|| BlackboardError::RunNotFound(run_id.to_string()))?;

        if let Some(idx) = state
            .entries
            .iter()
            .position(|existing| existing.id == entry.id)
        {
            state.entries[idx] = entry.clone();
        } else {
            state.entries.push(entry.clone());
        }
        Ok(())
    }

    async fn upsert_signal(&self, run_id: &RunId, signal: &Signal) -> Result<()> {
        let mut runs = self.runs.write().await;
        let records = runs
            .get_mut(run_id.as_str())
            .ok_or_else(|| BlackboardError::RunNotFound(run_id.to_string()))?;
        let state = records
            .state
            .as_mut()
            .ok_or_else(|| BlackboardError::RunNotFound(run_id.to_string()))?;

        if let Some(idx) = state
            .signals
            .iter()
            .position(|existing| existing.id == signal.id)
        {
            state.signals[idx] = signal.clone();
        } else {
            state.signals.push(signal.clone());
        }
        Ok(())
    }

    async fn upsert_obligation(&self, run_id: &RunId, obligation: &Obligation) -> Result<()> {
        let mut runs = self.runs.write().await;
        let records = runs
            .get_mut(run_id.as_str())
            .ok_or_else(|| BlackboardError::RunNotFound(run_id.to_string()))?;
        let state = records
            .state
            .as_mut()
            .ok_or_else(|| BlackboardError::RunNotFound(run_id.to_string()))?;

        if let Some(idx) = state
            .obligations
            .iter()
            .position(|existing| existing.id == obligation.id)
        {
            state.obligations[idx] = obligation.clone();
        } else {
            state.obligations.push(obligation.clone());
        }
        Ok(())
    }

    async fn record_worker_run(&self, worker_run: &WorkerRun) -> Result<()> {
        let mut runs = self.runs.write().await;
        let records = runs
            .get_mut(worker_run.run_id.as_str())
            .ok_or_else(|| BlackboardError::RunNotFound(worker_run.run_id.to_string()))?;
        records.worker_runs.push(worker_run.clone());
        Ok(())
    }

    async fn list_entries(&self, run_id: &RunId) -> Result<Vec<Entry>> {
        Ok(self.load_run(run_id).await?.entries)
    }

    async fn list_signals(&self, run_id: &RunId) -> Result<Vec<Signal>> {
        Ok(self.load_run(run_id).await?.signals)
    }

    async fn list_obligations(&self, run_id: &RunId) -> Result<Vec<Obligation>> {
        Ok(self.load_run(run_id).await?.obligations)
    }

    async fn get_entry(&self, run_id: &RunId, entry_id: &EntryId) -> Result<Option<Entry>> {
        Ok(self
            .load_run(run_id)
            .await?
            .entries
            .into_iter()
            .find(|entry| &entry.id == entry_id))
    }

    async fn get_signal(&self, run_id: &RunId, signal_id: &SignalId) -> Result<Option<Signal>> {
        Ok(self
            .load_run(run_id)
            .await?
            .signals
            .into_iter()
            .find(|signal| &signal.id == signal_id))
    }

    async fn get_obligation(
        &self,
        run_id: &RunId,
        obligation_id: &ObligationId,
    ) -> Result<Option<Obligation>> {
        Ok(self
            .load_run(run_id)
            .await?
            .obligations
            .into_iter()
            .find(|obligation| &obligation.id == obligation_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blackboard::Blackboard;
    use crate::entry::EntryKind;

    #[tokio::test]
    async fn board_store_roundtrip() {
        let store = InMemoryBoardStore::new();
        let board = Blackboard::new(RunId::new(), "store test");
        store.init_run(board.state()).await.unwrap();

        let entry = crate::entry::Entry::builder(EntryKind::Observation, "test").build();
        store
            .upsert_entry(board.run_id(), &entry)
            .await
            .unwrap();

        let loaded = store.get_entry(board.run_id(), &entry.id).await.unwrap();
        assert!(loaded.is_some());
    }
}
