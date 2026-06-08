# Decompose init_workspace Entanglement

**Date:** 2026-06-07 (updated 2026-06-08)  
**Status:** Steps 1–6 complete; Steps 7–11 in progress
**Scope:** Separate raw SEC/market ingestion from canonical concept processing; make heuristic vs LLM mapping swappable without re-fetching data.

## Problem Statement

`init_workspace` currently runs ingestion, concept catalog materialization, canonical mapping resolution, and starter fundamental derivation as one in-memory pipeline before writing anything to SQLite. Core domain types (`SecRawFact`, `CanonicalMapping`, `ConceptCatalogEntry`, `FinancialSnapshot`) live in the task module but are imported by multiple services — inverted layering.

We want:

1. A **hard separation** between capturing raw data into the SQL file and the first processing pass that defines canonical tables.
2. **Swappable canonical mapping strategies** (heuristic candidate scoring vs LLM review) without entangling ingest code.
3. The ability to **re-run mapping or derivation** against an existing workspace without re-fetching SEC Company Facts.

## Current State

### Pipeline today

`fetch_sec_companyfacts_snapshot` in `src/tasks/init_workspace.rs` runs synchronously:

1. Fetch + extract raw facts (`SecFactsProvider`)
2. `ConceptCatalog::materialize_catalog_entries`
3. `canonical_mappings_for_strategy` (heuristic OR LLM, with heuristic fallback)
4. `ConceptCatalog::build_observations`, `select_latest_baseline_bundle`, `latest_value_fact`, etc.
5. Pack everything into `FinancialSnapshot`

`persist_financial_snapshot` then writes all layers in one transaction: `sec_raw_facts`, `concept_catalog_entries`, `concept_review_decisions`, `canonical_metric_mappings`, `fundamental_observations`, `fundamentals`, `data_quality_flags`.

### Structural issues

| Issue | Where | Impact |
|-------|-------|--------|
| Types in task module | `init_workspace.rs` | Services import from tasks (`concept_catalog`, `concept_review`, `sec_facts_provider`, `review_workspace`, `market_quote_provider`) |
| God object | `FinancialSnapshot` | Cannot ingest without also deciding mappings and deriving TTM metrics |
| Mixed responsibilities | `ConceptCatalog` | Seeds DB defs, builds catalog inventory, scores heuristic mappings, derives starter fundamentals |
| Duplicated persistence | `init_workspace.rs`, `review_workspace.rs` | Same `insert_raw_sec_facts` / `insert_concept_catalog_entries` logic in two places |
| Throwaway review DB | `review_workspace.rs` | LLM path materializes temp SQLite, queries it, deletes it — even though real workspace could hold raw layer first |
| Strategy orchestration buried | `canonical_mappings_for_strategy` in `init_workspace.rs` | Hard to add strategies or re-run mapping independently |

### Key files

- `src/tasks/init_workspace.rs` — task, types, schema, persistence, orchestration
- `src/services/concept_catalog.rs` — catalog build, heuristic mapping, fundamental derivation
- `src/services/concept_review.rs` — LLM review, promotion, decision records
- `src/services/sec_facts_provider.rs` — SEC HTTP + JSON → `SecRawFact`
- `src/services/review_workspace.rs` — temp DB for LLM agent
- `src/services/workspace_store.rs` — workspace creation + schema seed

---

## Target Layer Model

Four explicit phases with artifacts between them:

```
┌─────────────────┐     ┌──────────────────────┐     ┌─────────────────────────┐     ┌────────────────────┐
│ 1. INGEST       │ --> │ 2. CATALOG BUILD     │ --> │ 3. CANONICAL RESOLVE    │ --> │ 4. DERIVE METRICS  │
│ (external APIs) │     │ (deterministic stats)│     │ (pluggable strategy)    │     │ (observations/TTM) │
└─────────────────┘     └──────────────────────┘     └─────────────────────────┘     └────────────────────┘
     sec_raw_facts          concept_catalog_entries      canonical_metric_mappings      fundamental_observations
     (+ payload JSON)       (+ raw_fact_metric_catalog)  concept_review_decisions       fundamentals (rollup)
```

