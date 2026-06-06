# Rust Blackboard Crate Interface

**Status:** INTERFACE SKETCH  
**Date:** 2026-06-06  
**Purpose:** Portable Rust library design for using the swarm/blackboard pattern across multiple projects and domains.

---

## 1. Design Goal

The crate should separate the **domain-neutral swarm runtime** from **domain-specific adapters**.

The runtime owns:

- blackboard state
- entry and signal schemas
- orchestration loop
- worker dispatch
- quality gate execution
- persistence
- provenance and source custody
- obligation-bound synthesis

Domain adapters own:

- task-state row definitions
- worker prompt strategy
- domain-specific quality gates
- evidence standards
- final artifact formats
- optional typed metadata

The key architectural choice is to keep the blackboard schema stable and portable, while allowing domains to attach typed or JSON metadata where needed.

---

## 2. Core Identifiers

Use small newtypes for IDs so APIs do not accidentally mix documents, entries, signals, and runs.

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RunId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DocumentId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct EntryId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SignalId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ObligationId(pub String);
```

---

## 3. Task and Document Model

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub instruction: String,
    pub documents: Vec<Document>,
    pub metadata: serde_json::Value,
    pub output: OutputSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputSpec {
    pub output_dir: Option<String>,
    pub deliverables: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub name: String,
    pub text: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentStatus {
    pub id: DocumentId,
    pub name: String,
    pub profile: Option<DocumentProfile>,
    pub sections_read: Vec<String>,
    pub sections_unread: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentProfile {
    pub numbered_items: Option<u32>,
    pub tables: Option<u32>,
    pub sections: Option<u32>,
    pub document_type: String,
    pub key_entities: Vec<String>,
    pub estimated_complexity: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}
```

`Document` may include full text at runtime, while `DocumentStatus` is the durable coverage record saved into blackboard snapshots.

---

## 4. Blackboard State

Persist the durable state separately from runtime indexes.

```rust
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

pub struct Blackboard {
    state: BlackboardState,
    index: BlackboardIndex,
}

#[derive(Default)]
pub struct BlackboardIndex {
    pub entries_by_id: HashMap<EntryId, usize>,
    pub signals_by_id: HashMap<SignalId, usize>,
    pub entries_by_document: HashMap<DocumentId, Vec<EntryId>>,
    pub entries_by_tag: HashMap<String, Vec<EntryId>>,
    pub entries_by_signal: HashMap<SignalId, Vec<EntryId>>,
}
```

The persisted form should remain simple and durable. Query speed should come from indexes rebuilt around the saved state.

---

## 5. Entries

