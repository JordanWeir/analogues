# Pipeline Phase Proposal

## Purpose

This proposal turns the research flow into a durable phase-by-phase contract for the Analogues narrative-scenario product.

The target flow is not just "gather data, ask agents, draft report." The product value comes from converting messy company-specific facts and market narratives into source-backed cruxes, then into scenario-conditioned financial paths and watchable signals.

Each phase should be precise about:

- What it consumes.
- What work it performs.
- What durable tables or artifacts it writes.
- What quality gates must pass before downstream phases trust it.
- How it contributes to the final user value.

This document is intended to stay useful as implementation details evolve. The exact table names may change, but each phase should continue to have clear responsibilities and handoffs.

## Shared Ledgers

The pipeline should keep a few shared ledgers stable across phases.

### Data Ledger

Durable company facts, canonical mappings, raw SEC concepts, observations, data gaps, and quality flags.

Examples:

- `sec_raw_facts`
- `canonical_metric_definitions`
- `canonical_metric_mappings`
- `fundamental_observations`
- `supporting_metric_selections`
- `data_quality_flags`
- `data_gaps`

### Evidence Ledger

Source-backed claims and citations used by narratives, cruxes, scenarios, and report sections.

Examples:

- `sources`
- `claims`
- source references on scenario assumptions, watch items, analogues, and content blocks

### Insight Ledger

Reviewed intermediate conclusions that are stronger than raw facts but not yet final report prose.

This should be implemented as a first-class `insights` table, or as a blackboard-compatible `entries` table if we decide to use that naming. The important contract is that insights must be durable, source-linked where possible, and typed. Final report prose should consume these records rather than becoming the only place where intermediate judgment lives.

Useful insight types:

- `observation`: A raw or lightly interpreted fact.
- `analysis`: A narrative interpretation or business implication.
- `calculation`: A scripted or deterministic financial result.
- `gap`: A missing data point or unresolved question.
- `contradiction`: Evidence that cuts against another claim or narrative.
- `crux`: A falsifiable assumption that materially changes the scenario path.

### Scenario Ledger

The assumptions and calculations that drive scenario-conditioned projections.

Examples:

- `scenario_assumptions`
- `scenario_crux_assumptions`
- `scenario_sensitivities`
- `scenario_signals`
- `scenario_periods`
- `monte_carlo_config`
- `monte_carlo_summary`
- `monte_carlo_histogram_bins`
- `monte_carlo_scenario_probabilities`

### Artifact Ledger

Rendered or render-ready outputs.

Examples:

- `sections`
- `content_blocks`
- `content_block_metrics`
- `content_block_items`
- `historical_analogues`
- `watch_items`
- `artifacts`
- `generated/report.html`

## Phase 1: Initialize Workspace And Ingest Facts

### Goal

Create a durable per-run workspace with enough deterministic data to support later agent research without repeatedly rediscovering baseline facts.

### Inputs

- Ticker.
- User goal and rough time horizon, if provided.
- SEC Company Facts API.
- Starter market data or quote endpoint.
- Existing product defaults for required report sections and Monte Carlo configuration.

### Work

- Create the run directory and `run.sqlite`.
- Apply the full run schema.
- Fetch and persist raw SEC Company Facts with provenance.
- Fetch starter stock info, quote data, and baseline fundamentals where available.
- Preserve raw facts even when they are not canonical fundamentals.
- Seed required report sections and calculator configuration.
- Record fetch failures, stale values, or missing baseline data as data gaps or quality flags.

### Writes

- Run metadata.
- Stock info.
- Raw SEC facts.
- Starter fundamentals.
- Fundamental observations.
- Data gaps.
- Data quality flags.
- Required empty section rows.
- Default Monte Carlo config.

### Quality Gates

- Workspace exists and can be reopened by later tasks.
- Raw SEC fact ingest preserves taxonomy, concept, unit, period, filing date, accession, and raw JSON.
- Baseline fundamentals are explicitly marked as fetched, missing, stale, or derived.
- Data fetch failures are captured instead of silently ignored.

### Downstream Value

This phase makes the run reproducible. It gives later phases a canonical database to inspect and prevents agent research from becoming a transient chat transcript.

## Phase 2: Build Canonical And Exploratory Fact Catalogs

### Goal

Separate standard fundamentals from company-specific SEC concepts that may contain the most interesting narrative signal.

