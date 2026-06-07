# Contract Spec: ReportPayloadCompiler

## Overview

`ReportPayloadCompiler` loads report-ready workspace and board state into the structured JSON payload used by artifact renderers.

The seam is a compiler, not a researcher. It should validate readiness, collect already-reviewed facts, claims, scenario outputs, Monte Carlo results, sections, limitations, and source notes, then produce a deterministic payload for HTML or future renderers.

It may persist artifact metadata through a separate artifact store, but it should not fetch data, choose canonical concepts, run scenario math, sample Monte Carlo, or invent missing prose.

## Public Types / Traits

- `ReportPayloadCompiler`: service or module responsible for report payload assembly.
- `ReportCompileRequest`: workspace handle, artifact format, optional section filter, and strictness mode.
- `ReportPayload`: structured JSON or typed equivalent containing company, generated-at timestamp, source pack, claim table, financial snapshot, historical growth, data quality, sections, scenarios, Monte Carlo, watch items, historical analogues, limitations, and artifacts.
- `ReportReadinessResult`: pass/fail status, blocking errors, warnings, waived obligations, and limitations.
- `SectionPayload`: section key, title, body or structured blocks, source references, payload metadata.
- `DataQualityPayload`: fetch status, gaps, flags, metric coverage, observation counts.
- `ArtifactRenderRequest`: payload, template, output path, title, generated-at timestamp.
- `ReportCompileError`: missing required inputs, unsatisfied obligation, stale dependency, invalid section payload, render failure.

## Operations

- `compile(request) -> ReportPayload`
- `validate_readiness(workspace, board) -> ReportReadinessResult`
- `load_company(workspace) -> CompanyPayload`
- `load_source_pack(workspace, board) -> SourcePackPayload`
- `load_financial_snapshot(workspace) -> FinancialSnapshotPayload`
- `load_historical_growth(workspace) -> HistoricalGrowthPayload`
- `load_scenarios(workspace, board) -> ScenarioPayload`
- `load_monte_carlo(workspace) -> MonteCarloPayload`
- `load_sections(workspace, board) -> SectionsPayload`
- `load_limitations(workspace, board) -> LimitationsPayload`
- `render(payload, renderer) -> Artifact`
- `record_artifact(workspace, artifact)`

## Behavioral Rules

- The compiler reads from persisted workspace and board state.
- The compiler does not make external network calls.
- The compiler does not call model providers.
- The compiler does not recalculate scenario paths or Monte Carlo distributions.
- Required report obligations must be satisfied, waived, or carried into visible limitations.
- Material factual claims require source references or explicit limitation treatment.
- Scenario prose must match persisted scenario math and Monte Carlo outputs.
- Data gaps and quality flags are included when material to user interpretation.
- Missing optional sections may be omitted only when readiness rules allow it.
- Missing required sections are blocking errors in strict mode.
- Section bodies that contain JSON are parsed as structured payloads; plain text remains plain text.
- Payload generation is deterministic except for the `generated_at` timestamp.
- Artifact rendering consumes `ReportPayload`; renderers should not query arbitrary workspace tables.

## Error Semantics

- Missing required fundamentals returns a readiness error.
- Missing source pack or claims returns a readiness error while those are required for v0.1 report generation.
- Missing scenario assumptions or periods returns a readiness error.
- Missing Monte Carlo output returns a readiness error when scenario-conditioned distribution is required.
- Unsatisfied high-priority obligations block compilation in strict mode.
- Waived or low-priority unresolved obligations are carried into limitations.
- Invalid JSON in a section body falls back to plain text only when the section contract allows plain text.
- Template read or artifact write failures return render errors with path context.
- Artifact registration failure should fail the operation after the file write is reported clearly.

## Valid Examples

- The compiler loads already-persisted scenario periods and Monte Carlo summary into `scenario_data`.
- A delayed SEC Company Facts warning appears in `data_quality` and source limitations.
- A financial math section stored as structured content blocks is emitted as structured JSON for the renderer.
- A missing low-priority open question appears in limitations after being waived.
- HTML rendering receives a complete payload and records a `report_html` artifact.

## Invalid Examples

- The compiler fetching SEC facts because revenue is missing.
- The compiler using a model call to fill `business_model` during render.
- The compiler recalculating Monte Carlo because no persisted summary exists.
- A renderer querying `scenario_periods` directly instead of receiving scenario payload data.
- Omitting source limitations for stale or partial financial fetches.

## Required Tests

- Fails readiness when required fundamentals are missing.
- Fails readiness when required source and claim records are missing.
- Fails readiness when scenarios or scenario periods are missing.
- Includes data gaps and quality flags in payload.
- Parses structured section JSON and preserves plain text section bodies.
- Emits source pack and claim table with source links.
- Emits scenario data from persisted calculator output.
- Emits Monte Carlo data from persisted engine output.
- Carries waived obligations into limitations.
- Does not perform provider fetches, model calls, or scenario recalculation.
- Records rendered artifact metadata with path and created-at timestamp.