Entries are atomic units of knowledge.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub kind: EntryKind,
    pub content: String,
    pub sources: Vec<SourceRef>,
    pub epistemic: EpistemicStatus,
    pub confidence: f32,
    pub status: EntryStatus,
    pub tags: Vec<String>,
    pub created_by: WorkerRecord,
    pub relations: EntryRelations,
    pub addresses_signals: Vec<SignalId>,
    pub domain: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntryKind {
    Observation,
    Analysis,
    Calculation,
    Strategy,
    Gap,
    Contradiction,
    Domain(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntryStatus {
    Active,
    Disputed,
    Superseded,
    Quarantined,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceRef {
    pub document_id: DocumentId,
    pub document_name: Option<String>,
    pub section: Option<String>,
    pub evidence: Option<String>,
    pub span: Option<TextSpan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpistemicStatus {
    pub classification: EpistemicClass,
    pub source_credibility: SourceCredibility,
    pub motivation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EpistemicClass {
    Fact,
    Inference,
    Calculation,
    ExpertOpinion,
    AdversarialClaim,
    Strategic,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SourceCredibility {
    High,
    Medium,
    Low,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub worker_id: String,
    pub description: String,
    pub iteration: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EntryRelations {
    pub supports: Vec<EntryId>,
    pub contradicts: Vec<EntryId>,
    pub supersedes: Vec<EntryId>,
    pub derived_from: Vec<EntryId>,
}
```

The `domain` field provides an escape hatch for legal, finance, game AI, or market research metadata without forcing one global ontology.

---

## 6. Signals

Signals are the work queue and control plane.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signal {
    pub id: SignalId,
    pub kind: SignalKind,
    pub content: String,
    pub priority: Priority,
    pub status: SignalStatus,
    pub origin_entry: Option<EntryId>,
    pub addressed_by: Option<EntryId>,
    pub iteration_created: u32,
    pub domain: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SignalKind {
    Question,
    ReadRequest,
    ConvergenceGap,
    ContradictionResolution,
    CoverageGap,
    Domain(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SignalStatus {
    Open,
    Addressed,
    Expired,
    Cancelled,
}
```

Signals should be queryable by priority, status, origin, and addressed-by entry.

---

## 7. Task-State Map

The task-state map is the portable coverage ledger.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStateMap {
    pub rows: Vec<TaskStateRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStateRow {
    pub object_type: String,
    pub required_fields: Vec<String>,
    pub relationships: Vec<String>,
    pub closure_checks: Vec<String>,
    pub worker_questions: Vec<String>,
    pub domain: serde_json::Value,
}
```

Example row types:

| Domain | Row types |
| --- | --- |
| Legal | clause, party, obligation, authority, claim |
| Finance | metric, period, assumption, variance, covenant |
| Game AI | player intent, world-state fact, quest objective, NPC constraint |
| Market research | persona, need, objection, segment, competitor claim |

---

## 8. Obligations

Obligations are final-output commitments generated from reviewed blackboard state.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Obligation {
    pub id: ObligationId,
    pub summary: String,
    pub required_entries: Vec<EntryId>,
    pub target_file: Option<String>,
    pub satisfaction_conditions: Vec<String>,
    pub priority: Priority,
    pub status: ObligationStatus,
    pub domain: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObligationStatus {
    Open,
    Satisfied,
    Waived,
}
```

The synthesizer should be constrained by obligations rather than raw context alone.

---

## 9. Model Interface

The runtime should not depend on a specific model provider.

```rust
use futures::future::BoxFuture;

pub trait ModelClient: Send + Sync {
    fn complete<'a>(
        &'a self,
        request: ModelRequest,
    ) -> BoxFuture<'a, anyhow::Result<ModelResponse>>;
}

#[derive(Clone, Debug)]
pub struct ModelRequest {
    pub prompt: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub json_mode: bool,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug)]
pub struct ModelResponse {
    pub text: String,
    pub parsed_json: Option<serde_json::Value>,
    pub usage: TokenUsage,
    pub model: String,
    pub latency_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub total: u64,
}
```

---

## 10. Domain Adapter

Each domain supplies how to plan, extract, review, and synthesize.

```rust
pub trait DomainAdapter: Send + Sync {
    fn name(&self) -> &'static str;

    fn build_task_state_map(&self, task: &Task, board: &BlackboardState) -> TaskStateMap;

    fn seed_plan_prompt(&self, task: &Task, board: &BlackboardState) -> String;

    fn worker_prompt(&self, task: &WorkerTask, ctx: &WorkerContext) -> String;

    fn quality_gates(&self) -> Vec<Box<dyn QualityGate>>;

    fn synthesis_format(&self, task: &Task) -> SynthesisFormat;
}

pub struct LegalAdapter;
pub struct FinanceAdapter;
pub struct GameAiAdapter;
pub struct MarketResearchAdapter;
```

Domain adapters should not own the core loop. They should shape the loop through prompts, quality gates, row types, and output formats.

---

## 11. Orchestration

```rust
pub trait Orchestrator: Send + Sync {
    fn plan_next<'a>(
        &'a self,
        board: &'a BlackboardState,
    ) -> BoxFuture<'a, anyhow::Result<OrchestratorDecision>>;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentReadRequest {
    pub document_id: DocumentId,
    pub sections: Vec<String>,
}
```

The orchestrator can be model-backed, deterministic, or hybrid.

---

## 12. Worker Interface

```rust
pub trait Worker: Send + Sync {
    fn run<'a>(
        &'a self,
        task: WorkerTask,
        ctx: WorkerContext,
    ) -> BoxFuture<'a, anyhow::Result<WorkerOutput>>;
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
```

Workers should return structured entries, not final prose.

---

## 13. Quality Gates

```rust
pub trait QualityGate: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, entry: &Entry, board: &BlackboardState) -> GateResult;
}

#[derive(Clone, Debug)]
pub enum GateResult {
    Pass,
    Warn(String),
    Reject(String),
    Quarantine(String),
}
```

Example gates:

- source must be valid
- evidence required for observations
- calculation entries must include arithmetic
- contradiction entries must reference both sides
- legal claims must cite authority
- financial calculations must identify period and unit
- game AI decisions must satisfy safety constraints
- market research claims must distinguish anecdote from pattern

---

## 14. Persistence Store

The persistence layer should distinguish snapshots from operational indexes.

```rust
pub trait PersistenceStore: Send + Sync {
    fn save_snapshot<'a>(
        &'a self,
        snapshot: BlackboardSnapshot,
    ) -> BoxFuture<'a, anyhow::Result<()>>;

    fn load_snapshot<'a>(
        &'a self,
        run_id: &'a RunId,
        label: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<BlackboardSnapshot>>;

    fn append_event<'a>(
        &'a self,
        event: BlackboardEvent,
    ) -> BoxFuture<'a, anyhow::Result<()>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlackboardSnapshot {
    pub run_id: RunId,
    pub label: String,
    pub state: BlackboardState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub schema_version: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlackboardEvent {
    SnapshotSaved { run_id: RunId, label: String },
    EntryAdded { run_id: RunId, entry_id: EntryId },
    SignalAdded { run_id: RunId, signal_id: SignalId },
    SignalAddressed { run_id: RunId, signal_id: SignalId, entry_id: EntryId },
    ObligationAdded { run_id: RunId, obligation_id: ObligationId },
    WorkerCompleted { run_id: RunId, worker_id: String, usage: TokenUsage },
}
```

A local implementation could write JSON files. A production implementation could use SQLite, Postgres, object storage, search indexes, or event streams.

---

## 15. Synthesis

```rust
pub trait Synthesizer: Send + Sync {
    fn synthesize<'a>(
        &'a self,
        request: SynthesisRequest,
    ) -> BoxFuture<'a, anyhow::Result<SynthesisOutput>>;
}

#[derive(Clone, Debug)]
pub struct SynthesisRequest {
    pub task: Task,
    pub board: BlackboardState,
    pub must_include: Vec<Obligation>,
    pub format: SynthesisFormat,
}

#[derive(Clone, Debug)]
pub enum SynthesisFormat {
    Markdown,
    Json,
    Docx,
    FileScoped(HashMap<String, String>),
    Domain(String),
}

#[derive(Clone, Debug)]
pub enum SynthesisOutput {
    Text(String),
    Json(serde_json::Value),
    Files(HashMap<String, Vec<u8>>),
}
```

---

## 16. Runtime API

The top-level crate API should be small.

```rust
pub struct SwarmRuntime<A, M, P> {
    pub adapter: A,
    pub model: M,
    pub persistence: P,
    pub config: SwarmConfig,
}

#[derive(Clone, Debug)]
pub struct SwarmConfig {
    pub max_iterations: u32,
    pub min_iterations: u32,
    pub max_workers: usize,
    pub token_budget: Option<u64>,
    pub save_snapshots: bool,
}

pub struct SwarmResult {
    pub run_id: RunId,
    pub output: SynthesisOutput,
    pub final_state: BlackboardState,
}

impl<A, M, P> SwarmRuntime<A, M, P>
where
    A: DomainAdapter,
    M: ModelClient,
    P: PersistenceStore,
{
    pub async fn run(&self, task: Task) -> anyhow::Result<SwarmResult> {
        todo!("profile -> seed -> extract -> orchestrate -> review -> synthesize")
    }
}
```

---

## 17. Query API

Querying should be explicit instead of scattered across ad hoc list scans.

```rust
impl Blackboard {
    pub fn state(&self) -> &BlackboardState {
        &self.state
    }

    pub fn entry(&self, id: &EntryId) -> Option<&Entry> {
        self.index
            .entries_by_id
            .get(id)
            .and_then(|idx| self.state.entries.get(*idx))
    }

    pub fn open_signals(&self, min_priority: Priority) -> Vec<&Signal> {
        self.state
            .signals
            .iter()
            .filter(|s| matches!(s.status, SignalStatus::Open))
            .filter(|s| s.priority >= min_priority)
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

    pub fn entries_addressing_signal(&self, signal_id: &SignalId) -> Vec<&Entry> {
        self.index
            .entries_by_signal
            .get(signal_id)
            .into_iter()
            .flatten()
            .filter_map(|id| self.entry(id))
            .collect()
    }
}
```

This query API can later be backed by a database without changing domain code.

---

## 18. Crate Layout

One possible crate/module layout:

```text
blackboard-swarm/
  src/
    lib.rs
    ids.rs
    task.rs
    document.rs
    blackboard.rs
    entry.rs
    signal.rs
    obligation.rs
    model.rs
    adapter.rs
    orchestrator.rs
    worker.rs
    quality.rs
    persistence.rs
    synthesis.rs
    runtime.rs
    query.rs
  crates/
    blackboard-swarm-legal/
    blackboard-swarm-finance/
    blackboard-swarm-game-ai/
    blackboard-swarm-market-research/
```

The domain crates should depend on the core runtime, not the other way around.

---

## 19. Strategic Interface Principle

The durable contract should be:

```text
Stable core:
  Task, DocumentStatus, BlackboardState, Entry, Signal, Obligation, SourceRef

Replaceable engines:
  ModelClient, Orchestrator, Worker, Reviewer, Synthesizer

Domain-specific extensions:
  DomainAdapter, quality gates, task-state rows, artifact formats

Persistence boundary:
  snapshots, events, operational indexes, evidence store
```

That separation allows the same crate to power legal review, financial analysis, game AI reasoning, market research synthesis, compliance workflows, technical diligence, or any other domain that needs source-grounded, iterative, auditable analysis.
