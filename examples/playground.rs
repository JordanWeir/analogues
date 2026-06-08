//! LLM concept-mapping review playground — agent investigates workspace SQLite,
//! validates mappings online, and returns structured review JSON.
//!
//! ```sh
//! cargo run --example playground
//! TICKER=ORCL cargo run --example playground
//! SQLITE=reports/stock-narrative-research/ORCL-2026-06-07-9/run.sqlite cargo run --example playground
//! SKIP_LLM=1 cargo run --example playground   # candidates + heuristic only
//! WEB_SEARCH=0 cargo run --example playground # disable online validation
//! ```
//!
//! Requires `OPENROUTER_API_KEY` unless `SKIP_LLM=1`.

use analogues::{
    services::{
        concept_catalog::{CanonicalMappingCandidate, ConceptCatalog},
        concept_review::{ConceptReviewOutput, ConceptReviewService, AGENT_REVIEW_PREAMBLE},
        model_client::OpenRouterModelClient,
        review_workspace::{cleanup_review_workspace, materialize_review_workspace},
    },
    workspace::{CanonicalMapping, SecRawFact},
};
use loco_rs::prelude::*;
use reqwest::Client;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use std::{
    collections::BTreeMap,
    env,
    path::{Path, PathBuf},
};

// =============================================================================
// Edit these while iterating on the prompt.
// =============================================================================

const DEFAULT_TICKER: &str = "ORCL";
const DEFAULT_MODEL: &str = "deepseek/deepseek-v4-flash";

/// Extra instructions appended after the auto-generated agent prompt.
const PROMPT_SUFFIX: &str = r#"
For balance-sheet debt metrics:
- debt_noncurrent must be an outstanding noncurrent borrowings balance, not a maturity schedule or repayment amount.
- Prefer concepts like Notes Payable Noncurrent or combined long-term borrowings over *Maturities* or *RepaymentsOfPrincipal*.
- If no direct noncurrent debt balance exists, return decision_type "calculated_from_components" or "unavailable".
"#;

const MAX_CANDIDATES_SHOWN: usize = 8;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    let _ctx = loco_rs::cli::playground::<analogues::app::App>().await?;

    let ticker = env::var("TICKER").unwrap_or_else(|_| DEFAULT_TICKER.to_string());
    let model = env::var("MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let skip_llm = env_bool("SKIP_LLM");
    let enable_web_search = env_bool_default("WEB_SEARCH", true);
    let fetched_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let (raw_facts, catalog_entries, workspace_sqlite, cleanup_workspace) =
        if let Ok(sqlite) = env::var("SQLITE") {
            let path = PathBuf::from(sqlite);
            println!("Loading sec_raw_facts from {}", path.display());
            let facts = load_raw_facts_from_sqlite(&path).await?;
            let entries = ConceptCatalog::materialize_catalog_entries(&facts);
            (facts, entries, path, false)
        } else {
            println!("Fetching SEC Company Facts for {ticker}");
            let facts = fetch_raw_facts(&ticker).await?;
            let entries = ConceptCatalog::materialize_catalog_entries(&facts);
            let workspace =
                materialize_review_workspace(&ticker, &facts, &entries, &fetched_at).await?;
            println!("Materialized review workspace at {}", workspace.display());
            (facts, entries, workspace, true)
        };

    println!(
        "Loaded {} raw facts across {} concepts",
        raw_facts.len(),
        unique_concepts(&raw_facts)
    );

    let candidates = ConceptCatalog::canonical_mapping_candidates(&catalog_entries);
    let heuristic = ConceptCatalog::seed_canonical_mappings(&raw_facts);
    let grouped = top_candidates_by_metric(&candidates, MAX_CANDIDATES_SHOWN);

    print_candidate_board(&grouped, &heuristic, &raw_facts);

    if skip_llm {
        println!("\nSKIP_LLM=1 set; not calling the model.");
        if cleanup_workspace {
            cleanup_review_workspace(&workspace_sqlite);
        }
        return Ok(());
    }

    let service = ConceptReviewService {
        model: model.clone(),
        enable_web_search,
        enable_workspace_sql: true,
        company_label: Some(ticker.clone()),
        workspace_sqlite: Some(workspace_sqlite.clone()),
        ..ConceptReviewService::default()
    };
    let client = OpenRouterModelClient;

    println!("\n=== LLM agent review ({model}) ===");
    println!("workspace_sql: enabled ({})", workspace_sqlite.display());
    if enable_web_search {
        println!("Web search: enabled (OpenRouter openrouter:web_search)");
    } else {
        println!("Web search: disabled");
    }
    println!("Preamble:\n{AGENT_REVIEW_PREAMBLE}\n");
    if !PROMPT_SUFFIX.trim().is_empty() {
        println!("Prompt suffix:\n{PROMPT_SUFFIX}\n");
    }

    let prompt = service.build_prompt()?;
    println!(
        "--- generated prompt ({} chars) ---\n{prompt}\n--- end prompt ---\n",
        prompt.len()
    );

    match service
        .review_workspace(&client, &raw_facts, AGENT_REVIEW_PREAMBLE, PROMPT_SUFFIX)
        .await
    {
        Ok((output, response)) => {
            println!("Model latency: {} ms", response.latency_ms);
            println!(
                "Agent rounds: {} | finish_reason: {:?} | workspace_sql calls: {} | web searches: {} | tokens: in={:?} out={:?}",
                response.agent_rounds,
                response.finish_reason,
                response.client_tool_calls,
                response.web_search_requests,
                response.input_tokens,
                response.output_tokens,
            );
            println!();
            print_review_results(&output, &service, &heuristic, &raw_facts);
        }
        Err(err) => {
            println!("LLM review failed: {err}");
            println!("\nHeuristic fallback that initWorkspace would use:");
            print_mapping_table(&heuristic, &raw_facts);
        }
    }

    if cleanup_workspace {
        cleanup_review_workspace(&workspace_sqlite);
    }

    Ok(())
}

