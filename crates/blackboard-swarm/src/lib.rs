//! Domain-neutral blackboard runtime for iterative, source-grounded agent swarms.
//!
//! The crate separates **durable blackboard cognition** (entries, signals, obligations)
//! from **domain adapters** that supply prompts, quality gates, and output formats.
//!
//! Financial time series and other canonical data should live in typed stores queried
//! by workers; the blackboard holds observations, questions, contradictions, and
//! references to that data via [`EvidenceRef`].
//!
//! # Layer model
//!
//! ```text
//! Canonical data (external)  ->  queried by workers, referenced via EvidenceRef
//! Blackboard cognition       ->  entries, signals, obligations, worker runs
//! Promoted outputs (domain)  ->  narratives, scenarios, alerts (outside this crate)
//! ```

mod id_gen;
pub mod adapter;
pub mod blackboard;
pub mod document;
pub mod entry;
pub mod error;
pub mod evidence;
pub mod ids;
pub mod model;
pub mod obligation;
pub mod orchestrator;
pub mod persistence;
pub mod quality;
pub mod runtime;
pub mod signal;
pub mod store;
pub mod synthesis;
pub mod task;
pub mod worker;
pub mod worker_run;

pub use adapter::DomainAdapter;
pub use blackboard::{Blackboard, BlackboardIndex, BlackboardState};
pub use document::{Document, DocumentProfile, DocumentStatus, TextSpan};
pub use entry::{
    Entry, EntryKind, EntryRelations, EntryStatus, EpistemicClass, EpistemicStatus,
    SourceCredibility, SourceRef, WorkerRecord,
};
pub use error::{BlackboardError, Result};
pub use evidence::{EvidenceKind, EvidenceRef, EvidenceRefId};
pub use ids::{DocumentId, EntryId, ObligationId, RunId, SignalId};
pub use model::{ModelClient, ModelRequest, ModelResponse, TokenUsage};
pub use obligation::{Obligation, ObligationStatus};
pub use orchestrator::{DocumentReadRequest, Orchestrator, OrchestratorDecision, WorkerTask};
pub use persistence::{
    BlackboardEvent, BlackboardSnapshot, JsonFileStore, MemoryStore, PersistenceStore,
};
pub use quality::{GateOutcome, QualityGate};
pub use runtime::{SwarmConfig, SwarmResult, SwarmRuntime};
pub use signal::{Priority, Signal, SignalKind, SignalStatus};
pub use store::{BoardStore, InMemoryBoardStore};
#[cfg(feature = "sqlite")]
pub use store::SqliteBoardStore;
pub use synthesis::{SynthesisFormat, SynthesisOutput, SynthesisRequest, Synthesizer};
pub use task::{OutputSpec, Task, TaskStateMap, TaskStateRow};
pub use worker::{DocumentSection, Worker, WorkerContext, WorkerOutput};
pub use worker_run::{WorkerRun, WorkerRunStatus};

pub const SCHEMA_VERSION: u32 = 1;