**Phase 1** and **phase 2** do not depend on which canonical mapping strategy will run.  
**Phase 3** is where heuristic vs LLM swaps.  
**Phase 4** only needs active mappings + raw facts (from DB or memory).

---

## Domain Types to Extract

Move from `init_workspace.rs` into `src/workspace/` (or `src/models/workspace/`):

| Struct | Layer | Notes |
|--------|-------|-------|
| `SecRawFact` | Ingest | Already well-shaped |
| `SecCompanyIdentity`, `SecCompanyFactsPayload` | Ingest | Currently in `sec_facts_provider`; co-locate or re-export |
| `ConceptCatalogEntry` | Catalog | Pure aggregation over raw facts |
| `CanonicalMetricDefinition` | Registry | Promote private `CanonicalMetricSpec` |
| `CanonicalMapping` | Resolve | Output of any mapping strategy |
| `CanonicalMappingCandidate` | Resolve | Heuristic intermediate only |
| `ConceptReviewDecisionRecord` | Resolve | Audit trail |
| `FundamentalObservation` | Derive | Per-fact observations through mappings |
| `StarterFundamentals` | Derive | Split from `FinancialSnapshot`: revenue_ttm, margins, cash, debt, eps, gaps |
| `MarketQuoteSnapshot` | Ingest (parallel) | Yahoo data; separate from SEC |

### Replace `FinancialSnapshot` with phase-specific results

```rust
pub struct SecIngestionResult {
    pub identity: SecCompanyIdentity,
    pub raw_facts: Vec<SecRawFact>,
    pub fetched_at: String,
    pub source_url: String,
    // optionally: raw_json blob for re-parsing
}

pub struct ConceptCatalogSnapshot {
    pub entries: Vec<ConceptCatalogEntry>,
    pub built_at: String,
}

pub struct CanonicalResolutionResult {
    pub mappings: Vec<CanonicalMapping>,
    pub review_decisions: Vec<ConceptReviewDecisionRecord>,
    pub quality_flags: Vec<String>,
    pub strategy_id: String,  // e.g. "candidate_scoring" | "llm_agent_review:model"
}

pub struct DerivedFundamentals {
    pub observations: Vec<FundamentalObservation>,
    pub starter: StarterFundamentals,
    pub quality_flags: Vec<String>,
}
```

Keep a thin `FinancialSnapshot` temporarily for backward compat if needed, but new code should use phase structs.

---

## Traits and Implementations

### 1. Ingestion (external → in-memory)

Formalize what `SecFactsProvider` already does:

```rust
#[async_trait]
pub trait SecFactsSource: Send + Sync {
    fn provider_name(&self) -> &'static str;
    async fn lookup_company(&self, ticker: &str) -> Result<SecCompanyIdentity>;
    async fn fetch_company_facts(&self, identity: &SecCompanyIdentity) -> Result<SecCompanyFactsPayload>;
}

pub struct SecFactsIngestor<S: SecFactsSource> { source: S }
// lookup + fetch + extract_raw_facts → SecIngestionResult
```

Market data stays separate:

```rust
#[async_trait]
pub trait MarketQuoteSource {
    async fn fetch_quotes(&self, ticker: &str) -> Result<MarketQuoteSnapshot>;
}
```

### 2. Persistence (in-memory ↔ SQLite)

Centralize all `insert_*` / `load_*` currently in `init_workspace.rs` and duplicated in `review_workspace.rs`:

```rust
#[async_trait]
pub trait WorkspaceFinancialStore: Send + Sync {
    // Phase 1
    async fn persist_sec_ingestion(&self, result: &SecIngestionResult) -> Result<()>;
    async fn load_sec_raw_facts(&self) -> Result<Vec<SecRawFact>>;

    // Phase 2
    async fn persist_concept_catalog(&self, snapshot: &ConceptCatalogSnapshot) -> Result<()>;
    async fn load_concept_catalog_entries(&self) -> Result<Vec<ConceptCatalogEntry>>;

    // Phase 3
    async fn persist_canonical_resolution(&self, result: &CanonicalResolutionResult, at: &str) -> Result<()>;
    async fn load_active_canonical_mappings(&self) -> Result<Vec<CanonicalMapping>>;

    // Phase 4
    async fn persist_derived_fundamentals(&self, derived: &DerivedFundamentals, at: &str) -> Result<()>;
}
```

