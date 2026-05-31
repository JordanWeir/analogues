# Hypothetical Timeline To v1

## Purpose

This is a rough calendar estimate for a human team building Stock Narratives from the current planning docs to a credible v1.

Assumed team:

- 2 full-time developers
- 1 designer, part-time to full-time depending on phase
- No dedicated data engineer, ML engineer, QA engineer, compliance specialist, or product manager

This estimate assumes the team uses LLM agents heavily for implementation support, but the timeline is for human-led product delivery, review, debugging, integration, and decision-making.

## High-Level Estimate

A credible v1 likely takes **8-11 months** with this team.

Aggressive path:

- **32-36 weeks**
- Requires fast vendor/source decisions, limited frontend complexity, narrow coverage, and strong acceptance of rough edges.

Base-case path:

- **38-44 weeks**
- Assumes normal integration drag, some source acquisition surprises, several pipeline redesigns, and meaningful UI iteration.

Conservative path:

- **48-56 weeks**
- Assumes source access/licensing difficulty, complex validation requirements, major frontend iteration, or the need for more robust review/compliance before launch.

## Major Assumptions

- v0.1 starts with a curated AI infrastructure coverage universe.
- Most expensive LLM research runs through scheduled or worker-driven pipelines, not user-triggered request-time generation.
- Reports are persisted and rendered from the database.
- `NarrativeClaim` and `Citation` are first-class in v0.1, but the exact validation strategy is allowed to evolve.
- Historical analogues can be rough early, as long as the product direction is visible.
- Source acquisition begins with a small set of blogs, news sites, strategy sources, and possibly forum-like sources.
- Databento is used for market data/pricing.
- The first v1 does not need institutional-terminal completeness.
- The first v1 does need enough trust, observability, and UX polish that serious users can understand and inspect generated research.

## Phase 0: Architecture And Product Grounding

Estimated duration: **1-2 weeks**

Goal:

Align the team around vocabulary, boundaries, sequencing, and unresolved ADRs before implementation begins.

Primary work:

- Finalize glossary, system map, and delivery plan.
- Draft crate charters for core crates or modules.
- Draft contract specs for source ingestion, LLM gateway, pipeline, validation, similarity, and read models.
- Decide initial frontend direction or explicitly defer it behind backend-rendered view contracts.
- Decide initial validation pipeline strategy: report-first, hybrid, claim-first, or multi-agent reviewer.
- Decide first source list for v0.1.

Developer focus:

- Dev 1: Loco app architecture, crate/module structure, persistence boundaries.
- Dev 2: pipeline contracts, source/LLM/validation seams, test fixture strategy.

Designer focus:

- Product flow sketches for search, stock overview, report sections, historical analogue view, and admin quality dashboard.

Exit criteria:

- Team knows what to build first.
- v0.1 scope is small enough to implement.
- Open uncertainties are isolated behind seams or ADRs.

## Phase 1: Loco Foundation And Domain Skeleton

Estimated duration: **2-3 weeks**

Goal:

Create the app foundation and persistence model needed for the first research workflow.

Primary work:

- Initialize Loco app.
- Configure database, migrations, fixtures, tasks, workers, and scheduler.
- Create initial persistence models:
  - `Stock`
  - `CoverageUniverse`
  - `NarrativeEpisode`
  - `ResearchReport`
  - `ReportSection`
  - `SourceDocument`
  - `Citation`
  - `NarrativeClaim`
  - `PipelineRun`
  - basic `QualitySignal`
- Add seeded AI infrastructure coverage universe.
- Add smoke tests for app boot, migrations, fixtures, and basic read/write paths.

Developer focus:

- Dev 1: Loco app, migrations, models, fixtures, controller skeletons.
- Dev 2: domain types, crate/module boundaries, fake providers, contract test scaffolding.

Designer focus:

- Low-fidelity page structure and information hierarchy for stock overview and admin dashboard.

Exit criteria:

- App runs locally.
- DB schema can support v0.1 artifacts.
- Workers/tasks/scheduler can be invoked.
- Seeded coverage universe appears in a basic internal page or task output.