### Inputs

- Raw SEC facts from Phase 1.
- Product-level canonical metric definitions: the product's required standard measures independent of any one company, such as revenue, gross profit, operating income, net income, EPS, diluted shares, cash, debt, operating cash flow, free cash flow, current price, and market capitalization.
- Full company concept inventory from SEC Facts, including concept names, labels, descriptions, units, periods, filing metadata, and observation counts.
- Canonical concept selection priors, such as known common `us-gaap` aliases. These should guide selection but not be treated as sufficient by themselves.
- SEC Facts insight patterns: a reusable playbook of useful financial mechanics and concept families, such as backlog, conversion, capex, leases, purchase obligations, financing, working capital, capital allocation, dilution, margins, and tax.

### Work

- Link canonical metrics to company-specific SEC concepts using the full concept inventory. Alias heuristics can seed candidates, but an LLM-assisted review should be allowed to inspect the full inventory and decide whether the metric is directly available, should be calculated from multiple concepts, or is unavailable.
- Compute or select core fundamentals such as revenue, net income, EPS, shares, cash, debt, and margins.
- Build a derived concept catalog with concept/unit-level metadata. This is deterministic and can run immediately after ingest, but it belongs conceptually in this phase because it transforms raw facts into a queryable analysis surface.
- Classify period shape deterministically: instant, quarter, year-to-date, annual, or irregular.
- Classify series usability: long history, medium history, sparse, event/point, stale, and plot-ready where applicable. This should not prevent agents from using non-plottable data; it tells downstream workers how to interpret and present the series.
- Tag concepts with reusable narrative categories such as backlog, conversion, capex, lease, purchase obligation, debt, interest, working capital, capital return, dilution, margin, and tax. This should be implemented as a cost-conscious batch operation, not hundreds of one-off tool calls: deterministic keyword/description rules first, then optional LLM batch review for ambiguous or high-potential concepts.
- Normalize selected flow concepts when needed so quarterly, year-to-date, and annual values are not mixed misleadingly.

### Writes

- Canonical metric mappings.
- Canonical fundamental observations.
- Derived concept catalog or equivalent materialized analysis view.
- Period-shape metadata.
- Series-usability and plot-readiness metadata.
- Narrative candidate tags.
- Supporting metric selections for obviously useful concepts.
- Quality flags for stale, mismatched, sparse, or non-comparable series.

### Quality Gates

- Core fundamentals are traceable back to specific SEC concepts or non-SEC sources.
- Flow metrics do not mix quarterly, year-to-date, and annual observations without labeling or normalization.
- Exotic concepts remain available even when they are not promoted to canonical fundamentals.
- Each promoted supporting metric has a rationale.
- LLM-selected canonical mappings include confidence and rationale, and low-confidence mappings are treated as review candidates rather than trusted facts.
- Plot readiness is not used as a proxy for analytical relevance. Sparse or event-like series can still be important if they are labeled correctly.

### Downstream Value

This phase is where SEC Facts start becoming a product advantage. Standard market data can show headline revenue and EPS; this catalog can expose the company-specific mechanics that make a narrative more precise.

## Phase 3: Build Source Pack And Narrative Map

### Goal

Create a source-backed map of what the market currently believes, what management claims, and what might make the stock narrative change.

### Inputs

- Company filings and annual reports.
- Investor relations material.
- Earnings calls and presentations when available.
- Financial media and analyst commentary.
- Company website and product pages.
- Canonical and exploratory fact catalogs from Phase 2.

### Work

- Gather citeable sources.
- Extract claims from sources and assign them to bull, bear, neutral, or mixed sides where useful.
- Identify the dominant market narrative, bull narrative, bear narrative, consensus assumptions, and counter-narrative.
- Draft early business model, industry context, and why-now notes.
- Distinguish facts, management claims, analyst opinions, and agent inferences.
- Record unresolved source gaps.

### Writes

- Sources.
- Claims.
- Narrative map.
- Early orientation section.
- Early business model section.
- Early industry context section.
- Data gaps and research gaps.

### Quality Gates

- Every important factual claim has a source or is explicitly marked as an inference.
- Sources include enough metadata for citations and later review.
- The narrative map contains both consensus and disagreement, not only a bull case.
- Claims are not treated as true merely because they are repeated by management or media.

### Downstream Value

