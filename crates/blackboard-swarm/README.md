# blackboard-swarm

Domain-neutral blackboard runtime for iterative, source-grounded agent swarms.

This crate implements the **reasoning membrane** between canonical typed data stores and messy narrative evidence. Financial time series, SEC facts, and price data should live in external queryable stores; the blackboard holds observations, signals, obligations, and references to that data via `EvidenceRef`.

## Layer model

```text
Canonical data (external)  ->  queried by workers, referenced via EvidenceRef
Blackboard cognition       ->  entries, signals, obligations, worker runs
Promoted outputs (domain)  ->  narratives, scenarios, alerts (outside this crate)
```

## Core concepts

| Concept | Role |
| --- | --- |
| **Entry** | Atomic unit of knowledge (observation, analysis, contradiction, …) |
| **Signal** | Work queue item — questions, investigations, coverage gaps |
| **Obligation** | Final-output commitment the synthesizer must satisfy |
| **EvidenceRef** | Pointer to canonical data (SEC fact, AV metric, price window, calculation) |
| **Blackboard** | In-memory state + query indexes |
| **BoardStore** | Incremental CRUD for entries/signals/obligations |
| **PersistenceStore** | Snapshots and event log |

## Quick start

```rust
use blackboard_swarm::{
    Blackboard, Entry, EntryKind, EvidenceKind, EvidenceRef, RunId, Signal, SignalKind,
};

let mut board = Blackboard::new(RunId::new(), "Analyze NVDA inventory trends");

board.add_signal(
    Signal::builder(
        SignalKind::Investigation,
        "Check whether inventory grew faster than revenue over 6 quarters",
    )
    .build(),
)?;

let entry = Entry::builder(
    EntryKind::Observation,
    "Inventory-related XBRL series shows a large change relative to revenue growth.",
)
.evidence_ref(EvidenceRef::new(
    EvidenceKind::SecFactObservation,
    "sec:NVDA:InventoryNet:FY2024",
))
.tag("inventory")
.build();

board.add_entry(entry)?;
```

## Runtime loop

The `SwarmRuntime` wires together:

- `DomainAdapter` — prompts, quality gates, output format
- `Orchestrator` — decides whether to dispatch workers or converge
- `Worker` — returns structured entries (not final prose)
- `PersistenceStore` — snapshots and events
- `Synthesizer` — produces final output from reviewed board state

Seed an existing board state for experimentation:

```rust
runtime.run_with_seed(task, Some(seed_state)).await?;
```

## Storage backends

- **In-memory**: `InMemoryBoardStore`, `MemoryStore` (always available)
- **JSON files**: `JsonFileStore` for snapshot/event persistence
- **SQLite** (default feature): `SqliteBoardStore` with `bb_*` tables

Disable SQLite with `default-features = false` when you only need in-memory types.

## Design reference

Based on `docs/references/blackboard-concept-design.md` in the Analogues repo, extended with `EvidenceRef` for hybrid canonical-data + blackboard-cognition architecture.

This crate is intentionally **not wired into Analogues tasks yet** — it exists for interface iteration and experimentation.
