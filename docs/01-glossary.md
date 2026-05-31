# Glossary: Stock Narratives

## Purpose

This glossary defines the common product and architecture language for Stock Narratives.

It is meant to help future system docs, crate charters, contract specs, ADRs, and agent task sheets use terms consistently. Definitions here should clarify boundaries and reduce coordination mistakes; they should not freeze implementation details that belong in later contracts.

## Product Terms

### Stock Narratives

The product: a narrative research workspace for market stories.

Stock Narratives helps users understand the story currently driving a stock, compare that story to similar current and historical market narratives, and identify the pivots that could validate, weaken, break, or transform the thesis.

It is not a stock-picking bot, recommendation engine, generic finance chatbot, charting app, or Bloomberg clone.

### Market Story

Plain-language product term for the explanation market participants appear to be using to interpret a stock.

Use this term in user-facing copy when `NarrativeEpisode` would feel too technical.

### Narrative

A market story or interpretation about why a stock matters now.

A narrative may involve fundamentals, expectations, sentiment, positioning, valuation, credibility, regulation, product cycles, hype cycles, or macro conditions.

Use carefully: a stock can have multiple competing narratives at the same time.

### Narrative Episode

The core product object.

A time-bounded market story about why a company's business performance, valuation, risk profile, or strategic position may change.

Important distinctions:

- A stock can have many narrative episodes over time.
- A stock can have multiple active or competing narrative episodes at once.
- Historical analogues should compare narrative episodes, not permanent company identities.

### Current Narrative

The most relevant or dominant narrative currently associated with a stock.

This is usually the first story shown on a stock page, but it should not imply there are no competing narratives.

### Dominant Narrative

The narrative that appears to be most important to current market interpretation.

This is a product judgment based on sources and model interpretation, not a sourced fact by itself.

### Bull Narrative

The strongest optimistic interpretation of the stock's current setup.

It should explain what bulls believe, which assumptions must hold, and which pivots would strengthen the case. It must not become a buy recommendation.

### Bear Narrative

The strongest skeptical interpretation of the stock's current setup.

It should explain what bears believe, which assumptions are contested, and which pivots would weaken or break the bull case. It must not become a sell recommendation.

### Consensus Narrative

The interpretation that appears broadly accepted or priced in.

Use cautiously. Consensus is often inferred from secondary sources, market commentary, valuation context, and repeated claims rather than directly observed.

### Counter-Narrative

An emerging or minority interpretation that challenges the dominant narrative.

Counter-narratives are important because they may become the next dominant story if evidence changes.

### Narrative Timeline

A stock-level view of how market stories changed over time.

Timeline items should usually be narrative episodes with date ranges, transition reasons, key sources, and major pivots.

### Narrative Cluster

A group of current or historical narrative episodes with similar structure.

Examples:

- AI infrastructure buildout
- regulatory overhang
- operating leverage turnaround
- product launch ramp
- fraud or credibility crisis
- commodity windfall

### Similar Current Narrative

An active narrative episode in another stock that resembles the current stock's narrative.

This supports discovery and comparative research.

### Historical Analogue

A past narrative episode that is structurally similar to a current narrative.

Historical analogues are tools for reasoning, not forecasts. Every historical analogue should include why it is useful and why it may be misleading.

### Analogy Breaker

A material difference that weakens or limits a historical comparison.

Examples include different margin profile, customer concentration, valuation environment, market structure, supply constraints, or regulatory exposure.

### Historical Outcome

A structured summary of what happened after a historical narrative episode.

This may include stock return, relative return, revenue growth change, margin change, valuation change, estimate revisions, key pivot events, and narrative resolution type.

Historical outcome data must be kept conceptually separate from source-time evidence to reduce hindsight leakage.

### Scenario

A possible future narrative path, usually over a 6-12 month horizon.

Scenarios should support conditional thinking, not prediction. They should describe what would cause the path, what would confirm or weaken it, and how it would affect the narrative.

