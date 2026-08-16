mod memory;
#[cfg(feature = "sqlite")]
mod sqlite;

pub use memory::InMemoryBoardStore;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteBoardStore;

use async_trait::async_trait;

use crate::{
    blackboard::BlackboardState,
    entry::Entry,
    error::Result,
    ids::{EntryId, ObligationId, RunId, SignalId},
    obligation::Obligation,
    signal::Signal,
    worker_run::WorkerRun,
};

/// Operational store for individual blackboard records.
///
/// [`crate::persistence::PersistenceStore`] handles snapshots and events;
/// `BoardStore` supports incremental CRUD during a run.
#[async_trait]
pub trait BoardStore: Send + Sync {
    async fn init_run(&self, state: &BlackboardState) -> Result<()>;
    async fn load_run(&self, run_id: &RunId) -> Result<BlackboardState>;
    async fn upsert_entry(&self, run_id: &RunId, entry: &Entry) -> Result<()>;
    async fn upsert_signal(&self, run_id: &RunId, signal: &Signal) -> Result<()>;
    async fn upsert_obligation(&self, run_id: &RunId, obligation: &Obligation) -> Result<()>;
    async fn record_worker_run(&self, worker_run: &WorkerRun) -> Result<()>;
    async fn list_entries(&self, run_id: &RunId) -> Result<Vec<Entry>>;
    async fn list_signals(&self, run_id: &RunId) -> Result<Vec<Signal>>;
    async fn list_obligations(&self, run_id: &RunId) -> Result<Vec<Obligation>>;
    async fn get_entry(&self, run_id: &RunId, entry_id: &EntryId) -> Result<Option<Entry>>;
    async fn get_signal(&self, run_id: &RunId, signal_id: &SignalId) -> Result<Option<Signal>>;
    async fn get_obligation(
        &self,
        run_id: &RunId,
        obligation_id: &ObligationId,
    ) -> Result<Option<Obligation>>;
}
