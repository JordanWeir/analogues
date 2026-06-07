# Contract Spec: WorkspaceStore

## Overview

`WorkspaceStore` owns creation, discovery, schema setup, and typed access for a single Analogues research run workspace.

The workspace is the durable center of a run: a filesystem directory, a `run.sqlite` database, and a `generated/` artifact directory. Task wrappers, workers, report compilation, and refresh flows should ask this seam for workspace access rather than rebuilding path conventions or SQLite open behavior themselves.

The first implementation can be a module with functions. It does not need to be a trait until tests or alternate stores make that useful.

## Public Types / Traits

- `WorkspaceStore`: service or trait responsible for create/open/resolve operations.
- `WorkspaceCreateRequest`: ticker, run date, base directory, schema options, and seed options.
- `WorkspaceOpenRequest`: exact workspace path or ticker/date/index lookup.
- `WorkspacePaths`: run slug, workspace directory, SQLite path, generated artifact directory.
- `WorkspaceHandle`: opened SQLite connection plus `WorkspacePaths` and schema metadata.
- `WorkspaceSchemaVersion`: current integer schema version and migration compatibility rules.
- `WorkspaceSeedOptions`: whether to seed required sections, Monte Carlo config, canonical metric definitions, and blackboard bootstrap rows.
- `WorkspaceError`: typed errors for invalid input, allocation failure, schema failure, open failure, and incompatible schema.

## Operations

- `create_workspace(request) -> WorkspacePaths`
- `open_workspace(request) -> WorkspaceHandle`
- `resolve_latest(base_dir, ticker, date) -> WorkspacePaths`
- `apply_schema(connection, target_version)`
- `seed_workspace(connection, request, paths, options)`
- `ensure_generated_dir(paths)`
- `read_run_metadata(connection) -> RunMetadata`
- `close(handle)`

## Behavioral Rules

- Tickers are normalized to uppercase ASCII and may contain only letters, numbers, dots, and hyphens.
- Dates use `YYYY-MM-DD`.
- Run slugs use `{TICKER}-{YYYY-MM-DD}-{index}`.
- Workspace allocation must not overwrite an existing run directory.
- `generated/` exists before any artifact renderer writes files.
- SQLite open mode is explicit:
  - Create flows use read-write-create.
  - Existing report/refresh flows use read-write and fail if the database is missing.
- Schema application is idempotent for the same schema version.
- Seed operations are idempotent where table constraints allow, and deterministic where ordering matters.
- Schema version is persisted in `run_metadata`.
- The store owns schema setup but not provider fetches, fact interpretation, scenario math, or report rendering.
- Task wrappers may parse CLI values, but path allocation and latest-workspace discovery live here.

## Error Semantics

- Invalid ticker or date returns a validation error before filesystem writes.
- Exhausting the run index range returns an allocation error.
- Existing run directories are skipped during allocation, not reused.
- Missing `run.sqlite` during an open flow returns a not-found error.
- Schema migration failure returns the SQL statement context when possible.
- Incompatible schema version returns an explicit compatibility error rather than attempting a best-effort read.
- Filesystem and SQLite errors retain the relevant path in the message.
- The caller owns retry policy; `WorkspaceStore` should not silently retry mutations that may have partially succeeded.

## Valid Examples

- `create_workspace(ticker: "msft", date: "2026-06-07", base_dir: reports/stock-narrative-research)` creates `MSFT-2026-06-07-1/run.sqlite`.
- If `MSFT-2026-06-07-1` exists, the next create call allocates `MSFT-2026-06-07-2`.
- `resolve_latest(base_dir, "MSFT", "2026-06-07")` returns the highest numeric run directory for that ticker/date.
- `open_workspace` for `generateReport` fails if the database is absent instead of creating an empty database.

## Invalid Examples

- A report compiler directly constructing `reports/stock-narrative-research/{ticker}-{date}-1/run.sqlite`.
- A provider adapter creating schema tables as a side effect of fetching data.
- A task wrapper using read-write-create mode during report generation and accidentally creating an empty database.
- A refresh worker writing artifacts before `generated/` has been created.

## Required Tests

- Normalizes lowercase tickers and rejects invalid ticker characters.
- Rejects invalid date formats.
- Allocates the first available run slug without overwriting existing directories.
- Resolves latest workspace by numeric index, not lexicographic order.
- Creates `run.sqlite` and `generated/`.
- Applies schema and writes `run_metadata.schema_version`.
- Opening an existing workspace uses read-write mode and fails when `run.sqlite` is missing.
- Re-running schema setup on an existing current-version workspace is idempotent.
- Seeded required sections and Monte Carlo config are present after create.