Concrete impl: `SqliteWorkspaceFinancialStore { db: DatabaseConnection }`.

**Key win:** LLM agent queries the **real** workspace DB after phases 1–2 are persisted. Delete or shrink `review_workspace.rs` temp DB materialization.

### 3. Catalog build (deterministic, strategy-agnostic)

```rust
pub trait ConceptCatalogBuilder: Send + Sync {
    fn build(&self, raw_facts: &[SecRawFact]) -> ConceptCatalogSnapshot;
}

pub struct DefaultConceptCatalogBuilder;  // current materialize_catalog_entries logic
```

A plain struct is sufficient unless we expect alternate catalog semantics.

### 4. Canonical resolution (the swap point)

```rust
pub struct CanonicalResolutionContext<'a> {
    pub ticker: &'a str,
    pub raw_facts: &'a [SecRawFact],
    pub catalog_entries: &'a [ConceptCatalogEntry],
    pub fetched_at: &'a str,
    pub workspace_db: Option<&'a DatabaseConnection>,  // for LLM workspace_sql tool
}

#[async_trait]
pub trait CanonicalMappingResolver: Send + Sync {
    fn strategy_id(&self) -> &'static str;
    async fn resolve(&self, ctx: CanonicalResolutionContext<'_>) -> Result<CanonicalResolutionResult>;
}
```

Implementations:

| Impl | Source of logic today |
|------|----------------------|
| `CandidateScoringResolver` | `ConceptCatalog::seed_canonical_mappings` |
| `LlmReviewedResolver` | `ConceptReviewService` + fallback to `CandidateScoringResolver` |

Move `canonical_mappings_for_strategy` from `init_workspace.rs` into these impls. Existing fallback-to-heuristic behavior becomes explicit in `LlmReviewedResolver`.

Shared promotion/validation (split across `promote_reviewed_mappings` and `mapping_from_review_decision`):

```rust
pub struct CanonicalMappingPromoter {
    registry: &'static CanonicalMetricRegistry,
}
```

### 5. Canonical metric registry (static definitions)

```rust
pub struct CanonicalMetricRegistry;

impl CanonicalMetricRegistry {
    pub fn definitions() -> &'static [CanonicalMetricDefinition];
    pub fn lookup(canonical_key: &str) -> Option<&'static CanonicalMetricDefinition>;
    pub async fn seed_definitions(db: &DatabaseConnection, at: &str) -> Result<()>;
}
```

Extract `CANONICAL_METRIC_SPECS` and `ConceptCatalog::seed_canonical_definitions`. Both heuristic and LLM paths depend on this; neither should own it.

### 6. Fundamental derivation (post-mapping)

```rust
pub struct FundamentalDeriver;

impl FundamentalDeriver {
    pub fn build_observations(raw_facts: &[SecRawFact], mappings: &[CanonicalMapping])
        -> Vec<FundamentalObservation>;

    pub fn derive_starter_fundamentals(
        raw_facts: &[SecRawFact],
        mappings: &[CanonicalMapping],
    ) -> DerivedFundamentals;
}
```

Move `IncomeBundle`, `TtmMetric`, `SecFact`, `select_latest_income_bundle`, `apply_income_bundle` here. Remove `ConceptCatalog::apply_income_bundle(&mut FinancialSnapshot, ...)`.

---

## Module Layout (target)

| Module | Responsibility |
|--------|----------------|
| `src/workspace/types.rs` | Domain structs (or split by layer) |
| `src/services/sec_facts_provider.rs` | HTTP + JSON parsing → `SecIngestionResult` |
| `src/services/sec_facts_ingestor.rs` (new) | Orchestrates `SecFactsSource` |
| `src/services/concept_catalog_builder.rs` (new) | `materialize_catalog_entries`, narrative tags, period shapes |
| `src/services/canonical_metric_registry.rs` (new) | Static metric defs + DB seed |
| `src/services/canonical_mapping/` (new) | Resolvers, promoter, strategy registry |
| `src/services/concept_review.rs` | LLM prompt, parse, `ConceptReviewOutput` only |
| `src/services/fundamental_deriver.rs` (new) | Observations, TTM bundles, starter metrics |
| `src/services/workspace_financial_store.rs` (new) | All SQL insert/load for financial layers |
| `src/tasks/init_workspace.rs` | Thin task orchestration |
| `src/services/review_workspace.rs` | Delete or reduce to `workspace_schema_hint()` only |

