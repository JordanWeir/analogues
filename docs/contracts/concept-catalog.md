# Contract Spec: ConceptCatalog

## Overview

`ConceptCatalog` owns deterministic cataloging of raw financial facts and the bridge from provider vocabulary to product-usable observations.

The seam materializes SEC concepts, canonical metric definitions, company-specific canonical mappings, derived observations, and exploratory concept metadata. It keeps raw provider facts available while giving workers and calculations compact, reliable query surfaces.

The catalog should be useful before model-backed review exists. Model-backed concept review may annotate or promote concepts later, but deterministic extraction and period classification are the base contract.

## Public Types / Traits

- `ConceptCatalog`: service or module responsible for catalog materialization and query.
- `CanonicalMetricDefinition`: canonical key, metric key, label, statement type, unit hint, display order.
- `CanonicalMetricMapping`: canonical key, taxonomy, concept name, unit, confidence, rationale, selected-by, active flag.
- `ConceptCatalogRow`: taxonomy, concept name, label, description, unit, fact count, earliest/latest period, latest filed date, min/max value, period-shape metadata, plot-readiness metadata, narrative tags.
- `FundamentalObservation`: canonical key, metric key, label, statement type, period type, period start/end, as-of date, filed date, fiscal year, fiscal period, value, unit, source metadata, quality, derived flag.
- `SupportingMetricSelection`: selection scope, optional scenario, taxonomy, concept, unit, label, rationale, selected-by.
- `PeriodShape`: instant, quarter, YTD, annual, TTM, projected, mixed, unknown.
- `CatalogError`: missing raw facts, invalid mapping, unit mismatch, ambiguous mapping, persistence failure.

## Operations

- `seed_canonical_definitions(store)`
- `materialize_raw_fact_catalog(store)`
- `seed_canonical_mappings(raw_facts, definitions) -> Vec<CanonicalMetricMapping>`
- `activate_mapping(mapping)`
- `deactivate_mapping(mapping_id, reason)`
- `build_observations(raw_facts, active_mappings) -> Vec<FundamentalObservation>`
- `build_ttm_series(canonical_key, raw_facts, active_mappings) -> Vec<FundamentalObservation>`
- `select_latest_baseline_bundle(observations) -> BaselineFinancialBundle`
- `select_supporting_metric(selection_request)`
- `query_catalog(filters) -> Vec<ConceptCatalogRow>`

## Behavioral Rules

- Raw facts are never deleted or hidden because they fail canonical mapping.
- Canonical definitions are product-level defaults; canonical mappings are company-specific and reviewable.
- Mapping activation is explicit and auditable.
- The same raw concept may be useful in exploratory catalog views without becoming canonical.
- Period-shape classification is deterministic.
- TTM rows are derived only from coherent period windows or clearly marked annual fallbacks.
- Derived observations must identify their source facts or calculation rationale.
- Unit hints are validation aids, not proof that a concept is analytically correct.
- Sparse concepts may be marked poor for plotting while remaining analytically important.
- Model-backed annotations may add tags, confidence, or rationale, but should not replace deterministic catalog materialization.
- Catalog queries should support workers asking for compact sets such as "backlog-like concepts with multi-period history" or "financing-related concepts with recent changes."

## Error Semantics

- Missing raw facts returns an empty catalog only when the workspace explicitly has no fetched facts; otherwise it is a data-gap condition.
- Unit mismatch prevents activation of a canonical mapping unless explicitly overridden with rationale.
- Ambiguous active mappings for the same canonical key and unit should produce a warning or validation error before baseline metrics are selected.
- TTM construction that cannot find a coherent window returns no TTM row and records fallback quality only if an annual fallback is used.
- Derived observations with missing source period or unit are rejected or quarantined before becoming trusted inputs.
- Persistence failures include table and operation context.

## Valid Examples

- `RevenueFromContractWithCustomerExcludingAssessedTax` maps to canonical `revenue` for a company when unit and period history are usable.
- `CloudRemainingPerformanceObligation` appears in the raw concept catalog even if it is not a canonical metric.
- Four contiguous quarterly revenue facts are summed into a `revenue_ttm` derived observation.
- A latest annual value is used only as a fallback and carries a quality flag stating that a TTM bridge was unavailable.
- An agent promotes a backlog concept into `supporting_metric_selections` for a scenario with rationale, without changing the canonical revenue mapping.

## Invalid Examples

- Dropping all SEC concepts that do not match the seed canonical list.
- Treating YTD facts as annual facts without period-shape metadata.
- Combining margin numerator and revenue denominator from different periods without a quality flag.
- Letting a model select canonical mappings without preserving selected-by, confidence, and rationale.
- Filtering catalog rows by plot readiness before analytical review.

## Required Tests

- Materializes raw catalog rows for canonical and non-canonical concepts.
- Seeds canonical metric definitions deterministically.
- Seeds known canonical mapping candidates without discarding exploratory concepts.
- Classifies instant, quarter, YTD, annual, and TTM shapes correctly.
- Builds TTM rows from four contiguous quarters.
- Avoids deriving margins from stale or mismatched periods.
- Preserves annual fallback quality flags.
- Supports deactivating and replacing an active mapping.
- Rejects or warns on ambiguous active canonical mappings.
- Stores supporting metric selections without changing canonical definitions.