## Phase 2: v0.1 Source And LLM Pipeline Slice

Estimated duration: **4-6 weeks**

Goal:

Generate the first persisted narrative report for one or two covered stocks from captured sources.

Primary work:

- Implement one or two source adapters.
- Persist normalized `SourceDocument` records.
- Implement LLM gateway with fake provider tests and one real provider.
- Build first research worker:
  - load covered stock
  - collect source documents
  - draft current narrative report sections
  - persist report and pipeline metadata
- Add idempotent re-run behavior.
- Add basic pipeline observability.

Developer focus:

- Dev 1: source persistence, worker/task wiring, pipeline run tracking.
- Dev 2: source adapters, LLM gateway, prompt response parsing, fixture tests.

Designer focus:

- First visual treatment of report page: section layout, source trail, loading/stale states.

Exit criteria:

- A task or scheduled worker can produce a persisted report for at least one stock.
- Report output is stable across page loads.
- Source documents and pipeline runs are inspectable.
- Fake-provider tests cover the main pipeline path.

## Phase 3: v0.1 Historical Analogues And Similarity

Estimated duration: **4-6 weeks**

Goal:

Add the first rough historical analogue experience.

Primary work:

- Seed or generate minimal historical narrative episodes.
- Add similarity interface over `NarrativeEpisode` records.
- Implement fake or simple embedding/index path for early iteration.
- Generate analogue comparison report sections.
- Add analogy breakers.
- Add basic market data return windows through Databento or a temporary fixture-backed adapter.

Developer focus:

- Dev 1: historical episode persistence, report rendering, market data adapter integration.
- Dev 2: similarity engine, embedding/fake-index strategy, analogue generation flow.

Designer focus:

- Historical analogue cards/detail treatment.
- Visual distinction between current narrative, historical analogue, outcome, and analogy breaker.

Exit criteria:

- Stock report includes at least a few historical analogues.
- Each analogue explains why it is useful and why it may be misleading.
- Basic performance/outcome context is visible or clearly marked unavailable.
- Similarity logic is replaceable behind a seam.

## Phase 4: v0.1 Validation And Quality Dashboard

Estimated duration: **4-5 weeks**

Goal:

Make generated research inspectable enough to trust and debug.

Primary work:

- Extract `NarrativeClaim` records from final report sections.
- Persist `Citation` records with citation types.
- Add basic claim types:
  - factual claim
  - source-time narrative claim
  - interpretation claim
  - speculative claim
  - implied performance promise
  - contested claim
- Add support status and citation coverage checks.
- Add risk/quality signals:
  - weak source coverage
  - citation gap
  - high speculation
  - implied performance promise
  - validation failure
- Build internal quality dashboard.
- Add re-run or mark-for-review admin action.

Developer focus:

- Dev 1: dashboard, read models, admin actions, report claim/citation rendering.
- Dev 2: validation pipeline, claim extraction, risk scoring, fixture tests.

Designer focus:

- Admin quality dashboard UX.
- Claim/citation inspection patterns.
- User-facing uncertainty and source-trail presentation.

Exit criteria:

- Final reports have persisted claims and citations.
- Risky reports can be found through dashboard filters.
- Claims and citations can be inspected from report/admin views.
- Publication does not require manual approval, but low-quality output is visible.

## Phase 5: v0.1 Internal Aha Hardening

Estimated duration: **3-4 weeks**

Goal:

Turn the prototype into a coherent internal demo that reliably shows product value.

Primary work:

- Improve prompts and report section structure.
- Tighten source selection and deduplication.
- Add better empty, stale, and failed states.
- Improve report readability.
- Add more curated AI infrastructure stocks.
- Add enough fixtures to catch regressions.
- Run repeated report-generation cycles and compare quality.

Developer focus:

- Dev 1: UI polish, read model stability, pipeline re-run/admin flows.
- Dev 2: prompt iteration, pipeline reliability, source quality, validation fixtures.

Designer focus:

