# AGENTS.md

## Cursor Cloud specific instructions

This is a single Rust application ("Analogues", built on the [Loco.rs](https://loco.rs) SaaS
starter). It has two operational modes from one crate: a **stock research pipeline** (CLI
tasks — the primary product) and an **HTTP web/auth server**. Standard commands live in
`README.md` and `commands.md`; this section only captures non-obvious caveats.

### Toolchain (important)

- The base image ships an older Rust (1.83) that **cannot build this repo**: transitive deps
  (e.g. the `arrow` crates) require Rust edition 2024, i.e. Rust ≥ 1.85. The startup update
  script installs and defaults the latest `stable` toolchain (matching CI's `RUST_TOOLCHAIN:
  stable`).
- If a shell shows an old `rustc`/`cargo`, run `rustup default stable`. There is no
  `rust-toolchain.toml`, so the default is whatever the update script set.

### Build / test / run

- Build: `cargo build`. Cargo aliases (`.cargo/config.toml`): `cargo loco` → `cargo run --`,
  `cargo loco-tool`, `cargo playground`.
- Tests: `cargo test --all-features --all`. Tests default to **SQLite** via `config/test.yaml`
  (`analogues_test.sqlite`, recreated on boot), so **Postgres/Redis are not needed locally**
  even though CI starts them via `DATABASE_URL`/`REDIS_URL`. Task tests use
  `fetch_financials:false`, so they run fully offline.
- Lint: `cargo fmt --all -- --check` and the CI clippy command
  (`cargo clippy --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery
  -W rust-2018-idioms`). Both commands run, but on the current latest `stable` toolchain they
  report many version-drift findings (fmt diffs / new pedantic+nursery lints) against the
  committed tree. This is toolchain drift, not broken setup; do not "fix" the tree unless asked.

### Web/auth server

- Run: `cargo loco start` → listens on `http://localhost:5150`. Auth API is under `/api/auth`
  (`register`, `login`, `current`, `magic-link`, ...). SQLite dev DB auto-migrates on startup.
- `register` enqueues a welcome email through the background worker to SMTP `localhost:1025`.
  No SMTP server runs here, so the send fails in the background, but the HTTP request still
  returns `200` and **login works without email verification**. No MailHog is required to test
  the core auth flow.

### Stock research pipeline (primary product)

- Run tasks from repo root: `cargo loco task <TASK> key:value ...` (see `commands.md`). Run
  outputs land in `reports/stock-narrative-research/<TICKER>-<DATE>-<N>/` (gitignored).
- Offline scaffolding works with `initWorkspace ticker:<T> fetch_financials:false` (creates
  `run.sqlite`, schema, sections) — no network or keys needed.
- **Full networked ingest requires `ALPHA_VANTAGE_API_KEY`**: Alpha Vantage fundamentals are
  required (not optional) — without it `initWorkspace` fails the `av_provenance` quality gate
  and aborts. LLM lanes (default narrative-map / financial-analysis, `rigTest`, and
  `mapping_strategy:llm_reviewed`) additionally require `OPENROUTER_API_KEY`. Keys can be set
  as plain env vars (`commands.md` documents a 1Password `op run` wrapper, but that is optional).
- Network notes: `data.sec.gov` is reachable; Yahoo Finance quote endpoints may return HTTP
  429 (rate limited) — transient/external, not a setup problem.
- `generateReport` requires a populated workspace (fundamentals `revenue_ttm` +
  `shares_outstanding`, at least one source, claim, and scenario). On a bare/offline workspace
  it exits with a clear "cannot render yet" validation error listing what is missing.