### Pivot

An observable event, metric, disclosure, or development that could validate, weaken, break, or transform a narrative.

Examples:

- margin guidance
- backlog disclosure
- customer concentration disclosure
- regulatory decision
- filing delay
- product adoption metric
- short report
- management change

### Key Assumption

An assumption that must be true for a narrative to work.

Examples:

- AI capex keeps growing.
- Margins stabilize.
- Regulatory risk is overblown.
- Customers do not churn.
- Competition does not compress pricing.

### Key Metric

A metric users should watch because it bears directly on a narrative or pivot.

Examples:

- revenue growth
- gross margin
- backlog
- estimate revisions
- customer concentration
- valuation multiple

### Source Trail

The set of sources and citations behind a narrative report, report section, claim, metric, or historical analogue.

The source trail is a trust feature. It should help users inspect why the system said something.

### Source-Time Evidence

Evidence from the period being studied.

For a historical analogue, source-time evidence is what people could know or were saying at the time, before the outcome was known.

This is separate from modern interpretation and historical outcome data.

### Hindsight Leakage

The error of letting later-known outcomes contaminate reconstruction of what was knowable or believed at the time.

Avoiding hindsight leakage is central to trustworthy historical analogues.

### Modern Interpretation

The system's present-day synthesis of what happened, why it mattered, and how it may or may not compare to a current narrative.

Modern interpretation may use outcome data, but it should be labeled separately from source-time evidence.

### Research Workspace

The broader v1 product experience where users save and track stocks, narratives, historical analogues, scenarios, pivots, watchlists, alerts, and research pages.

### Magic Moment

The first user-visible "aha" experience.

For v0.1, this means searching a covered AI infrastructure ticker and seeing a stable, source-backed narrative report with current narrative, bull/bear/counter framing, key assumptions, key pivots, and rough historical analogues.

## Trust And Safety Terms

### Recommendation

A buy, sell, hold, target price, allocation, or personalized investment instruction.

Stock Narratives must not provide recommendations.

### Research Context

The product's intended output: sourced context, competing interpretations, scenarios, historical comparisons, and pivots that help users reason.

Research context is allowed; investment advice is not.

### Fact

A claim about something that happened or can be checked directly.

Example: "Revenue grew 35% year over year."

Facts should be cited when material.

### Interpretation

The system's synthesis of what facts, claims, and market commentary may mean.

Example: "The key debate is whether margin compression is temporary or structural."

Interpretations should be clearly distinguishable from facts.

### Speculation

Forward-looking or uncertain reasoning about what could happen.

Speculation is not forbidden, but it should be labeled and risk-scored when material.

### Uncertainty Label

An explicit marker that source coverage, evidence agreement, analogy quality, or confidence is limited.

Examples:

- insufficient source coverage
- conflicting evidence
- unclear narrative dominance
- sparse historical analogues
- high dependence on future catalysts

### Implied Performance Promise

A statement that strongly suggests future stock or business performance, even if it avoids explicit recommendation language.

These claims should be detected and risk-scored because they can drift toward advice-like behavior.

### Quality Signal

A persisted indicator used to detect potentially low-quality or risky content.

Examples:

- citation gap
- high speculation weight
- weak source coverage
- conflicting sources
- failed validation step
- implied performance promise

### Risk Score

A validation output estimating whether a report, section, or claim may require extra review.

Risk score is not investment risk. It is content-quality and trust/safety risk.

## Evidence And Validation Terms

### Source Document

A captured source used as evidence.

Examples include news articles, blog posts, forum discussions, press releases, transcripts, filings, investor presentations, and archived pages.

The application should store normalized source text and source identity metadata such as URL, vendor ID, publisher, publication date, and retrieval date.

### Citation

A reference from a report section, claim, metric, scenario, analogue, or outcome to a source document or source span.

Important rule:

- A citation is broader than a `NarrativeClaim`.
- Not every citation is a claim.
- Claims usually need citations, but citations can also support metrics, outcomes, source trails, or generated interpretation.

### Citation Type

A classification describing what a citation is doing.

Initial citation types may include:

- factual support
- narrative-at-the-time support
- contested evidence
- metric source
- historical outcome source
- analogue comparison source
- generated interpretation support

### Citation Coverage

The degree to which important report statements, claims, metrics, and comparisons have adequate citations.

Citation coverage is a validation concern, not just a rendering concern.

### Narrative Claim

A first-class persisted assertion made by the system or found in source material.

Claims are used for validation, quality scoring, citation coverage, and auditability.

Important distinctions:

- `NarrativeClaim` belongs primarily to the validation framework.
- Final reports should have extractable claims.
- Claim extraction strategy is still open.
- Not all citations are claims.

### Claim Type

A classification describing the nature of a `NarrativeClaim`.

Initial claim types may include:

- factual claim
- source-time narrative claim
- market-belief claim
- interpretation claim
- speculative claim
- implied performance promise
- contested claim

### Support Status

The validation state of a claim relative to available sources.

Examples:

- supported
- partially supported
- contested
- unsupported
- unclear
- not yet checked

### Opposing Source

A source that challenges, qualifies, or contradicts a claim.

The product should preserve disagreement instead of collapsing evidence into one blended summary.

### Validation

The framework that checks generated artifacts for claims, citation coverage, speculation, implied performance promises, source quality, and other risk signals.

Validation does not necessarily block publication in v0.1. It should at minimum create inspectable metadata and dashboard signals.

### Validation Framework

The part of the system responsible for persisted claims, citations, risk scoring, quality signals, and report audits.

This is conceptually separate from the generation framework, even if v0.1 implements them close together.

### Generation Framework

The part of the system responsible for drafting narrative episodes, report sections, historical analogue comparisons, scenarios, and other generated artifacts.

Generation should produce content; validation should inspect and score that content.

### Report Audit

A validation pass over a generated report or report section.

An audit may extract claims, link citations, classify speculation, flag citation gaps, and produce quality signals.

## System And Architecture Terms

### System Map

A high-level architecture artifact showing major runtime components, candidate crates, seams, dependency direction, and key data flows.

It should guide implementation sequencing and parallel agent work without fully designing every internal module.

### Crate

A Rust packaging and compilation boundary.

Use a crate when the system needs stronger ownership, dependency control, reuse, or a stable public surface.

### Seam

Any meaningful interface where work can be split and validated independently.

A seam might be a crate boundary, trait, protocol/schema boundary, adapter normalization boundary, storage interface, or event stream contract.

Not every seam needs its own crate.

### Contract

The public expectations at a seam.

A contract should define public types or traits, behavioral rules, invariants, error semantics, examples, and required tests.

### Contract Test

A test that verifies behavior at a seam.

Examples include fixture normalization tests, serialization tests, trait conformance tests, deterministic fake-provider tests, valid/invalid examples, and compatibility tests.

### Fixture

A concrete example input or output used to test contracts and keep agent work aligned.

Fixtures are first-class architecture tools in this project.

### External Architecture

The parts of a crate or module that other parts of the system are allowed to rely on.

Examples include public types, traits, functions, DTOs, schemas, error surfaces, config shapes, invariants, and ordering expectations.

External architecture should be precise.

### Internal Architecture

Implementation details that should remain free to change.

Examples include helper types, internal modules, algorithms, storage details, caching choices, and local refactors.

Internal architecture should stay flexible early.

### Boundary Design

The practice of defining the important seams, responsibilities, dependency direction, and contracts before over-designing internals.

This project pulls boundary design forward because multiple LLM agents may work in parallel.

### Crate Charter

A short ownership document for a crate.

It should define purpose, owns, does not own, public entry points, allowed dependencies, forbidden dependencies, and key invariants.