- High-fidelity stock overview page.
- Report hierarchy, typography, source/citation affordances, and analogue layout.

Exit criteria:

- A user can search a covered ticker and understand the current market story in under three minutes.
- Historical analogues feel directionally useful.
- The system is visibly different from a generic LLM stock summary.
- The team can repeatedly regenerate and inspect reports without manual database surgery.

Milestone:

- **v0.1 internal prototype**

Likely cumulative time:

- **18-26 weeks**

## Phase 6: v0.5 Automated Narrative Episode Library

Estimated duration: **8-10 weeks**

Goal:

Move from a contrived curated demo toward repeatable narrative indexing and broader coverage.

Primary work:

- Expand source adapters and source discovery.
- Improve source-time reconstruction.
- Improve episode drafting and episode update behavior.
- Add richer pipeline observability:
  - prompt/model versions
  - stage inputs/outputs
  - cost/token tracking
  - retry/failure views
- Improve similarity indexing and structured similarity attributes.
- Add scenario generation and pivot detection.
- Add better historical outcome enrichment.
- Add current narrative cluster browsing.
- Add review tools for medium/high-risk narratives.

Developer focus:

- Dev 1: workflow UX, admin/review flows, cluster browsing, report/read-model expansion.
- Dev 2: pipeline robustness, source adapters, similarity, market data, validation improvements.

Designer focus:

- Narrative cluster browsing.
- Scenario/pivot presentation.
- Review workflow and observability surfaces.

Exit criteria:

- System can generate useful first drafts for a broader curated universe.
- Similarity retrieval returns plausible historical analogues.
- Quality dashboard helps decide what needs attention.
- Pipeline behavior can be debugged without reading logs only.

Milestone:

- **v0.5 automated episode library**

Likely cumulative time:

- **26-36 weeks**

## Phase 7: v1 Research Workspace UX

Estimated duration: **6-8 weeks**

Goal:

Build the user-facing workspace features that turn the tool from a demo into a product.

Primary work:

- Polish ticker search and stock overview.
- Add narrative timeline.
- Add scenario tree view.
- Add historical analogue explorer and analogue detail page.
- Add source explorer.
- Add similar current narratives page.
- Add watchlists and saved research.
- Add basic alerts or upcoming pivot reminders.
- Add shareable/exportable research pages if still in scope.
- Add user accounts if not already present.

Developer focus:

- Dev 1: frontend/product surfaces, read models, auth/user features.
- Dev 2: backend support for timelines, watchlists, alerts, source explorer, saved research.

Designer focus:

- Main product UX, visual system, interaction states, information density, onboarding, and trust affordances.

Exit criteria:

- Product has the major v1 screens from the product brief.
- Users can return to monitor narratives, not just run one-off lookups.
- Source and uncertainty presentation is clear.
- Workspace features feel integrated rather than bolted on.

## Phase 8: Beta Hardening And Launch Readiness

Estimated duration: **4-6 weeks**

Goal:

Make v1 stable enough for external beta users.

Primary work:

- Reliability hardening.
- Performance tuning.
- Queue/scheduler operational checks.
- Error handling and observability.
- Security pass.
- Backups and basic operational runbooks.
- UX polish from beta feedback.
- Legal/compliance review if commercial launch is likely.
- Source licensing review.
- Regression test expansion.
- Prompt regression fixtures.

Developer focus:

- Dev 1: app reliability, UX fixes, auth/security, deployment.
- Dev 2: pipeline reliability, source failures, validation regressions, monitoring.

Designer focus:

- Final polish, beta feedback triage, onboarding, empty states, trust/safety copy.

Exit criteria:

- External users can use the product without handholding.
- Pipeline failures are visible and recoverable.
- Generated content has inspectable source/claim/quality metadata.
- Product avoids recommendation-like behavior.
- Team has a credible operating model for recurring refreshes.

Milestone:

- **v1 beta / launch candidate**

Likely cumulative time:

- **38-44 weeks base case**

## Timeline Summary

