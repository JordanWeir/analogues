# Build Initial Agent Swarm

**Date:** 2026-06-08  
**Status:** In progress (steps 1–6 complete)  
**Scope:** Agent runtime foundation, shared tool loop, and initial worker lanes behind a linear orchestrator (v0).

## Summary

We're looking to do complex financial analysis of a stock, and ultimately produce a report with informed scenarios, narratives, and metrics.

We need to get in place a worker system capable of actually executing our various tasks.

These should share a common core, while still allowing specialization for specific tasks.

Many tasks will benefit greatly from detailed prompts that lay out extremely specific "golden paths" to follow, and many will have various custom tools that should be shared.  The existing skills implementations relied on fairly powerful frontier models, while many of these may be running the equivalent of "mini" models.  

In practice, this means baking in stronger prompt guidance and "In Context Learning" style examples will be an important part of keeping them effective, and we cannot just copy-paste over prompts from the other system.

## Proposed Folder Structure

/src/agents
-- tool_loop_agent.rs
-- narrative_researcher/
-- fundamental_catalog_manager/
-- financial_model_explorer/
-- scenario_builder/
-- content_manager/
/src/agents/tools
-- sql_query.rs
-- web_search.rs
-- fundamentals_lookup.rs
/src/lanes
-- init_workspace
-- build_catalog
-- build_narrative_map
-- identify_crux_candidates
-- financial_mechanics_experiments
-- scenario_generation
-- scenario_artifacts
-- report_synthesis

## Responsibilities

### Tool Loop Agent

- Owns traits / structs / shared impl for generic "Run in a loop" logic.
- Needs to capture things like usages and costs automatically
- Should assign a name / task per instance
- Store data about usage in the DB after each invocation
- Allow adding both server side tools and regular tools
- Can be adapted from the existing POC code
- We should also test out openrouter-rs to see if it's a better fit then RIG, since we're likely to do everything through openrouter anyways

### Narrative Researcher

- Is in charge of using web search to see what the popular narratives around the stock are
- Looks for impactful news, etc
- Builds out the Source Pack and Narrative Map

### Fundamental Catalog Manager

- A possible alternative for our Heuristics based identification of core fundamental columns
- Both searches for fundamentals online, and reads the SEC facts data
- Identifies which time series should be promoted to fundamental
- NOTE: Current implementation runs web search; maybe instead it could just use a freely available API that has that fundamental data?  Big token savings + speed improvements + reliability improvements
- NOTE: Current implementation may not be consistently checking the values either; verify

### Financial Model Explorer

- Receives Narratives and comes up with interesting bridges
- Runs Historical modelling on the Facts data to find relationships that may not be obvious from just the core fundamentals
- Runs Forward-Looking models on the facts data to see what key sensitivities or cruxes might exist for narratives and how they may play out in practice

### Scenario Builder

- Is in charge of building out the final scenario asset
- Can read and borrow ideas from the runs executed by the Financial Model Explorer
- Should ultimately produce walk forward models using the same SQL tools, and attach them to the scenario alongside prose lists of sensitivities, what the scenario represents, etc.

### Content Manager

- Input: The final scenarios, narratives, financial mechanics, interesting insights generated, and business model description
- Output: A complete report with sections filled out in a coherent manner

## Strategy

I want to see all these individual parts working *essentially correctly* before we push for a full blackboard implementation.

The blackboard concept treats it more as a distributed control system, and in practice a lot of this work can be done linearly.

Many aspects of the distributed control system would improve reliability and may lead to a higher success rate overall, alongside better ability to recover from failures.

I think we just need to actually witness the chain of agents working first though, and get a better sense of where they are stronger or weaker then the Skill-based approach on real tasks.

We have too much theory about why these decomposed agents should be better, but now we really need to actually prove they can execute competently.

## Linear Path

The Linear Path maps closely to [01-pipeline-plan.md](../01-pipeline-plan.md).

1. Initialize Workspace And Ingest Facts: Deterministic
2. Build Canonical And Exploratory Fact Catalogs: Fundamental Catalog Manager OR Deterministic
3. Build Source Pack And Narrative Map: Narrative Researcher
4. Triage Concepts Into Crux Candidates: Financial Model Explorer
5. Run Financial Mechanics Experiments: Financial Model Explorer
6. Construct Scenarios And Projection Inputs: Scenario Builder
7. Calculate Distribution And Render Artifacts: Deterministic
8. Synthesize Report And Refresh Hooks: Content Manager

## Lane Abstraction (v0)

Keep the lane contract small. Lanes orchestrate; `src/services/` owns deterministic domain logic; agents own model loops.

- `LaneContext` — workspace handle, run id, config
- `LaneResult` — status, writes summary, gate results
- `trait Lane` — `async fn run(&self, ctx) -> Result<LaneResult>`
- `trait Gate` — `fn check(&self, ctx) -> GateResult`
- `LinearRunner` — runs `Vec<Box<dyn Lane>>` in order; stops on gate failure

Each lane module should own its contract, not reimplement domain logic:

- `mod.rs` — lane runner
- `inputs.rs` / `writes.rs` — which tables are read/written
- `gate.rs` — minimal quality check before downstream lanes trust output
- `strategy.rs` — `Deterministic | Agent(...)` where applicable