async fn fetch_raw_facts(ticker: &str) -> Result<Vec<SecRawFact>> {
    let client = Client::builder()
        .user_agent("stock-agent-2/0.1 research@example.local")
        .build()
        .map_err(|err| Error::string(&format!("failed to build HTTP client: {err}")))?;
    let provider = analogues::services::sec_facts_provider::SecFactsProvider::new(client);
    let company = provider.lookup_company(ticker).await?;
    let payload = provider.fetch_company_facts(&company).await?;
    provider.extract_raw_facts(&payload)
}

async fn load_raw_facts_from_sqlite(path: &Path) -> Result<Vec<SecRawFact>> {
    let path = path
        .canonicalize()
        .map_err(|err| Error::string(&format!("invalid SQLITE path: {err}")))?;
    let url = format!(
        "sqlite://{}?mode=ro",
        path.to_string_lossy().replace('\\', "/")
    );
    let db = Database::connect(&url)
        .await
        .map_err(|err| Error::string(&format!("failed to open sqlite database: {err}")))?;

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT taxonomy, concept_name, label, description, unit, form, period_start, period_end,
                    filed_at, fiscal_year, fiscal_period, accession, frame, metric_value, raw_json, fetched_at
             FROM sec_raw_facts
             ORDER BY taxonomy, concept_name, period_end"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("failed to query sec_raw_facts: {err}")))?;

    rows.into_iter()
        .map(|row| {
            Ok(SecRawFact {
                taxonomy: row.try_get("", "taxonomy")?,
                concept_name: row.try_get("", "concept_name")?,
                label: row.try_get("", "label").ok(),
                description: row.try_get("", "description").ok(),
                unit: row.try_get("", "unit")?,
                form: row.try_get("", "form").ok(),
                start: row.try_get("", "period_start").ok(),
                end: row.try_get("", "period_end").ok(),
                filed: row.try_get("", "filed_at").ok(),
                fiscal_year: row.try_get("", "fiscal_year").ok(),
                fiscal_period: row.try_get("", "fiscal_period").ok(),
                accession: row.try_get("", "accession").ok(),
                frame: row.try_get("", "frame").ok(),
                value: row.try_get("", "metric_value")?,
                raw_json: row.try_get("", "raw_json")?,
                fetched_at: row.try_get("", "fetched_at")?,
            })
        })
        .collect()
}