This phase gives the system the "why now" context. It explains what investors are likely reacting to and gives later phases a structured debate to test against company facts.

## Phase 4: Triage Concepts Into Crux Candidates

### Goal

Connect the fact catalog to the narrative map and identify the few mechanics that could actually change the scenario paths.

### Inputs

- Narrative map and claims from Phase 3.
- Canonical fundamentals from Phase 2.
- Exploratory SEC concept catalog from Phase 2.
- SEC Facts insight patterns.

### Work

- Search for SEC concepts that confirm, complicate, or contradict the major narratives.
- Promote useful concepts into supporting metric selections.
- Identify concept clusters, not just isolated metrics.
- Connect each cluster to a possible crux.
- Flag concepts that are interesting but too sparse, stale, or ambiguous for projection.
- Open follow-up questions for financial mechanics experiments.

### Writes

- Supporting metric selections.
- Insight entries or equivalent durable analysis records.
- Crux candidate entries.
- Open questions or signals for experiments.
- Data quality flags for concepts that are tempting but unsafe to use.

### Quality Gates

- A promoted concept must say why it matters to a narrative or crux.
- A crux candidate must be falsifiable or at least watchable.
- Sparse/event concepts are not over-treated as smooth time series.
- Concepts with period-shape problems are labeled before they are plotted or projected.

### Downstream Value

This phase turns raw data abundance into research judgment. It is the bridge from "there are hundreds of SEC concepts" to "these five mechanics are the ones that matter."

## Phase 5: Run Financial Mechanics Experiments

### Goal

Use deterministic scripts and lightweight models to test how crux mechanics affect revenue, margins, cash flow, EPS, multiples, or balance-sheet risk.

### Inputs

- Crux candidates from Phase 4.
- Supporting metric selections.
- Canonical fundamentals.
- Historical time series.
- Source-backed claims.
- Research questions opened by earlier phases.

### Work

- Write focused calculation scripts for specific questions.
- Test alternative normalizations or derived views.
- Compare historical trends against narrative claims.
- Build simple economic bridges, such as backlog conversion, capex intensity, debt-to-interest pressure, share count effects, margin sensitivity, or revenue-per-share paths.
- Record experiment outputs and dispose of each result as promoted, rejected, background, or unresolved.

### Writes

- Calculation-backed insight entries.
- Script outputs or reproducible calculation payloads.
- Financial math content blocks.
- Scenario input recommendations.
- Additional data gaps or contradiction entries.

### Quality Gates

- Each experiment has a clear question.
- Inputs, formulas, assumptions, and units are recorded.
- Results distinguish arithmetic from interpretation.
- Promoted results are linked to source facts or claims.
- Rejected experiments explain why they were not used.

### Downstream Value

This phase is where the report becomes more than prose. It gives scenarios concrete financial mechanics and helps the final output feel non-obvious without becoming numerology.

## Phase 6: Construct Scenarios And Projection Inputs

### Goal

Create company-specific scenario paths that translate narratives and crux assumptions into explicit financial projections.

### Inputs

- Narrative map.
- Crux candidates.
- Financial mechanics experiment results.
- Canonical fundamentals.
- Supporting metrics.
- Historical analogues.
- Current market price and baseline fundamentals.

### Work

- Draft 4-6 company-specific scenarios.
- Include at least one bullish, one neutral, and one bearish path.
- Avoid generic scenario names unless they are genuinely clearest.
- Assign scenario probabilities.
- Define which cruxes are settled in each scenario and how.
- Populate period-level assumptions for revenue, margins, EPS, shares, P/S, P/E, and blend weights.
- Add key sensitivities.
- Add confirming and breaking signals tied to watch items.
- Use historical analogues to inform scenario shape, probability, or multiple assumptions, while noting where analogies can mislead.

### Writes

- Scenario assumptions.
- Scenario crux assumptions.
- Scenario sensitivities.
- Scenario signals.
- Scenario periods.
- Watch items.
- Historical analogue rows.
- Scenario-linked supporting metrics or insights.

### Quality Gates

- Probabilities are present and usually sum to 1.0 before calculator normalization.
- Each scenario has a company-specific narrative path.
- Each scenario has visible period-level financial assumptions.
- Assumptions connect back to cruxes, claims, calculations, or historical analogues.
- Confirming and breaking signals are specific enough to update later.
- Projection language avoids investment-advice framing such as target price or guaranteed upside.