### Dependency direction

```
tasks/init_workspace
  → workspace_financial_store
  → sec_facts_ingestor
  → canonical_mapping (resolvers)
  → fundamental_deriver

services/*  →  workspace/types   (never tasks::init_workspace)
```

---

## Task / CLI Shape

Composable operations (new tasks or flags on existing task):

| Command | Phases | Writes |
|---------|--------|--------|
| `initWorkspace --ticker X --date Y` | schema seed only | `run_metadata`, `canonical_metric_definitions`, `sections` |
| `ingestSecFacts --workspace <path>` | 1 | `sec_raw_facts`, `stock_info` company name |
| `buildConceptCatalog --workspace <path>` | 2 | `concept_catalog_entries` |
| `resolveCanonicalMappings --workspace <path> --strategy candidate_scoring\|llm_reviewed` | 3 | `canonical_metric_mappings`, `concept_review_decisions` |
| `deriveStarterFundamentals --workspace <path>` | 4 | `fundamental_observations`, `fundamentals`, `data_quality_flags` |

Or keep one `initWorkspace` with staged flags:

- `fetch_financials=true, mapping_strategy=none` → ingest only (phases 1–2)
- `mapping_strategy=candidate_scoring` → phases 1–4
- Re-run `resolveCanonicalMappings` on existing DB without SEC re-fetch

Replace `ConceptMappingStrategy` enum on `InitWorkspaceRequest` with resolver registry:

```rust
pub enum CanonicalMappingStrategyKind {
    CandidateScoring,
    LlmReviewed,
}

pub fn build_resolver(
    kind: CanonicalMappingStrategyKind,
    config: ResolverConfig,
) -> Box<dyn CanonicalMappingResolver> { /* ... */ }
```

Use enum + match for two strategies today; introduce `dyn CanonicalMappingResolver` when adding a third or runtime plugins.

---

## Migration Plan (incremental)

### Step 1 — Extract domain types ✅
- Created `src/workspace/types.rs` and `src/workspace/mod.rs`.
- Moved `SecRawFact`, `CanonicalMapping`, `ConceptCatalogEntry`, `FundamentalObservation`, `WorkspacePaths`.
- Re-exported from `init_workspace` for backward compat; services import from `crate::workspace`.
- Deferred: `FinancialSnapshot` (step 6), `ConceptReviewDecisionRecord` (still in `concept_review`).

### Step 2 — Extract `WorkspaceFinancialStore` ✅
- Created `src/services/workspace_financial_store.rs` with all `insert_*` methods.
- Added `load_*` for `sec_raw_facts`, `concept_catalog_entries`, `active_canonical_mappings`, `concept_review_decisions`, `fundamental_observations`.
- `persist_financial_snapshot` delegates to `WorkspaceFinancialStore::persist_snapshot` via `SnapshotPersist` (avoids circular dep on `FinancialSnapshot`).
- `review_workspace.rs` deduped to use shared insert methods.
- Round-trip unit tests for facts and catalog entries.

### Step 3 — Split ingest from mapping ✅
- Extracted `ingest_sec_facts`, `apply_sec_ingest_to_snapshot`, `resolve_sec_canonical_layer`.
- `mapping_strategy: None` via CLI `none` / `skip` / `skip_mapping`; persists phases 1–2 only via `persist_ingestion`.
- `fetch_sec_companyfacts_snapshot` composes ingest + optional resolve; full pipeline unchanged when strategy is `Some`.

### Step 4 — Extract resolvers ✅
- Added `src/services/canonical_mapping/` with `CandidateScoringResolver`, `LlmReviewedResolver`, `CanonicalMappingResolver` trait, and `ConceptMappingStrategy`.
- `LlmReviewedResolver` uses the real workspace `run.sqlite` path; `fetch_and_seed_financials` persists ingest before LLM review.
- Removed temp `materialize_review_workspace`; `review_workspace.rs` retains `workspace_schema_hint()` only. Playground/tests use `materialize_standalone_ingest_workspace`.

