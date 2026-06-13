use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    document::DocumentStatus,
    entry::Entry,
    error::{BlackboardError, Result},
    ids::{DocumentId, EntryId, ObligationId, RunId, SignalId},
    obligation::Obligation,
    signal::{Priority, Signal, SignalStatus},
};

/// Durable blackboard state persisted across iterations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlackboardState {
    pub run_id: RunId,
    pub task_instruction: String,
    pub documents: Vec<DocumentStatus>,
    pub entries: Vec<Entry>,
    pub signals: Vec<Signal>,
    pub obligations: Vec<Obligation>,
    pub iteration: u32,
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub metadata: serde_json::Value,
}

impl BlackboardState {
    pub fn new(run_id: RunId, task_instruction: impl Into<String>) -> Self {
        Self {
            run_id,
            task_instruction: task_instruction.into(),
            documents: Vec::new(),
            entries: Vec::new(),
            signals: Vec::new(),
            obligations: Vec::new(),
            iteration: 0,
            token_budget: None,
            tokens_used: 0,
            metadata: serde_json::Value::Object(Default::default()),
        }
    }
}

/// Runtime indexes rebuilt from durable state for fast queries.
#[derive(Default, Debug)]
pub struct BlackboardIndex {
    pub entries_by_id: HashMap<EntryId, usize>,
    pub signals_by_id: HashMap<SignalId, usize>,
    pub obligations_by_id: HashMap<ObligationId, usize>,
    pub entries_by_document: HashMap<DocumentId, Vec<EntryId>>,
    pub entries_by_tag: HashMap<String, Vec<EntryId>>,
    pub entries_by_signal: HashMap<SignalId, Vec<EntryId>>,
}

impl BlackboardIndex {
    pub fn rebuild(state: &BlackboardState) -> Self {
        let mut index = Self::default();

        for (idx, entry) in state.entries.iter().enumerate() {
            index.entries_by_id.insert(entry.id.clone(), idx);

            for tag in &entry.tags {
                index
                    .entries_by_tag
                    .entry(tag.clone())
                    .or_default()
                    .push(entry.id.clone());
            }

            for source in &entry.sources {
                index
                    .entries_by_document
                    .entry(source.document_id.clone())
                    .or_default()
                    .push(entry.id.clone());
            }

            for signal_id in &entry.addresses_signals {
                index
                    .entries_by_signal
                    .entry(signal_id.clone())
                    .or_default()
                    .push(entry.id.clone());
            }
        }

        for (idx, signal) in state.signals.iter().enumerate() {
            index.signals_by_id.insert(signal.id.clone(), idx);
        }

        for (idx, obligation) in state.obligations.iter().enumerate() {
            index.obligations_by_id.insert(obligation.id.clone(), idx);
        }

        index
    }
}

/// In-memory blackboard with durable state and rebuilt query indexes.
pub struct Blackboard {
    state: BlackboardState,
    index: BlackboardIndex,
}

impl Blackboard {
    pub fn new(run_id: RunId, task_instruction: impl Into<String>) -> Self {
        let state = BlackboardState::new(run_id, task_instruction);
        let index = BlackboardIndex::rebuild(&state);
        Self { state, index }
    }

    pub fn from_state(state: BlackboardState) -> Self {
        let index = BlackboardIndex::rebuild(&state);
        Self { state, index }
    }

    pub fn state(&self) -> &BlackboardState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut BlackboardState {
        &mut self.state
    }

    pub fn into_state(self) -> BlackboardState {
        self.state
    }

    pub fn run_id(&self) -> &RunId {
        &self.state.run_id
    }

    pub fn iteration(&self) -> u32 {
        self.state.iteration
    }

    pub fn increment_iteration(&mut self) {
        self.state.iteration += 1;
    }

    pub fn add_tokens_used(&mut self, tokens: u64) {
        self.state.tokens_used += tokens;
    }

    pub fn entry(&self, id: &EntryId) -> Option<&Entry> {
        self.index
            .entries_by_id
            .get(id)
            .and_then(|idx| self.state.entries.get(*idx))
    }

    pub fn signal(&self, id: &SignalId) -> Option<&Signal> {
        self.index
            .signals_by_id
            .get(id)
            .and_then(|idx| self.state.signals.get(*idx))
    }

    pub fn obligation(&self, id: &ObligationId) -> Option<&Obligation> {
        self.index
            .obligations_by_id
            .get(id)
            .and_then(|idx| self.state.obligations.get(*idx))
    }

    pub fn add_entry(&mut self, entry: Entry) -> Result<&Entry> {
        if self.index.entries_by_id.contains_key(&entry.id) {
            return Err(BlackboardError::DuplicateId(entry.id.to_string()));
        }

        let id = entry.id.clone();
        self.state.entries.push(entry);
        self.rebuild_index();
        Ok(self.entry(&id).expect("entry just inserted"))
    }