### Downstream Value

This phase creates the product's centerpiece: scenario-conditioned paths that a user can inspect, disagree with, and monitor over time.

## Phase 7: Calculate Distribution And Render Artifacts

### Goal

Run deterministic scenario math, generate the Monte Carlo distribution, validate report readiness, and produce renderable artifacts.

### Inputs

- Scenario assumptions and periods.
- Monte Carlo configuration.
- Baseline fundamentals.
- Sources and claims.
- Financial math content blocks.
- Draft sections and artifacts.

### Work

- Validate required report inputs.
- Calculate scenario period outputs.
- Calculate derived fields such as revenue per share, implied prices, blended price bands, EPS growth, and margin outputs.
- Run Monte Carlo simulation from normalized scenario probabilities and terminal price bands.
- Persist histogram, summary statistics, and scenario probability diagnostics.
- Compile the report payload.
- Render final HTML.

### Writes

- Monte Carlo summary.
- Monte Carlo histogram bins.
- Monte Carlo scenario probabilities.
- Scenario output payload.
- Rendered report artifact.
- Artifact registry rows.
- Validation errors or report readiness flags.

### Quality Gates

- Required fundamentals are present or explicitly marked as missing.
- Sources and claims are non-empty.
- Every scenario has at least one period.
- Scenario periods include enough information to calculate revenue and valuation bands.
- Monte Carlo methodology is documented.
- Rendered report includes source notes and limitations.

### Downstream Value

This phase turns structured research into the visual artifact users will remember: scenario paths, probability distribution, supporting math, and citations.

## Phase 8: Synthesize Report And Refresh Hooks

### Goal

Assemble a coherent final report from reviewed obligations, not from raw context alone, and make the output maintainable for refreshes.

### Inputs

- Reviewed source pack.
- Narrative map.
- Financial math blocks.
- Scenario outputs.
- Monte Carlo outputs.
- Historical analogues.
- Watch items.
- Data gaps and quality flags.

### Work

- Generate final report prose and visual sections from structured rows.
- Ensure the executive summary matches the scenario math.
- Ensure financial math explains why scenario assumptions are economically plausible or implausible.
- Surface data limitations and stale facts.
- Preserve citations.
- Record watch items as future refresh hooks.
- Identify which cruxes should be monitored after earnings, filings, news, or macro changes.

### Writes

- Final sections.
- Content blocks.
- Watch item refinements.
- Source notes and limitations.
- Final rendered artifact.
- Future refresh obligations.

### Quality Gates

- Final report does not introduce unsupported claims.
- Scenario probabilities, summaries, and prose are consistent.
- Citations cover material factual claims.
- Watch items map back to cruxes and scenarios.
- The report clearly labels historical, projected, and mixed periods.
- The tone remains scenario-conditioned rather than investment-advice oriented.

### Downstream Value

This phase makes the report feel coherent and useful. It turns the research database into a user-facing explanation of what matters, what could change, and how different futures could affect the stock story.

## Blackboard Mapping

If the system adopts the blackboard/swarm architecture, the phase contracts above map cleanly to blackboard concepts.

### Entries

Entries are durable units of knowledge.

Analogues examples:

- SEC observation.
- Source-backed claim.
- Narrative analysis.
- Calculation result.
- Data gap.
- Contradiction.
- Crux assumption.
- Historical analogue lesson.

### Signals

Signals are work requests or unresolved questions.

Analogues examples:

- "Find source for backlog conversion claim."
- "Inspect whether debt issuance is recurring or one-time."
- "Normalize capex from YTD to quarterly."
- "Resolve contradiction between management margin guidance and historical gross margin trend."
- "Test EPS sensitivity to interest expense."

### Obligations

Obligations are final-output commitments.

Analogues examples:

- Include source limitation for delayed SEC Company Facts.
- Explain the key crux for each scenario.
- Cite the evidence behind a major claim.
- Include confirming and breaking signals for each scenario.
- Explain why an analogue is useful and why it can mislead.

### Quality Gates

Quality gates should run before entries become trusted downstream inputs.

Analogues examples:

- Observations require source and period.
- Calculations require units and formula.
- Flow metrics require period-shape clarity.
- Scenario assumptions require crux linkage.
- Claims require source custody and confidence.
- Final synthesis must satisfy open obligations or explicitly waive them.

