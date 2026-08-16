# CLI commands w/ Examples

Run all tasks from the workspace root:

```sh
cargo loco task <TASK_NAME> <key>:<value> ...
```

Parameters use `key:value` pairs (no `=`). Aliases are noted where supported.

Default workspace root: `reports/stock-narrative-research/`  
Run directories follow `{TICKER}-{YYYY-MM-DD}-{INDEX}/` (e.g. `ORCL-2026-06-12-1/`).

## Environment Variables are needed; use onepass

eval $(op signin)

Preface
op run --env-file .env -- [command]

---

## initWorkspace

Initialize a stock research workspace, create `run.sqlite`, and run the init pipeline (ingest, and optionally catalog, narrative map, and financial analysis).

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `ticker` | **yes** | — | Stock symbol. Alias: `symbol`. |
| `date` | no | today (`YYYY-MM-DD`) | Run date used in the workspace slug. |
| `base_dir` | no | `reports/stock-narrative-research` | Parent directory for run folders. |
| `fetch_financials` | no | `true` | Set to `false`, `0`, `no`, or `skip` to skip SEC/quote fetch. |
| `mapping_strategy` | no | `candidate_scoring` | Canonical mapping strategy. Alias: `concept_mapping_strategy`. |
| `build_narrative_map` | no | `true` | Set to `false`, `0`, `no`, or `skip` to skip the narrative-map lane. Requires `fetch_financials` and a non-`none` mapping strategy. |
| `build_financial_analysis` | no | `true` | Set to `false`, `0`, `no`, or `skip` to skip crux identification and mechanics experiments. Implies `build_narrative_map` when enabled. Requires `fetch_financials` and a non-`none` mapping strategy. |
| `checkpoints` | no | `false` | Set to `true`, `1`, or `yes` to save a SQLite snapshot in `checkpoints/` after each lane completes. |

**`mapping_strategy` values**

| Value | Effect |
|-------|--------|
| `candidate_scoring`, `candidate`, `heuristic` | Deterministic candidate-scoring resolver (default). |
| `llm_reviewed`, `llm`, `model` | LLM-reviewed resolver (requires API keys). |
| `none`, `skip`, `skip_mapping` | Ingest + concept catalog only; skip mapping and fundamentals. |

**Examples (ORCL)**

```sh
# Full init: ingest, catalog, narrative map, crux triage, and mechanics (defaults)
cargo loco task initWorkspace ticker:ORCL

# Init for a specific date
cargo loco task initWorkspace ticker:ORCL date:2026-06-12

# Ingest only — no mapping or fundamentals
cargo loco task initWorkspace ticker:ORCL mapping_strategy:none

# LLM-reviewed mapping with full default pipeline
cargo loco task initWorkspace ticker:ORCL mapping_strategy:llm_reviewed

# Catalog + ingest only (skip agent lanes)
cargo loco task initWorkspace ticker:ORCL build_narrative_map:false build_financial_analysis:false

# Narrative map only (skip financial model explorer)
cargo loco task initWorkspace ticker:ORCL build_financial_analysis:false

# Scaffold workspace without network fetch (useful for tests)
cargo loco task initWorkspace ticker:ORCL fetch_financials:false

# Save a SQLite checkpoint after each lane completes
cargo loco task initWorkspace ticker:ORCL checkpoints:true
```

---

## resolveCanonicalMappings

Re-run phase 3 canonical metric mapping against an existing workspace.

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `workspace` | **yes** | — | Path to `run.sqlite`. |
| `mapping_strategy` | **yes** | — | `candidate_scoring` (or `candidate`, `heuristic`) or `llm_reviewed` (or `llm`, `model`). Alias: `concept_mapping_strategy`. |

**Examples (ORCL)**

```sh
# Heuristic / candidate-scoring re-run
cargo loco task resolveCanonicalMappings \
  workspace:reports/stock-narrative-research/ORCL-2026-06-12-1/run.sqlite \
  mapping_strategy:candidate_scoring

# LLM-reviewed re-run
cargo loco task resolveCanonicalMappings \
  workspace:reports/stock-narrative-research/ORCL-2026-06-12-1/run.sqlite \
  mapping_strategy:llm_reviewed
```

---

## deriveStarterFundamentals

Derive starter fundamentals from active canonical mappings (phase 4).

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `workspace` | **yes** | — | Path to `run.sqlite`. |

**Example (ORCL)**

```sh
cargo loco task deriveStarterFundamentals \
  workspace:reports/stock-narrative-research/ORCL-2026-06-12-1/run.sqlite
```

---

## generateReport

Calculate scenario outputs, run Monte Carlo simulation, and render `generated/report.html`.

| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `ticker` | **yes** | — | Stock symbol. Alias: `symbol`. |
| `date` | no | today (`YYYY-MM-DD`) | Run date used to locate the workspace. |
| `index` | no | latest run for ticker+date | Positive integer matching the run suffix (e.g. `1` → `ORCL-2026-06-12-1`). |
| `base_dir` | no | `reports/stock-narrative-research` | Parent directory for run folders. |

**Examples (ORCL)**

```sh
# Use the latest ORCL run for today's date
cargo loco task generateReport ticker:ORCL

# Target a specific run
cargo loco task generateReport ticker:ORCL date:2026-06-12 index:1

# Custom base directory
cargo loco task generateReport ticker:ORCL date:2026-06-12 index:1 base_dir:reports/stock-narrative-research
```

Output: `reports/stock-narrative-research/ORCL-2026-06-12-1/generated/report.html`

---

## rigTest

Smoke-test LLM provider connectivity (OpenRouter comedian prompt). No CLI parameters.

Requires `OPENROUTER_API_KEY` in the environment.

**Example**

```sh
cargo loco task rigTest
```

---

## Typical ORCL pipeline

```sh
# 1. Initialize workspace (needs network for SEC/quote fetch)
cargo loco task initWorkspace ticker:ORCL

# 2. (Optional) Re-run mapping or fundamentals on an existing DB
cargo loco task resolveCanonicalMappings \
  workspace:reports/stock-narrative-research/ORCL-2026-06-12-1/run.sqlite \
  mapping_strategy:candidate_scoring

cargo loco task deriveStarterFundamentals \
  workspace:reports/stock-narrative-research/ORCL-2026-06-12-1/run.sqlite

# 3. After populating scenarios and research data in SQLite, render the report
cargo loco task generateReport ticker:ORCL date:2026-06-12 index:1
```