    pub fn add_signal(&mut self, signal: Signal) -> Result<&Signal> {
        if self.index.signals_by_id.contains_key(&signal.id) {
            return Err(BlackboardError::DuplicateId(signal.id.to_string()));
        }

        let id = signal.id.clone();
        self.state.signals.push(signal);
        self.rebuild_index();
        Ok(self.signal(&id).expect("signal just inserted"))
    }

    pub fn add_obligation(&mut self, obligation: Obligation) -> Result<&Obligation> {
        if self
            .index
            .obligations_by_id
            .contains_key(&obligation.id)
        {
            return Err(BlackboardError::DuplicateId(obligation.id.to_string()));
        }

        let id = obligation.id.clone();
        self.state.obligations.push(obligation);
        self.rebuild_index();
        Ok(self.obligation(&id).expect("obligation just inserted"))
    }

    pub fn address_signal(&mut self, signal_id: &SignalId, entry_id: &EntryId) -> Result<()> {
        let signal_idx = self
            .index
            .signals_by_id
            .get(signal_id)
            .copied()
            .ok_or_else(|| BlackboardError::SignalNotFound(signal_id.to_string()))?;

        if !self.index.entries_by_id.contains_key(entry_id) {
            return Err(BlackboardError::EntryNotFound(entry_id.to_string()));
        }

        let signal = &mut self.state.signals[signal_idx];
        signal.status = SignalStatus::Addressed;
        signal.addressed_by = Some(entry_id.clone());
        Ok(())
    }

    pub fn open_signals(&self, min_priority: Priority) -> Vec<&Signal> {
        self.state
            .signals
            .iter()
            .filter(|signal| signal.status == SignalStatus::Open)
            .filter(|signal| signal.priority >= min_priority)
            .collect()
    }

    pub fn open_obligations(&self) -> Vec<&Obligation> {
        self.state
            .obligations
            .iter()
            .filter(|obligation| obligation.status == crate::obligation::ObligationStatus::Open)
            .collect()
    }

    pub fn entries_for_document(&self, document_id: &DocumentId) -> Vec<&Entry> {
        self.index
            .entries_by_document
            .get(document_id)
            .into_iter()
            .flatten()
            .filter_map(|id| self.entry(id))
            .collect()
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&Entry> {
        self.index
            .entries_by_tag
            .get(tag)
            .into_iter()
            .flatten()
            .filter_map(|id| self.entry(id))
            .collect()
    }

    pub fn entries_addressing_signal(&self, signal_id: &SignalId) -> Vec<&Entry> {
        self.index
            .entries_by_signal
            .get(signal_id)
            .into_iter()
            .flatten()
            .filter_map(|id| self.entry(id))
            .collect()
    }

    pub fn entries_by_kind(&self, kind: &crate::entry::EntryKind) -> Vec<&Entry> {
        self.state
            .entries
            .iter()
            .filter(|entry| &entry.kind == kind)
            .collect()
    }

    pub fn active_entries(&self) -> Vec<&Entry> {
        self.state
            .entries
            .iter()
            .filter(|entry| entry.status == crate::entry::EntryStatus::Active)
            .collect()
    }

    fn rebuild_index(&mut self) {
        self.index = BlackboardIndex::rebuild(&self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        entry::EntryKind,
        signal::{SignalKind, SignalStatus},
    };

    #[test]
    fn add_entry_and_query_by_tag() {
        let mut board = Blackboard::new(RunId::new(), "analyze NVDA");
        let entry = crate::entry::Entry::builder(EntryKind::Observation, "inventory spike")
            .tag("inventory")
            .build();
        board.add_entry(entry).unwrap();

        assert_eq!(board.entries_with_tag("inventory").len(), 1);
        assert_eq!(board.active_entries().len(), 1);
    }

    #[test]
    fn address_signal_links_entry() {
        let mut board = Blackboard::new(RunId::new(), "test");
        let signal = crate::signal::Signal::builder(
            SignalKind::Investigation,
            "check inventory vs revenue",
        )
        .build();
        let signal_id = signal.id.clone();
        board.add_signal(signal).unwrap();

        let entry = crate::entry::Entry::builder(
            EntryKind::Analysis,
            "inventory grew faster than revenue",
        )
        .addresses_signal(signal_id.clone())
        .build();
        let entry_id = entry.id.clone();
        board.add_entry(entry).unwrap();
        board.address_signal(&signal_id, &entry_id).unwrap();

        let signal = board.signal(&signal_id).unwrap();
        assert_eq!(signal.status, SignalStatus::Addressed);
        assert_eq!(signal.addressed_by.as_ref(), Some(&entry_id));
        assert_eq!(board.entries_addressing_signal(&signal_id).len(), 1);
    }

    #[test]
    fn rejects_duplicate_entry_id() {
        let mut board = Blackboard::new(RunId::new(), "test");
        let id = EntryId::new();
        let entry_a = crate::entry::Entry::builder(EntryKind::Gap, "a")
            .id(id.clone())
            .build();
        let entry_b = crate::entry::Entry::builder(EntryKind::Gap, "b")
            .id(id)
            .build();
        board.add_entry(entry_a).unwrap();
        assert!(board.add_entry(entry_b).is_err());
    }
}