## Design Decisions And Deferred Questions

### First-Class Insights Table

Decision: add a dedicated `insights` table. If we later adopt the blackboard architecture, this table can either become `entries` or map cleanly into a blackboard entries table.

It should likely include:

- Entry type.
- Content.
- Source references.
- Related concept or metric.
- Related scenario or crux.
- Epistemic classification.
- Confidence.
- Status: active, disputed, superseded, quarantined.
- Relations to other insights, such as supports, contradicts, supersedes, or derived-from.
- Worker or phase that created it.
- Created by.
- Created at.

This table is important even for a linear pipeline. It gives agents and deterministic tasks a shared place to record intermediate judgment without forcing everything into final report sections.

### Concept Catalog Materialization

Decision: materialize the concept catalog and its key derived metadata in SQLite, even if some fields can also be exposed as views.

The base catalog can be generated deterministically from `sec_raw_facts`. Period shape should be deterministic. Series usability and plot readiness should also start deterministic, using observation counts, date coverage, latest filing date, period shape consistency, and unit consistency.

Narrative tags should be stored as annotations on catalog rows or in a related concept-tag table. They should not require one model call per concept. The preferred path is:

1. Deterministic keyword and description rules.
2. Batch LLM review over the compact concept inventory.
3. Human or agent promotion of selected concepts into `supporting_metric_selections`.

The value of materialization is agent usability. Downstream workers can ask for "all backlog/conversion concepts with medium or long history" or "all financing-related concepts that recently changed" without rescanning the raw facts every time.

Plot readiness should be treated as presentation metadata, not as a filter on analytical importance. A sparse concept may be useless as a line chart but valuable as an event clue.

### Script Experiment Persistence

Decision: add script experiment tables to the SQLite workspace. RHAI or other financial mechanics scripts should be edited and persisted in SQLite, then executed by a separate tool or task that reads the script, runs it against approved workspace data, and writes the result back.

The minimum useful record is:

- Question.
- Script or formula.
- Inputs.
- Output.
- Interpretation.
- Disposition.
- Linked sources or facts.

Suggested table family:

- `analysis_scripts`: script body, language, purpose, status, created_by, created_at, updated_at.
- `analysis_script_inputs`: linked metrics, concepts, insights, sources, or scenario IDs used by the script.
- `analysis_script_runs`: run timestamp, execution status, stdout/stderr, error message, cost or runtime metadata.
- `analysis_script_outputs`: structured result rows, JSON payloads, promoted insight ID, and disposition.

The script body should be durable because the calculation itself is part of the research evidence. The final report should be able to cite not only the output, but also the calculation path that produced it.

### Refresh Semantics

Deferred decision: the exact refresh policy should wait until we better understand cost, effectiveness, and the final shape of the research loop.

For now, treat refresh semantics as metadata the system should be ready to express rather than a fully decided policy.

Likely stable across routine refreshes:

- Historical sources.
- Long-lived business model description.
- Some historical analogues.
- Previously reviewed insights whose source facts have not changed.
- Canonical metric definitions.

Likely refreshable after new filings, earnings, major news, or material price moves:

- SEC facts.
- Current price.
- Watch item status.
- Crux probabilities.
- Scenario assumptions after earnings or major news.
- Monte Carlo outputs.
- Executive summary and final talk track.
- Data quality flags.

Likely invalidation rules:

- If a raw fact changes, derived observations and concept catalog metadata for that concept become stale.
- If a claim is superseded by a newer source, dependent insights should be marked stale or superseded.
- If a crux changes, dependent scenarios, probabilities, watch items, and Monte Carlo outputs should be regenerated.
- If only current price changes, scenario assumptions may remain valid while implied price-relative framing and distribution presentation refresh.

## Proposed Living Document Format

The main pipeline document should eventually keep a compact version of this structure:

1. Phase name.
2. Responsibility in one sentence.
3. Inputs.
4. Writes.
5. Quality gates.
6. Downstream consumers.

Detailed implementation notes can live in separate phase-specific docs, but the main flow document should always let a reader answer:

- Where does this data first enter the system?
- Where is it validated?
- Where does agent judgment enter?
- Where is arithmetic deterministic?
- Where are sources attached?
- Where are scenarios created?
- Where does final prose come from?
- What gets refreshed when new information arrives?
