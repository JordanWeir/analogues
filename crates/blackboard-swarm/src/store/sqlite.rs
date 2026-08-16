use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use rusqlite::{params, Connection};
use serde_json;

use crate::{
    blackboard::BlackboardState,
    document::DocumentStatus,
    entry::Entry,
    error::{BlackboardError, Result},
    ids::{EntryId, ObligationId, RunId, SignalId},
    obligation::Obligation,
    signal::Signal,
    store::BoardStore,
    worker_run::WorkerRun,
    SCHEMA_VERSION,
};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS bb_runs (
    run_id TEXT PRIMARY KEY,
    task_instruction TEXT NOT NULL,
    documents_json TEXT NOT NULL DEFAULT '[]',
    iteration INTEGER NOT NULL DEFAULT 0,
    token_budget INTEGER,
    tokens_used INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    schema_version INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bb_entries (
    run_id TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (run_id, entry_id)
);

CREATE TABLE IF NOT EXISTS bb_signals (
    run_id TEXT NOT NULL,
    signal_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (run_id, signal_id)
);

CREATE TABLE IF NOT EXISTS bb_obligations (
    run_id TEXT NOT NULL,
    obligation_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    status TEXT NOT NULL,
    priority TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (run_id, obligation_id)
);

CREATE TABLE IF NOT EXISTS bb_worker_runs (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_bb_entries_run ON bb_entries(run_id);
CREATE INDEX IF NOT EXISTS idx_bb_signals_run_status ON bb_signals(run_id, status);
CREATE INDEX IF NOT EXISTS idx_bb_obligations_run_status ON bb_obligations(run_id, status);
"#;

pub struct SqliteBoardStore {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl SqliteBoardStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        }

        let conn = Connection::open(&path)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        conn.execute_batch(SCHEMA)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        Ok(Self {
            path,
            conn: Mutex::new(conn),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }
}

#[async_trait]
impl BoardStore for SqliteBoardStore {
    async fn init_run(&self, state: &BlackboardState) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let now = Self::now();
        let documents_json = serde_json::to_string(&state.documents)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        conn.execute(
            "INSERT INTO bb_runs (
                run_id, task_instruction, documents_json, iteration,
                token_budget, tokens_used, metadata_json, schema_version,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                state.run_id.as_str(),
                state.task_instruction,
                documents_json,
                state.iteration,
                state.token_budget.map(|v| v as i64),
                state.tokens_used as i64,
                state.metadata.to_string(),
                SCHEMA_VERSION as i64,
                now,
                now,
            ],
        )
        .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        Ok(())
    }

    async fn load_run(&self, run_id: &RunId) -> Result<BlackboardState> {
        let (task_instruction, documents, iteration, token_budget, tokens_used, metadata) = {
            let conn = self.conn.lock().map_err(|_| {
                BlackboardError::persistence("sqlite connection lock poisoned".to_string())
            })?;

            conn.query_row(
                "SELECT task_instruction, documents_json, iteration, token_budget, tokens_used, metadata_json
                 FROM bb_runs WHERE run_id = ?1",
                params![run_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .map_err(|err| BlackboardError::persistence(err.to_string()))?
        };

        let documents: Vec<DocumentStatus> = serde_json::from_str(&documents)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        let metadata: serde_json::Value = serde_json::from_str(&metadata)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        Ok(BlackboardState {
            run_id: run_id.clone(),
            task_instruction,
            documents,
            entries: self.list_entries(run_id).await?,
            signals: self.list_signals(run_id).await?,
            obligations: self.list_obligations(run_id).await?,
            iteration: iteration as u32,
            token_budget: token_budget.map(|v| v as u64),
            tokens_used: tokens_used as u64,
            metadata,
        })
    }

    async fn upsert_entry(&self, run_id: &RunId, entry: &Entry) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let now = Self::now();
        let payload = serde_json::to_string(entry)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        conn.execute(
            "INSERT INTO bb_entries (run_id, entry_id, payload_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(run_id, entry_id) DO UPDATE SET
                payload_json = excluded.payload_json,
                updated_at = excluded.updated_at",
            params![run_id.as_str(), entry.id.as_str(), payload, now, now],
        )
        .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        Ok(())
    }

    async fn upsert_signal(&self, run_id: &RunId, signal: &Signal) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let now = Self::now();
        let payload = serde_json::to_string(signal)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        conn.execute(
            "INSERT INTO bb_signals (
                run_id, signal_id, payload_json, status, priority, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(run_id, signal_id) DO UPDATE SET
                payload_json = excluded.payload_json,
                status = excluded.status,
                priority = excluded.priority,
                updated_at = excluded.updated_at",
            params![
                run_id.as_str(),
                signal.id.as_str(),
                payload,
                format!("{:?}", signal.status),
                format!("{:?}", signal.priority),
                now,
                now,
            ],
        )
        .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        Ok(())
    }

    async fn upsert_obligation(&self, run_id: &RunId, obligation: &Obligation) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let now = Self::now();
        let payload = serde_json::to_string(obligation)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        conn.execute(
            "INSERT INTO bb_obligations (
                run_id, obligation_id, payload_json, status, priority, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(run_id, obligation_id) DO UPDATE SET
                payload_json = excluded.payload_json,
                status = excluded.status,
                priority = excluded.priority,
                updated_at = excluded.updated_at",
            params![
                run_id.as_str(),
                obligation.id.as_str(),
                payload,
                format!("{:?}", obligation.status),
                format!("{:?}", obligation.priority),
                now,
                now,
            ],
        )
        .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        Ok(())
    }

    async fn record_worker_run(&self, worker_run: &WorkerRun) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let payload = serde_json::to_string(worker_run)
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        conn.execute(
            "INSERT INTO bb_worker_runs (id, run_id, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                worker_run.id,
                worker_run.run_id.as_str(),
                payload,
                worker_run.started_at.to_rfc3339(),
            ],
        )
        .map_err(|err| BlackboardError::persistence(err.to_string()))?;
        Ok(())
    }

    async fn list_entries(&self, run_id: &RunId) -> Result<Vec<Entry>> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let mut stmt = conn
            .prepare("SELECT payload_json FROM bb_entries WHERE run_id = ?1 ORDER BY created_at")
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        let rows = stmt
            .query_map(params![run_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        let mut entries = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| BlackboardError::persistence(err.to_string()))?;
            entries.push(
                serde_json::from_str(&payload)
                    .map_err(|err| BlackboardError::persistence(err.to_string()))?,
            );
        }
        Ok(entries)
    }

    async fn list_signals(&self, run_id: &RunId) -> Result<Vec<Signal>> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let mut stmt = conn
            .prepare("SELECT payload_json FROM bb_signals WHERE run_id = ?1 ORDER BY created_at")
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        let rows = stmt
            .query_map(params![run_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        let mut signals = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| BlackboardError::persistence(err.to_string()))?;
            signals.push(
                serde_json::from_str(&payload)
                    .map_err(|err| BlackboardError::persistence(err.to_string()))?,
            );
        }
        Ok(signals)
    }

    async fn list_obligations(&self, run_id: &RunId) -> Result<Vec<Obligation>> {
        let conn = self.conn.lock().map_err(|_| {
            BlackboardError::persistence("sqlite connection lock poisoned".to_string())
        })?;
        let mut stmt = conn
            .prepare(
                "SELECT payload_json FROM bb_obligations WHERE run_id = ?1 ORDER BY created_at",
            )
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        let rows = stmt
            .query_map(params![run_id.as_str()], |row| row.get::<_, String>(0))
            .map_err(|err| BlackboardError::persistence(err.to_string()))?;

        let mut obligations = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| BlackboardError::persistence(err.to_string()))?;
            obligations.push(
                serde_json::from_str(&payload)
                    .map_err(|err| BlackboardError::persistence(err.to_string()))?,
            );
        }
        Ok(obligations)
    }

    async fn get_entry(&self, run_id: &RunId, entry_id: &EntryId) -> Result<Option<Entry>> {
        Ok(self
            .list_entries(run_id)
            .await?
            .into_iter()
            .find(|entry| &entry.id == entry_id))
    }

    async fn get_signal(&self, run_id: &RunId, signal_id: &SignalId) -> Result<Option<Signal>> {
        Ok(self
            .list_signals(run_id)
            .await?
            .into_iter()
            .find(|signal| &signal.id == signal_id))
    }

    async fn get_obligation(
        &self,
        run_id: &RunId,
        obligation_id: &ObligationId,
    ) -> Result<Option<Obligation>> {
        Ok(self
            .list_obligations(run_id)
            .await?
            .into_iter()
            .find(|obligation| &obligation.id == obligation_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{blackboard::Blackboard, entry::EntryKind};

    #[tokio::test]
    async fn sqlite_store_persists_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.sqlite");
        let store = SqliteBoardStore::open(&path).unwrap();
        let board = Blackboard::new(RunId::new(), "sqlite test");
        store.init_run(board.state()).await.unwrap();

        let entry = crate::entry::Entry::builder(EntryKind::Analysis, "persisted").build();
        store
            .upsert_entry(board.run_id(), &entry)
            .await
            .unwrap();

        let loaded = store.load_run(board.run_id()).await.unwrap();
        assert_eq!(loaded.entries.len(), 1);
    }
}