| Phase | Duration | Cumulative |
| --- | ---: | ---: |
| Phase 0: Architecture and grounding | 1-2 weeks | 1-2 weeks |
| Phase 1: Loco foundation and domain skeleton | 2-3 weeks | 3-5 weeks |
| Phase 2: Source and LLM pipeline slice | 4-6 weeks | 7-11 weeks |
| Phase 3: Historical analogues and similarity | 4-6 weeks | 11-17 weeks |
| Phase 4: Validation and quality dashboard | 4-5 weeks | 15-22 weeks |
| Phase 5: Internal aha hardening | 3-4 weeks | 18-26 weeks |
| Phase 6: Automated episode library | 8-10 weeks | 26-36 weeks |
| Phase 7: v1 research workspace UX | 6-8 weeks | 32-44 weeks |
| Phase 8: Beta hardening and launch readiness | 4-6 weeks | 38-50 weeks |

Base-case expectation:

- **v0.1 internal prototype:** 4.5-6 months
- **v0.5 automated episode library:** 6.5-9 months
- **v1 beta / launch candidate:** 9-12 months

## Why This Is Not A 3-Month v1

The hard parts are not only CRUD screens or LLM calls.

The project has several compounding workstreams:

- source acquisition and normalization
- historical source-time reconstruction
- LLM report generation
- persistent report/version behavior
- claim/citation extraction
- validation and quality scoring
- similarity search
- market data enrichment
- admin observability
- user-facing research UX
- trust/safety positioning

Two developers can make fast progress if boundaries are clean, but v1 requires repeated integration and quality passes. The system has to be useful, inspectable, and stable, not merely able to produce plausible text once.

## Biggest Timeline Risks

### Source Access And Licensing

If the desired sources cannot be accessed, searched, cached, or reused cleanly, source ingestion may need major redesign.

Potential impact:

- +4-12 weeks

### Historical Analogue Quality

Useful historical analogues require more than vector similarity. They need source-time reconstruction, outcome context, and analogy breakers.

Potential impact:

- +4-8 weeks

### Validation Strategy Churn

If the team changes from report-first validation to claim-first generation or multi-agent review midstream, pipeline contracts and persistence models may churn.

Potential impact:

- +3-8 weeks

### Frontend Scope

React-heavy workspace UX may take longer than backend-driven Datastar/HTMX screens, especially with timeline, scenario tree, source explorer, and admin dashboard interactions.

Potential impact:

- +3-8 weeks

### Compliance Or Trust Requirements

If legal/compliance review imposes stricter controls, manual approval, disclaimers, audit logs, or source requirements, the product may need additional review infrastructure.

Potential impact:

- +4-12 weeks

### Pipeline Observability

LLM pipelines are hard to debug without prompt/run/stage visibility. Underbuilding observability early can slow every later phase.

Potential impact:

- +2-6 weeks

## Possible Ways To Shorten The Timeline

### Narrow v1 Coverage

Keep v1 coverage to a curated universe instead of broad S&P 500 coverage.

Likely savings:

- 4-8 weeks

### Defer Full Workspace Features

Ship v1 without alerts, exports, or rich watchlists.

Likely savings:

- 3-6 weeks

### Use Typed Report Sections Before Evidence Graph

Persist report sections, claims, and citations without building a full evidence graph.

Likely savings:

- 4-8 weeks

### Choose Backend-Driven UI

Use Loco-rendered views with Datastar/HTMX-style interaction for admin and report pages.

Likely savings:

- 3-6 weeks versus a custom React-heavy app

### Start With Seeded Historical Episodes

Seed or semi-curate the first historical episodes while building the generation pipeline in parallel.

Likely savings:

- 3-5 weeks to first "aha" moment

## Suggested Delivery Posture

Do not treat v1 as one giant launch.

Recommended posture:

1. Build v0.1 around one excellent stock page.
2. Add rough but visible historical analogues early.
3. Add validation/quality visibility before expanding coverage.
4. Expand automation only after the report shape is convincing.
5. Build workspace features once users want to return and monitor narratives.

The product should earn scope by demonstrating that the generated narrative research is meaningfully better than a generic LLM summary.