### Contract Spec

A short specification for a seam.

It should define public types or traits, operations, behavioral rules, error semantics, valid examples, invalid examples, and required tests.

### ADR

Architecture Decision Record.

Use ADRs for cross-cutting structural decisions that affect multiple crates or seams, such as validation pipeline strategy, frontend direction, citation taxonomy, or source-time modeling.

### Task Sheet

A short implementation brief for an agent task.

It should define objective, read-first docs, allowed changes, forbidden changes, required tests, and definition of done.

## Runtime And Implementation Terms

### Loco App

The main Rust web application built with Loco.

It owns controllers, views/templates, SeaORM models and migrations, worker registration, scheduler/task wiring, and product runtime configuration.

### Controller

A Loco HTTP request adapter.

Controllers should stay thin: parse requests, call read models or domain/pipeline entry points, and render responses. They should not contain research logic, source ingestion, or LLM orchestration.

### Worker

A Loco background job handler.

Workers should handle slow, retryable, IO-heavy research workflows such as source ingestion, report generation, similarity enrichment, validation, and backfills.

### Scheduler

Loco's recurring automation mechanism.

The scheduler should trigger recurring coverage updates and eventually daily or event-triggered research workflows.

### Task

A Loco operational command.

Tasks are for seeding data, one-off imports, diagnostics, backfills, data fixes, and manual pipeline triggers. They should not become a separate hidden implementation of the research pipeline.

### Research Pipeline

The async workflow that gathers sources, generates or updates narrative artifacts, finds analogues, validates claims/citations, persists outputs, and records run metadata.

The pipeline should be idempotent where practical and observable enough to debug.

### Pipeline Stage

One explicit step in a research pipeline.

Examples:

- source discovery
- source normalization
- narrative drafting
- analogue retrieval
- analogue comparison generation
- claim extraction
- citation audit
- risk scoring
- persistence

### Pipeline Run

A persisted execution record for a research workflow.

It should track what ran, when, why, with which inputs, and with enough prompt/model/version metadata to debug generated outputs.

### Read Model

A query-oriented shape used by controllers to render pages or dashboards.

Read models should shield controllers from complicated persistence details and provide stable page data for stock overviews, report pages, analogue pages, and quality dashboards.

### Application Database

The primary persisted store for domain objects, source documents, reports, claims, citations, pipeline runs, quality signals, and similarity metadata.

The database is the source of truth for user-visible reports.

### Coverage Universe

The set of stocks the system is allowed or scheduled to research.

For v0.1, this is a curated AI infrastructure universe. It should be represented in data, not hardcoded into pipeline internals.

### Curated Universe

A deliberately limited coverage universe chosen for quality and speed.

For v0.1, this allows the product to prove the narrative research experience without solving all-stock coverage.

### Source Ingestion

The subsystem that searches, fetches, normalizes, deduplicates, and persists source documents.

It should hide source-specific quirks behind adapter contracts.

### Source Adapter

A provider-specific implementation that fetches or searches a source and normalizes results into internal source document records.

Adapters should not generate final narrative reports.

### Market Data Adapter

The subsystem that wraps market data providers such as Databento.

It should provide stable internal access to price series, return windows, and eventually relative return context.

### Databento

The preferred market data/pricing provider for this project unless later ADRs decide otherwise.

### LLM Gateway

The subsystem that centralizes calls to LLM providers.

It owns provider details, prompt execution, structured parsing, retries, error mapping, and run metadata. It should not own product-level narrative decisions.

### Similarity Engine

The subsystem that finds similar narrative episodes.

For v0.1, similarity should use `NarrativeEpisode` embeddings plus structured episode properties, not raw source chunk search.

### Embedding

A vector representation used for similarity search.

Embeddings are useful, but they should not be the only source of similarity. Structured metadata and generated explanations are also needed.

### Structured Similarity Attribute