Introduce shared gate infrastructure once (step 2 or 4). Add per-lane gate definitions after each lane lands.

### Lane scope split (ingest vs catalog)

Aligns with [06-07-decompose-init-entanglement.md](./06-07-decompose-init-entanglement.md):

| Lane | Owns |
|------|------|
| `init_workspace` | Phase 1: fetch + persist raw facts, stock info, gaps |
| `build_catalog` | Phases 2–4: catalog materialize, canonical resolve (heuristic or agent), derive starter fundamentals |

## Build Discipline

- Each step should leave existing Loco tasks working.
- `initWorkspace` can call lane modules internally before separate task commands are exposed.
- Ship ingest-only lane before peeling catalog off; avoid a big-bang cutover.
- OpenRouter transport uses `openrouter-rs` for completions; server tools still need a thin custom serde shim.

## Plan

Setup a simple linear track, see where agents break down, iterate.

1. **Tool loop + worker_runs + shared tools** ✅
   - `ToolLoopAgent` in `src/agents/tool_loop_agent.rs`; OpenRouter transport stays in `src/services/openrouter_chat.rs`
   - `worker_runs` table + `WorkerRunStore` persist name, model, rounds, tokens, cost, status per invocation
   - Shared tool registry in `src/agents/tools/`: `sql_query` (`workspace_sql`), `web_search`, `fundamentals_lookup` (stub)
   - `ModelClient` delegates to `ToolLoopAgent`; existing concept review path unchanged

2. **Lane + Gate traits + LinearRunner skeleton** ✅
   - `src/lanes/`: `LaneContext`, `LaneResult`, `Lane` + `Gate` traits, `LinearRunner`
   - Gate statuses: pass / warn / reject / quarantine; runner stops on reject/quarantine or failed lane
   - `quality_gate_results` table + `QualityGateStore::persist_batch` after each lane

3. **`init_workspace` lane (ingest only)** ✅
   - `InitWorkspaceLane` in `src/lanes/init_workspace/`; phase 1 fetch + persist via `workspace_ingest` and `RawIngestPersist`
   - Phase 1 only: fetch + persist raw facts, stock info, gaps (`sec_raw_facts`, `stock_info`, `run_metadata`, `data_gaps`)
   - `initWorkspace` delegates ingest through `LinearRunner`

4. **`init_workspace` gates** ✅
   - `workspace_exists`, `sec_provenance`, `fetch_failures_recorded` in `src/lanes/init_workspace/gate.rs`
   - Workspace readable with matching ticker; SEC facts carry required provenance fields; skipped/failed fetches recorded as gaps

5. **`build_catalog` lane (deterministic path first)** ✅
   - `BuildCatalogLane` in `src/lanes/build_catalog/`; phases 2–4 via `workspace_phases`
   - Catalog materialize, heuristic canonical resolve (`CandidateScoring` default), derive starter fundamentals
   - `initWorkspace` chains `init_workspace` + `build_catalog` when `mapping_strategy` is set; ingest-only path (`mapping_strategy:none`) still defers catalog

6. **`build_catalog` gates** ✅
   - `catalog_materialized`, `core_fundamentals_traceable`, `flow_metrics_period_labeled` in `src/lanes/build_catalog/gate.rs`
   - Catalog non-empty; revenue mapping traceable to `sec_raw_facts`; flow observations do not mix quarter/ytd/annual period types without normalization

7. **`fundamental_catalog_manager` agent** wired into `build_catalog` as optional strategy
   - Thin move of `concept_review` into `src/agents/fundamental_catalog_manager/`
   - Workspace-native review (no throwaway DB); depends on steps 5–6

8. **`narrative_researcher` agent**

9. **`build_narrative_map` lane + gates**

10. **`financial_model_explorer` agent** ✅
    - `src/agents/financial_model_explorer/` — crux-triage and mechanics-experiment modes
    - Tools: `workspace_sql`, `submit_crux_triage`, `run_analysis_draft`, `finalize_analysis`, `submit_mechanics_experiments`
    - Persistence via `financial_analysis_store` → `crux_candidates`, `analysis_runs`, `analysis_experiments`

11. **`identify_crux_candidates` lane + gates** ✅
    - `src/lanes/identify_crux_candidates/`; gates: narrative context, falsifiable cruxes, metric rationale, period-shape warn

12. **`financial_mechanics_experiments` lane + gates** ✅
    - Same agent family as step 10; different golden path, reads/writes, and gates
    - More SQL/calculation-heavy than crux triage
    - Draft → finalize tool flow; gates on promoted experiment structure

13. **`scenario_builder` agent**

14. **`scenario_generation` lane + gates**

15. **`scenario_artifacts` lane (deterministic) + gates**
    - Refactor `generate_report` math/render (Monte Carlo, valuation bands, HTML)

16. **`content_manager` agent**

17. **`report_synthesis` lane + gates**

18. **`run_linear_research` end-to-end on a real ticker**
    - Thin task chaining all completed lanes
    - Can be introduced earlier (after step 3 or 6) to test partial chains