### Step 5 — Extract `FundamentalDeriver` ✅
- Added `src/services/fundamental_deriver.rs` with observations, TTM bundles, starter metrics, and `derive_starter_fundamentals`.
- `ConceptCatalog` retains catalog materialization, mapping candidates, and registry seed only; `classify_period` delegates to the deriver.
- `resolve_sec_canonical_layer` calls `FundamentalDeriver::derive_starter_fundamentals` after mapping resolution.

### Step 6 — Split `FinancialSnapshot` ✅
- Added phase structs in `workspace/types.rs`: `SecIngestionResult`, `MarketQuoteSnapshot`, `MarketHeadlines`, `StarterFundamentals`, `DerivedFundamentals`.
- Replaced flat `FinancialSnapshot` with `FinancialRun` (task-layer composer: `ingest`, `market`, `resolution`, `derived`).
- `fundamental_deriver` and `market_quote_provider` no longer import `tasks::init_workspace`.
- Public fetch API renamed to `fetch_financial_run`.

## Migration Plan, Part 2

### Step 7 — Phase re-run on existing workspace
- Open existing run.sqlite, load phases 1–2, run 3–4, persist without SEC re-fetch
- Tasks: resolveCanonicalMappings, deriveStarterFundamentals
- Wire WorkspaceFinancialStore::load_* into production
- Add partial persist methods if needed
- LLM re-run passes sqlite_path like fresh-init path
- Tests: round-trip on mapping_strategy:none fixture

### Step 8 — Break tasks ↔ services dependency cycle
- Move SCHEMA_STATEMENTS, seed_database, InitWorkspaceRequest out of task module
- Remove tasks::init_workspace imports from services

### Step 9 — Slim the task layer
- Move compute_derived_metrics, fundamental_metrics, tests out of init_workspace.rs
- Optional: SecFactsIngestor extract
- Target ~200 lines in task file

### Step 10 — Optional module splits
- canonical_metric_registry.rs, concept_catalog_builder.rs
- Move ConceptReviewDecisionRecord to workspace/types if needed

### Step 11 — Housekeeping
- Doc/QA naming updates, workspace_sql.rs, persist mapping_strategy in run_metadata
- Cross-ref 06-08-fundamental-derivation-policy-repair.md — land Step 7 before policy fixes so re-derive works


---

## Design Decisions

### Should catalog build (phase 2) be separate from canonical resolve (phase 3)?

**Yes.** Catalog entries are deterministic aggregations over `sec_raw_facts`. They do not depend on which revenue concept is chosen. The LLM agent already treats them as investigation input.

### Should phase 4 be separate?

**Yes**, if we want to re-derive fundamentals after changing mappings without re-ingesting SEC data.

### Trait objects vs enums?

Enum + match for two strategies now. Define `CanonicalMappingResolver` trait early for test doubles; use `Box<dyn …>` when a third strategy arrives.

### Keep analysis in Rust?

Yes. Phases 2 and 4 are pure Rust over SQLite or in-memory slices. Only phase 3's LLM variant needs `ModelClient`. The repository trait also allows an external agent to write `canonical_metric_mappings` rows directly if needed.

---

## Success Criteria

- [ ] `initWorkspace` can persist `sec_raw_facts` without running canonical mapping.
- [ ] `resolveCanonicalMappings` can run against an existing workspace SQLite file.
- [ ] Heuristic and LLM strategies are selectable without changing ingest code.
- [ ] No service module imports `tasks::init_workspace`.
- [ ] LLM concept review uses the real workspace DB (no throwaway copy).
- [ ] Existing tests pass after each migration step; behavior unchanged until explicitly changed.

## Highest-Leverage First Step

Extract `WorkspaceFinancialStore` + `SecIngestionResult`, stop running mapping inside `fetch_sec_companyfacts_snapshot`, and point `LlmReviewedResolver` at the real workspace SQLite instead of a throwaway copy.