A non-vector attribute used to explain or filter narrative similarity.

Examples:

- narrative type
- sector
- business model
- catalyst type
- valuation setup
- margin profile
- credibility risk
- regulatory exposure
- customer concentration

### Explainable Similarity

A similarity result that explains why two narrative episodes are comparable and where the comparison breaks.

The product should avoid black-box similarity scores as user-facing answers.

### Report

A persisted user-facing research artifact.

Reports are stable outputs users read. They group report sections, citations, claims, validation status, and quality metadata.

### Report Section

A structured section inside a report.

Examples include Current Narrative, Bull Case, Bear Case, Emerging Counter-Narrative, Key Assumptions, Key Pivots, Historical Analogues, Analogy Breakers, and Similar Current Narratives.

### Draft Artifact

An intermediate generated output that has not yet become a stable report section or domain object.

Draft artifacts may be useful for debugging but should not be treated as final user-visible truth.

### Persisted Artifact

A generated or derived object saved to the database for stable reuse, inspection, and rendering.

Examples include reports, report sections, narrative episodes, claims, citations, historical outcomes, and quality signals.

### Quality Dashboard

An internal admin surface for detecting risky or low-quality generated content.

In v0.1, this should focus on visibility and risk detection rather than mandatory approval gates.

### Admin Review

Human inspection or correction of generated content.

Admin review may become more important later, but v0.1 should not depend on manual review before every narrative page can be shown.

## Pipeline Strategy Terms

### Report-First Validation

A pipeline option where the system generates report sections first, then extracts and validates claims from the final report.

This is fast and validates what the user will read, but may discover evidence problems late.

### Claim-First Generation

A pipeline option where the system extracts source claims first, builds a claim graph, then generates report sections from that structured evidence.

This is more disciplined but may be too heavy for the first prototype.

### Hybrid Claim Flow

A pipeline option where the system extracts coarse source claims/themes, generates the report, then audits final report claims.

This may be the best balance for v0.1 if report-first validation is too loose.

### Multi-Agent Reviewer Flow

A pipeline option where separate generation, claim review, citation review, risk review, and aggregation agents collaborate.

This may improve trust but adds orchestration cost and debugging complexity.

## Initial Stage Terms

### v0.1

The curated narrative research prototype.

Goal: reach an internal "aha" moment for a small AI infrastructure coverage universe using persisted reports, rough historical analogues, first-class claims/citations, and visible quality metadata.

### v0.5

The automated narrative extraction and episode library stage.

Goal: move from curated prototype toward repeatable source ingestion, episode drafting, similarity retrieval, scenario generation, pivot detection, and reviewable automation.

### v1

The polished narrative research workspace.

Goal: deliver ticker search, narrative overview, timeline, scenario tree, key pivots, historical analogue explorer, similar current narratives, watchlists, alerts, source explorer, saved research, and shareable outputs.

## Naming Guidance

Use these terms in docs and code when possible:

- Use `Stock` for the security/company entry point.
- Use `NarrativeEpisode` for the core time-bounded story.
- Use `ResearchReport` or `Report` for the persisted user-facing artifact.
- Use `ReportSection` for structured generated sections.
- Use `SourceDocument` for captured evidence sources.
- Use `Citation` for source references.
- Use `NarrativeClaim` for persisted auditable assertions.
- Use `Validation` for claim/citation/risk audit work.
- Use `Generation` for drafting report content.
- Use `Similarity` for analogue/current-narrative retrieval.
- Use `CoverageUniverse` for the set of stocks researched by automation.

Avoid these ambiguities:

- Do not use `narrative` as if each stock has only one.
- Do not use `citation` and `claim` interchangeably.
- Do not use `risk score` to mean investment risk.
- Do not use `historical analogue` as if it predicts the future.
- Do not use `source-time evidence` and `modern interpretation` interchangeably.
- Do not put research generation in controllers.