fn unique_concepts(facts: &[SecRawFact]) -> usize {
    facts
        .iter()
        .map(|fact| (&fact.taxonomy, &fact.concept_name))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn top_candidates_by_metric<'a>(
    candidates: &'a [CanonicalMappingCandidate],
    limit: usize,
) -> BTreeMap<&'a str, Vec<&'a CanonicalMappingCandidate>> {
    let mut grouped: BTreeMap<&str, Vec<&CanonicalMappingCandidate>> = BTreeMap::new();
    for candidate in candidates {
        grouped
            .entry(candidate.mapping.canonical_key.as_str())
            .or_default()
            .push(candidate);
    }
    for list in grouped.values_mut() {
        list.sort_by(|left, right| {
            (right.score, right.fact_count).cmp(&(left.score, left.fact_count))
        });
        list.truncate(limit);
    }
    grouped
}

fn heuristic_for_metric<'a>(
    heuristic: &'a [CanonicalMapping],
    canonical_key: &str,
) -> Option<&'a CanonicalMapping> {
    heuristic
        .iter()
        .find(|mapping| mapping.canonical_key == canonical_key)
}

fn latest_fact_value(
    facts: &[SecRawFact],
    taxonomy: &str,
    concept_name: &str,
    unit: &str,
) -> Option<(String, f64)> {
    facts
        .iter()
        .filter(|fact| {
            fact.taxonomy == taxonomy && fact.concept_name == concept_name && fact.unit == unit
        })
        .max_by(|left, right| left.end.cmp(&right.end))
        .and_then(|fact| fact.end.clone().map(|period| (period, fact.value)))
}

fn format_value(value: f64, unit: &str) -> String {
    if unit.contains('/') || unit.contains("shares") {
        format!("{value:.4} {unit}")
    } else {
        format!("${:.3}B", value / 1_000_000_000.0)
    }
}

fn print_candidate_board(
    grouped: &BTreeMap<&str, Vec<&CanonicalMappingCandidate>>,
    heuristic: &[CanonicalMapping],
    raw_facts: &[SecRawFact],
) {
    println!("\n=== Candidate board (heuristic vs top gated candidates — reference only, not sent to agent) ===");
    for (canonical_key, candidates) in grouped {
        let label = candidates
            .first()
            .map(|candidate| candidate.mapping.metric_label.as_str())
            .unwrap_or(canonical_key);
        println!("\n[{canonical_key}] {label}");
        if let Some(mapping) = heuristic_for_metric(heuristic, canonical_key) {
            let latest = latest_fact_value(
                raw_facts,
                &mapping.taxonomy,
                &mapping.concept_name,
                &mapping.unit,
            );
            let latest_note = latest
                .map(|(period, value)| {
                    format!(" latest@{}={}", period, format_value(value, &mapping.unit))
                })
                .unwrap_or_default();
            println!(
                "  heuristic -> {} / {} (score via catalog, conf={}){latest_note}",
                mapping.taxonomy, mapping.concept_name, mapping.confidence
            );
            println!("    rationale: {}", mapping.rationale);
        } else {
            println!("  heuristic -> <none selected>");
        }

        for (index, candidate) in candidates.iter().enumerate() {
            let latest = latest_fact_value(
                raw_facts,
                &candidate.mapping.taxonomy,
                &candidate.mapping.concept_name,
                &candidate.mapping.unit,
            );
            let latest_note = latest
                .map(|(period, value)| {
                    format!(
                        " latest@{}={}",
                        period,
                        format_value(value, &candidate.mapping.unit)
                    )
                })
                .unwrap_or_default();
            println!(
                "  {:>2}. score={:<4} facts={:<4} conf={:<6} {} / {}{}",
                index + 1,
                candidate.score,
                candidate.fact_count,
                candidate.mapping.confidence,
                candidate.mapping.taxonomy,
                candidate.mapping.concept_name,
                latest_note
            );
            println!("      {}", candidate.mapping.rationale);
        }
    }
}

