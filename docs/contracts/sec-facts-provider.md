# Contract Spec: SecFactsProvider

## Overview

`SecFactsProvider` owns SEC ticker lookup and SEC Company Facts retrieval behind a stable internal interface.

The seam separates provider behavior from domain interpretation. It should preserve raw provider custody and enough metadata for downstream cataloging, quality checks, refresh invalidation, and source limitations. It should not decide which SEC concepts are canonical or analytically important.

The first implementation may live beside the current Yahoo/SEC ingestion code, but the SEC-specific contract should be independent from market quote fetching.

## Public Types / Traits

- `SecFactsProvider`: service or trait responsible for SEC ticker lookup and Company Facts fetch.
- `SecTickerLookupRequest`: ticker and optional cache policy.
- `SecCompanyFactsRequest`: ticker or CIK, fetch timestamp, and optional cache policy.
- `SecCompanyIdentity`: ticker, CIK, company title, and lookup source metadata.
- `SecCompanyFactsPayload`: company identity, fetched-at timestamp, source URL, raw JSON payload.
- `SecRawFact`: taxonomy, concept name, label, description, unit, form, period start, period end, filed date, fiscal year, fiscal period, accession, frame, value, raw JSON, fetched-at timestamp.
- `SecFetchError`: lookup failure, HTTP failure, rate limit, invalid JSON, missing facts root, unsupported value shape, and provider unavailable.

## Operations

- `lookup_company(ticker) -> SecCompanyIdentity`
- `fetch_company_facts(identity_or_ticker) -> SecCompanyFactsPayload`
- `extract_raw_facts(payload) -> Vec<SecRawFact>`
- `source_url(identity) -> Url`
- `provider_name() -> "SEC Company Facts"`

## Behavioral Rules

- SEC requests use a configured SEC-compliant user agent.
- Ticker lookup is case-insensitive.
- Company Facts URLs use zero-padded CIK values.
- Raw JSON is preserved for each extracted fact row when practical.
- Facts are extracted across all taxonomies, concepts, and units.
- The provider only extracts numeric `val` facts into `SecRawFact`.
- Missing optional SEC fields remain nullable; missing required fields produce extraction skips or structured errors depending on severity.
- Fetch timestamps are generated once per payload and propagated to extracted facts.
- Provider output keeps provider vocabulary intact: taxonomy, concept name, unit, form, fiscal period, accession, and frame are not rewritten.
- Provider code does not seed canonical metric mappings, TTM rows, margins, gaps, or report sections.
- Rate limits and transient provider failures are surfaced to callers so ingestion can record data gaps.

## Error Semantics

- Unknown ticker returns a lookup-not-found error with the ticker.
- Non-success HTTP status returns an HTTP error with URL and status.
- Network timeout returns a transport error with URL.
- Invalid JSON returns a parse error with URL.
- Missing top-level `facts` returns a payload-shape error.
- Individual malformed fact rows may be skipped if the enclosing payload is valid and enough required metadata is absent only for that row.
- A fully failed fetch should not write partial raw facts unless the caller explicitly asked for partial persistence.
- The provider does not hide SEC failures behind empty vectors.

## Valid Examples

- Looking up `orcl` returns the SEC identity for `ORCL` with its CIK.
- Fetching Company Facts for a valid CIK returns raw payload plus source URL and fetched-at timestamp.
- A company-specific concept such as backlog or remaining performance obligation is preserved in raw facts even if no canonical mapping uses it.
- A fact with `form`, `start`, `end`, `filed`, `fy`, `fp`, `accn`, `frame`, and numeric `val` becomes one `SecRawFact`.

## Invalid Examples

- Filtering raw facts to only revenue, net income, EPS, and share count inside the provider.
- Renaming `RevenueFromContractWithCustomerExcludingAssessedTax` to `revenue` in provider output.
- Treating an unavailable SEC response as a successful empty payload.
- Making one SEC request per concept after the Company Facts payload is already available.
- Embedding canonical metric confidence or scenario relevance in `SecRawFact`.

## Required Tests

- Builds zero-padded Company Facts URLs from CIK values.
- Looks up tickers case-insensitively.
- Extracts facts across multiple taxonomies, concepts, and units.
- Preserves company-specific raw concepts that are not canonical metrics.
- Preserves accession, form, fiscal period, period dates, frame, and raw JSON.
- Skips or rejects malformed fact rows according to documented severity.
- Returns structured errors for unknown ticker, HTTP status failure, invalid JSON, and missing `facts`.
- Uses the configured SEC user agent.
- Does not perform canonical metric mapping or period-shape classification.