fn print_review_results(
    output: &ConceptReviewOutput,
    service: &ConceptReviewService,
    heuristic: &[CanonicalMapping],
    raw_facts: &[SecRawFact],
) {
    println!(
        "--- raw model JSON ---\n{}\n--- end raw JSON ---\n",
        serde_json::to_string_pretty(output).unwrap_or_else(|_| "{}".to_string())
    );

    let promoted = service.promote_reviewed_mappings(output, raw_facts);
    if !promoted.warnings.is_empty() {
        println!("Promotion warnings:");
        for warning in &promoted.warnings {
            println!("  - {warning}");
        }
        println!();
    }

    println!("=== Decision summary ===");
    for decision in &output.decisions {
        let latest = decision
            .taxonomy
            .as_deref()
            .zip(decision.concept_name.as_deref())
            .zip(decision.unit.as_deref())
            .and_then(|((taxonomy, concept), unit)| {
                latest_fact_value(raw_facts, taxonomy, concept, unit)
            });
        let unit = decision.unit.as_deref().unwrap_or("USD");
        let latest_note = latest
            .map(|(period, value)| format!(" latest@{}={}", period, format_value(value, unit)))
            .unwrap_or_default();
        println!(
            "\n[{}] decision={} conf={}{}",
            decision.canonical_key, decision.decision_type, decision.confidence, latest_note
        );
        if let Some(concept) = &decision.concept_name {
            println!(
                "  concept: {} / {}",
                decision.taxonomy.as_deref().unwrap_or("?"),
                concept
            );
        }
        println!("  rationale: {}", decision.rationale);
        if let Some(validation) = &decision.online_validation {
            println!(
                "  online_validation: {} — {}",
                validation.status, validation.summary
            );
            if let Some(value) = validation.db_latest_value {
                println!("    db_latest_value: {value}");
            }
            if let Some(period) = &validation.db_latest_period_end {
                println!("    db_latest_period_end: {period}");
            }
            if let Some(note) = &validation.online_value_note {
                println!("    online_value_note: {note}");
            }
            if !validation.search_queries.is_empty() {
                println!("    queries: {}", validation.search_queries.join(" | "));
            }
            if !validation.sources.is_empty() {
                println!("    sources:");
                for source in &validation.sources {
                    println!("      - {source}");
                }
            }
        }
        if !decision.warnings.is_empty() {
            println!("  warnings: {}", decision.warnings.join("; "));
        }
    }

    println!("\n=== Promoted mappings (what initWorkspace would persist on success) ===");
    if promoted.mappings.is_empty() {
        println!("<none — would fall back to heuristic seed_canonical_mappings>");
        print_mapping_table(heuristic, raw_facts);
    } else {
        print_mapping_table(&promoted.mappings, raw_facts);
        println!("\n=== Heuristic diff ===");
        for mapping in &promoted.mappings {
            let baseline = heuristic_for_metric(heuristic, &mapping.canonical_key);
            match baseline {
                Some(base) if base.concept_name == mapping.concept_name => {
                    println!(
                        "  {}: unchanged ({})",
                        mapping.canonical_key, mapping.concept_name
                    );
                }
                Some(base) => {
                    println!(
                        "  {}: {} -> {}",
                        mapping.canonical_key, base.concept_name, mapping.concept_name
                    );
                }
                None => println!(
                    "  {}: <none> -> {}",
                    mapping.canonical_key, mapping.concept_name
                ),
            }
        }
    }
}

fn print_mapping_table(mappings: &[CanonicalMapping], raw_facts: &[SecRawFact]) {
    for mapping in mappings {
        let latest = latest_fact_value(
            raw_facts,
            &mapping.taxonomy,
            &mapping.concept_name,
            &mapping.unit,
        );
        let latest_note = latest
            .map(|(period, value)| format!(" @{}={}", period, format_value(value, &mapping.unit)))
            .unwrap_or_default();
        println!(
            "  {} -> {} / {}{} [{}]",
            mapping.canonical_key,
            mapping.taxonomy,
            mapping.concept_name,
            latest_note,
            mapping.confidence
        );
    }
}

fn env_bool(key: &str) -> bool {
    env_bool_default(key, false)
}

fn env_bool_default(key: &str, default: bool) -> bool {
    match env::var(key).ok().as_deref() {
        Some("1" | "true" | "yes") => true,
        Some("0" | "false" | "no") => false,
        _ => default,
    }
}
