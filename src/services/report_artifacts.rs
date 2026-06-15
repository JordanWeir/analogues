//! Compile report payloads and render HTML artifacts from persisted workspace data.
//! Monte Carlo and valuation-band math live in `scenario_projection`; this module
//! is the view layer consumed by `scenario_artifacts` and `generateReport`.

use crate::services::{
    scenario_projection::{monte_carlo_is_persisted, scenario_data_with_monte_carlo},
    workspace_sql::{execute_sql, sql_quote},
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult, Statement};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

const REPORT_TEMPLATE_PATH: &str = ".agents/skills/stock-agent2/templates/report.html.j2";
const PROJECTION_NOTE: &str = "These scenario projections are illustrative and assumption-driven. They are not predictions, price targets, or investment advice. They show how different narrative outcomes could translate into financial assumptions and valuation ranges.";

#[derive(Debug, Clone)]
struct StockInfo {
    ticker: String,
    company_name: Option<String>,
    exchange: Option<String>,
    currency: Option<String>,
    sector: Option<String>,
    industry: Option<String>,
    source_note: Option<String>,
}

#[derive(Debug, Clone)]
struct FundamentalMetric {
    value: f64,
    period: Option<String>,
    source_note: Option<String>,
}

type Fundamentals = HashMap<String, FundamentalMetric>;

#[derive(Debug, Clone)]
struct FundamentalObservationRow {
    metric_key: String,
    metric_label: String,
    period_type: String,
    period_start: Option<String>,
    period_end: Option<String>,
    value: f64,
    unit: Option<String>,
    source_type: String,
    source_note: Option<String>,
    quality: Option<String>,
    is_derived: bool,
}

#[derive(Debug, Clone)]
struct DataGapRow {
    gap_key: String,
    description: String,
    status: String,
}

#[derive(Debug, Clone)]
struct DataQualityFlagRow {
    flag_key: String,
    severity: String,
    description: String,
    metric_key: Option<String>,
    period: Option<String>,
}

#[derive(Debug, Clone)]
struct RunMetadataRow {
    financial_fetch_status: String,
    financial_fetch_error: Option<String>,
}

#[derive(Debug, Clone)]
struct SourceRow {
    id: i64,
    title: String,
    url: Option<String>,
    source_type: Option<String>,
    published_at: Option<String>,
    why_it_matters: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Clone)]
struct ClaimRow {
    claim: String,
    source_id: Option<i64>,
    claim_type: Option<String>,
    side: Option<String>,
    confidence: Option<String>,
    metric: Option<String>,
}

#[derive(Debug, Clone)]
struct ScenarioInput {
    id: i64,
    name: String,
    stance: String,
    probability: Option<f64>,
    description: String,
    assumption_summary: Option<String>,
    crux_assumptions: Vec<Value>,
    sensitivities: Vec<String>,
    confirming_signals: Vec<String>,
    breaking_signals: Vec<String>,
    periods: Vec<ScenarioPeriodInput>,
}

#[derive(Debug, Clone)]
struct ScenarioPeriodInput {
    label: String,
    revenue: Option<f64>,
    revenue_growth: Option<f64>,
    diluted_shares: Option<f64>,
    gross_margin: Option<f64>,
    operating_margin: Option<f64>,
    net_margin: Option<f64>,
    net_income: Option<f64>,
    eps: Option<f64>,
    ps_low: Option<f64>,
    ps_median: Option<f64>,
    ps_high: Option<f64>,
    pe_low: Option<f64>,
    pe_median: Option<f64>,
    pe_high: Option<f64>,
    blend_ps_weight: f64,
    blend_pe_weight: f64,
}

/// Render `report.html`, persist the artifact row, and return the output path.
pub async fn render_and_persist_report(
    db: &sea_orm::DatabaseConnection,
    generated_dir: &Path,
    rendered_by: &str,
) -> Result<PathBuf> {
    fs::create_dir_all(generated_dir).map_err(|err| {
        Error::string(&format!(
            "failed to create generated directory {}: {err}",
            generated_dir.display()
        ))
    })?;

    let report = compile_report_payload(db).await?;
    let report_path = generated_dir.join("report.html");
    let html = render_report(&report)?;
    fs::write(&report_path, html).map_err(|err| {
        Error::string(&format!(
            "failed to write report HTML {}: {err}",
            report_path.display()
        ))
    })?;
    record_artifact(db, &report_path, rendered_by).await?;
    Ok(report_path)
}

/// Compile the full report JSON payload from workspace tables.
pub async fn compile_report_payload(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let stock = load_stock_info(db).await?;
    let fundamentals = load_fundamentals(db).await?;
    let observations = load_fundamental_observations(db).await?;
    let run_metadata = load_run_metadata(db).await?;
    let data_gaps = load_data_gaps(db).await?;
    let quality_flags = load_data_quality_flags(db).await?;
    let sources = load_sources(db).await?;
    let claims = load_claims(db).await?;
    let scenarios = load_scenarios(db).await?;
    let monte_carlo_persisted = monte_carlo_is_persisted(db).await?;

    validate_report_inputs(
        &fundamentals,
        &sources,
        &claims,
        &scenarios,
        monte_carlo_persisted,
    )?;

    let scenario_data = scenario_data_with_monte_carlo(db).await?;

    let generated_at = Utc::now().to_rfc3339();
    let report = json!({
        "company": company_json(&stock),
        "generated_at": generated_at,
        "projection_note": PROJECTION_NOTE,
        "source_pack": source_pack_json(&sources, &claims),
        "claim_table": claim_table_json(&sources, &claims),
        "financial_snapshot": financial_snapshot_json(&fundamentals),
        "historical_growth": historical_growth_json(&observations),
        "data_quality": data_quality_json(&run_metadata, &data_gaps, &quality_flags, &fundamentals, &observations),
        "sections": {
            "orientation": load_section_value(db, "orientation").await?,
            "business_model": load_section_value(db, "business_model").await?,
            "why_now": load_section_value(db, "why_now").await?,
            "narrative_map": load_narrative_map(db).await?,
            "financial_math": load_financial_math(db).await?,
            "historical_growth": historical_growth_json(&observations),
            "scenario_projection_summary": summarize_scenarios(&scenario_data),
            "industry_context": load_section_value(db, "industry_context").await?,
            "final_talk_track": load_section_value(db, "final_talk_track").await?,
        },
        "historical_analogues": historical_analogues_json(db).await?,
        "watch_items": watch_items_json(db).await?,
        "source_notes_and_limitations": source_notes_and_limitations(&stock, &fundamentals),
        "scenario_data": scenario_data,
    });

    Ok(report)
}

fn validate_report_inputs(
    fundamentals: &Fundamentals,
    sources: &[SourceRow],
    claims: &[ClaimRow],
    scenarios: &[ScenarioInput],
    monte_carlo_persisted: bool,
) -> Result<()> {
    let mut errors = Vec::new();

    for key in ["revenue_ttm", "shares_outstanding"] {
        if metric_value(fundamentals, key).is_none() {
            errors.push(format!("fundamentals needs numeric metric_key '{key}'"));
        }
    }
    if sources.is_empty() {
        errors.push("sources needs at least one row".to_string());
    }
    if claims.is_empty() {
        errors.push("claims needs at least one row".to_string());
    }
    if scenarios.is_empty() {
        errors.push("scenario_assumptions needs at least one scenario".to_string());
    }
    for scenario in scenarios {
        if scenario.periods.is_empty() {
            errors.push(format!(
                "scenario '{}' needs at least one scenario_periods row",
                scenario.name
            ));
        }
        for period in &scenario.periods {
            if period.revenue.is_none() && period.revenue_growth.is_none() {
                errors.push(format!(
                    "scenario '{}' period '{}' needs revenue or revenue_growth",
                    scenario.name, period.label
                ));
            }
        }
        if !monte_carlo_persisted {
            let Some(terminal) = scenario.periods.last() else {
                continue;
            };
            if terminal.ps_median.is_none() && terminal.pe_median.is_none() {
                errors.push(format!(
                    "scenario '{}' terminal period '{}' needs ps_median or pe_median",
                    scenario.name, terminal.label
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::string(&format!(
            "report cannot render yet:\n- {}",
            errors.join("\n- ")
        )))
    }
}

#[cfg(test)]
mod validate_tests {
    use super::*;

    fn scenario_with_periods(name: &str, ps_median: Option<f64>) -> ScenarioInput {
        ScenarioInput {
            id: 1,
            name: name.to_string(),
            stance: "neutral".to_string(),
            probability: Some(1.0),
            description: "test".to_string(),
            assumption_summary: None,
            crux_assumptions: Vec::new(),
            sensitivities: Vec::new(),
            confirming_signals: Vec::new(),
            breaking_signals: Vec::new(),
            periods: vec![
                ScenarioPeriodInput {
                    label: "Q1".to_string(),
                    revenue: None,
                    revenue_growth: Some(0.1),
                    diluted_shares: None,
                    gross_margin: None,
                    operating_margin: None,
                    net_margin: None,
                    net_income: None,
                    eps: None,
                    ps_low: None,
                    ps_median: None,
                    ps_high: None,
                    pe_low: None,
                    pe_median: None,
                    pe_high: None,
                    blend_ps_weight: 0.5,
                    blend_pe_weight: 0.5,
                },
                ScenarioPeriodInput {
                    label: "terminal".to_string(),
                    revenue: None,
                    revenue_growth: Some(0.08),
                    diluted_shares: None,
                    gross_margin: None,
                    operating_margin: None,
                    net_margin: None,
                    net_income: None,
                    eps: None,
                    ps_low: None,
                    ps_median,
                    ps_high: None,
                    pe_low: None,
                    pe_median: None,
                    pe_high: None,
                    blend_ps_weight: 0.5,
                    blend_pe_weight: 0.5,
                },
            ],
        }
    }

    fn minimal_fundamentals() -> Fundamentals {
        let mut map = Fundamentals::new();
        map.insert(
            "revenue_ttm".to_string(),
            FundamentalMetric {
                value: 1_000_000_000.0,
                period: None,
                source_note: None,
            },
        );
        map.insert(
            "shares_outstanding".to_string(),
            FundamentalMetric {
                value: 100_000_000.0,
                period: None,
                source_note: None,
            },
        );
        map
    }

    #[test]
    fn allows_intermediate_periods_without_ps_median_when_terminal_has_multiples() {
        let fundamentals = minimal_fundamentals();
        let sources = vec![SourceRow {
            id: 1,
            title: "filing".to_string(),
            url: None,
            source_type: None,
            published_at: None,
            why_it_matters: None,
            notes: None,
        }];
        let claims = vec![ClaimRow {
            claim: "growth".to_string(),
            source_id: Some(1),
            claim_type: None,
            side: None,
            confidence: None,
            metric: None,
        }];
        let scenarios = vec![scenario_with_periods("base", Some(9.0))];

        validate_report_inputs(&fundamentals, &sources, &claims, &scenarios, false)
            .expect("terminal multiples are sufficient");
    }

    #[test]
    fn rejects_missing_terminal_multiples_when_monte_carlo_not_persisted() {
        let fundamentals = minimal_fundamentals();
        let sources = vec![SourceRow {
            id: 1,
            title: "filing".to_string(),
            url: None,
            source_type: None,
            published_at: None,
            why_it_matters: None,
            notes: None,
        }];
        let claims = vec![ClaimRow {
            claim: "growth".to_string(),
            source_id: Some(1),
            claim_type: None,
            side: None,
            confidence: None,
            metric: None,
        }];
        let scenarios = vec![scenario_with_periods("base", None)];

        let err = validate_report_inputs(&fundamentals, &sources, &claims, &scenarios, false)
            .expect_err("missing terminal multiples");
        assert!(err.to_string().contains("terminal period"));
    }

    #[test]
    fn skips_terminal_multiple_checks_when_monte_carlo_persisted() {
        let fundamentals = minimal_fundamentals();
        let sources = vec![SourceRow {
            id: 1,
            title: "filing".to_string(),
            url: None,
            source_type: None,
            published_at: None,
            why_it_matters: None,
            notes: None,
        }];
        let claims = vec![ClaimRow {
            claim: "growth".to_string(),
            source_id: Some(1),
            claim_type: None,
            side: None,
            confidence: None,
            metric: None,
        }];
        let scenarios = vec![scenario_with_periods("base", None)];

        validate_report_inputs(&fundamentals, &sources, &claims, &scenarios, true)
            .expect("persisted monte carlo implies upstream validation passed");
    }
}


fn render_report(report: &Value) -> Result<String> {
    let template = fs::read_to_string(REPORT_TEMPLATE_PATH).map_err(|err| {
        Error::string(&format!(
            "failed to read report template {REPORT_TEMPLATE_PATH}: {err}"
        ))
    })?;
    let payload = serde_json::to_string_pretty(report)
        .map_err(|err| Error::string(&format!("failed to serialize report JSON: {err}")))?
        .replace("</", "<\\/");
    let title = report_title(report);
    Ok(template
        .replace("{{REPORT_JSON}}", &payload)
        .replace("{{REPORT_TITLE}}", &html_escape(&title))
        .replace(
            "{{GENERATED_AT}}",
            &html_escape(
                report
                    .get("generated_at")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
            ),
        ))
}

fn report_title(report: &Value) -> String {
    let ticker = report
        .pointer("/company/ticker")
        .and_then(Value::as_str)
        .unwrap_or("");
    let name = report
        .pointer("/company/name")
        .and_then(Value::as_str)
        .unwrap_or("");
    [ticker, name]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" / ")
}

async fn load_stock_info(db: &sea_orm::DatabaseConnection) -> Result<StockInfo> {
    let row = query_one_required(
        db,
        "SELECT ticker, company_name, exchange, currency, sector, industry, source_note
         FROM stock_info WHERE id = 1",
    )
    .await?;
    Ok(StockInfo {
        ticker: row_string(&row, 0)?,
        company_name: row_opt_string(&row, 1)?,
        exchange: row_opt_string(&row, 2)?,
        currency: row_opt_string(&row, 3)?,
        sector: row_opt_string(&row, 4)?,
        industry: row_opt_string(&row, 5)?,
        source_note: row_opt_string(&row, 6)?,
    })
}

async fn load_fundamentals(db: &sea_orm::DatabaseConnection) -> Result<Fundamentals> {
    let rows = query_all(
        db,
        "SELECT metric_key, metric_value, period, source_note
         FROM fundamentals
         ORDER BY metric_key, CASE WHEN period IS NULL THEN 0 ELSE 1 END, period, updated_at",
    )
    .await?;
    let mut fundamentals = HashMap::new();
    for row in rows {
        if let Some(value) = row_opt_f64(&row, 1)? {
            fundamentals.insert(
                row_string(&row, 0)?,
                FundamentalMetric {
                    value,
                    period: row_opt_string(&row, 2)?,
                    source_note: row_opt_string(&row, 3)?,
                },
            );
        }
    }
    Ok(fundamentals)
}

async fn load_fundamental_observations(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<FundamentalObservationRow>> {
    let rows = query_all(
        db,
        "SELECT metric_key, metric_label, period_type, period_start, period_end,
                metric_value, unit, source_type, source_note, quality, is_derived
         FROM canonical_fundamental_observations
         WHERE period_end IS NOT NULL
         ORDER BY metric_key, period_end, period_type, is_derived",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(FundamentalObservationRow {
                metric_key: row_string(&row, 0)?,
                metric_label: row_string(&row, 1)?,
                period_type: row_string(&row, 2)?,
                period_start: row_opt_string(&row, 3)?,
                period_end: row_opt_string(&row, 4)?,
                value: row_f64(&row, 5)?,
                unit: row_opt_string(&row, 6)?,
                source_type: row_string(&row, 7)?,
                source_note: row_opt_string(&row, 8)?,
                quality: row_opt_string(&row, 9)?,
                is_derived: row_i64(&row, 10)? != 0,
            })
        })
        .collect()
}

async fn load_run_metadata(db: &sea_orm::DatabaseConnection) -> Result<RunMetadataRow> {
    let row = query_one_required(
        db,
        "SELECT financial_fetch_status, financial_fetch_error FROM run_metadata WHERE id = 1",
    )
    .await?;
    Ok(RunMetadataRow {
        financial_fetch_status: row_string(&row, 0)?,
        financial_fetch_error: row_opt_string(&row, 1)?,
    })
}

async fn load_data_gaps(db: &sea_orm::DatabaseConnection) -> Result<Vec<DataGapRow>> {
    let rows = query_all(
        db,
        "SELECT gap_key, description, status FROM data_gaps ORDER BY id",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(DataGapRow {
                gap_key: row_string(&row, 0)?,
                description: row_string(&row, 1)?,
                status: row_string(&row, 2)?,
            })
        })
        .collect()
}

async fn load_data_quality_flags(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<DataQualityFlagRow>> {
    let rows = query_all(
        db,
        "SELECT flag_key, severity, description, metric_key, period
         FROM data_quality_flags ORDER BY id",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(DataQualityFlagRow {
                flag_key: row_string(&row, 0)?,
                severity: row_string(&row, 1)?,
                description: row_string(&row, 2)?,
                metric_key: row_opt_string(&row, 3)?,
                period: row_opt_string(&row, 4)?,
            })
        })
        .collect()
}

async fn load_sources(db: &sea_orm::DatabaseConnection) -> Result<Vec<SourceRow>> {
    let rows = query_all(
        db,
        "SELECT id, title, url, source_type, published_at, why_it_matters, notes
         FROM sources ORDER BY id",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(SourceRow {
                id: row_i64(&row, 0)?,
                title: row_string(&row, 1)?,
                url: row_opt_string(&row, 2)?,
                source_type: row_opt_string(&row, 3)?,
                published_at: row_opt_string(&row, 4)?,
                why_it_matters: row_opt_string(&row, 5)?,
                notes: row_opt_string(&row, 6)?,
            })
        })
        .collect()
}

async fn load_claims(db: &sea_orm::DatabaseConnection) -> Result<Vec<ClaimRow>> {
    let rows = query_all(
        db,
        "SELECT claim, source_id, claim_type, side, confidence, metric
         FROM claims ORDER BY id",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ClaimRow {
                claim: row_string(&row, 0)?,
                source_id: row_opt_i64(&row, 1)?,
                claim_type: row_opt_string(&row, 2)?,
                side: row_opt_string(&row, 3)?,
                confidence: row_opt_string(&row, 4)?,
                metric: row_opt_string(&row, 5)?,
            })
        })
        .collect()
}

async fn load_scenarios(db: &sea_orm::DatabaseConnection) -> Result<Vec<ScenarioInput>> {
    let rows = query_all(
        db,
        "SELECT id, name, stance, probability, description, assumption_summary
         FROM scenario_assumptions ORDER BY scenario_order",
    )
    .await?;
    let mut scenarios = Vec::new();
    for row in rows {
        let id = row_i64(&row, 0)?;
        scenarios.push(ScenarioInput {
            id,
            name: row_string(&row, 1)?,
            stance: row_string(&row, 2)?,
            probability: row_opt_f64(&row, 3)?,
            description: row_string(&row, 4)?,
            assumption_summary: row_opt_string(&row, 5)?,
            crux_assumptions: load_crux_assumptions(db, id).await?,
            sensitivities: load_strings(
                db,
                &format!(
                    "SELECT body FROM scenario_sensitivities WHERE scenario_id = {id} ORDER BY sensitivity_order"
                ),
            )
            .await?,
            confirming_signals: load_signals(db, id, "confirming").await?,
            breaking_signals: load_signals(db, id, "breaking").await?,
            periods: load_scenario_periods(db, id).await?,
        });
    }
    Ok(scenarios)
}

async fn load_crux_assumptions(
    db: &sea_orm::DatabaseConnection,
    scenario_id: i64,
) -> Result<Vec<Value>> {
    let rows = query_all(
        db,
        &format!(
            "SELECT crux, assumption, impact
             FROM scenario_crux_assumptions
             WHERE scenario_id = {scenario_id}
             ORDER BY crux_order"
        ),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "crux": row_string(&row, 0)?,
                "assumption": row_string(&row, 1)?,
                "impact": row_opt_string(&row, 2)?.unwrap_or_default(),
            }))
        })
        .collect()
}

async fn load_signals(
    db: &sea_orm::DatabaseConnection,
    scenario_id: i64,
    signal_type: &str,
) -> Result<Vec<String>> {
    load_strings(
        db,
        &format!(
            "SELECT body FROM scenario_signals
             WHERE scenario_id = {scenario_id} AND signal_type = '{}'
             ORDER BY signal_order",
            sql_quote(signal_type)
        ),
    )
    .await
}

async fn load_scenario_periods(
    db: &sea_orm::DatabaseConnection,
    scenario_id: i64,
) -> Result<Vec<ScenarioPeriodInput>> {
    let rows = query_all(
        db,
        &format!(
            "SELECT label, revenue, revenue_growth, diluted_shares, gross_margin,
                    operating_margin, net_margin, net_income, eps, ps_low, ps_median,
                    ps_high, pe_low, pe_median, pe_high, blend_ps_weight, blend_pe_weight
             FROM scenario_periods
             WHERE scenario_id = {scenario_id}
             ORDER BY period_order"
        ),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ScenarioPeriodInput {
                label: row_string(&row, 0)?,
                revenue: row_opt_f64(&row, 1)?,
                revenue_growth: row_opt_f64(&row, 2)?,
                diluted_shares: row_opt_f64(&row, 3)?,
                gross_margin: row_opt_f64(&row, 4)?,
                operating_margin: row_opt_f64(&row, 5)?,
                net_margin: row_opt_f64(&row, 6)?,
                net_income: row_opt_f64(&row, 7)?,
                eps: row_opt_f64(&row, 8)?,
                ps_low: row_opt_f64(&row, 9)?,
                ps_median: row_opt_f64(&row, 10)?,
                ps_high: row_opt_f64(&row, 11)?,
                pe_low: row_opt_f64(&row, 12)?,
                pe_median: row_opt_f64(&row, 13)?,
                pe_high: row_opt_f64(&row, 14)?,
                blend_ps_weight: row_opt_f64(&row, 15)?.unwrap_or(0.5),
                blend_pe_weight: row_opt_f64(&row, 16)?.unwrap_or(0.5),
            })
        })
        .collect()
}


async fn load_section_value(db: &sea_orm::DatabaseConnection, section_key: &str) -> Result<Value> {
    let row = query_one_required(
        db,
        &format!(
            "SELECT body FROM sections WHERE section_key = '{}'",
            sql_quote(section_key)
        ),
    )
    .await?;
    Ok(parse_body(row_opt_string(&row, 0)?))
}

async fn load_narrative_map(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let row = query_one_optional(
        db,
        "SELECT dominant, bull, bear, consensus, counter_narrative FROM narrative_map WHERE id = 1",
    )
    .await?;
    let mut map = Map::new();
    if let Some(row) = row {
        map.insert(
            "dominant".to_string(),
            json!(row_opt_string(&row, 0)?.unwrap_or_default()),
        );
        map.insert(
            "bull".to_string(),
            json!(row_opt_string(&row, 1)?.unwrap_or_default()),
        );
        map.insert(
            "bear".to_string(),
            json!(row_opt_string(&row, 2)?.unwrap_or_default()),
        );
        map.insert(
            "consensus".to_string(),
            json!(row_opt_string(&row, 3)?.unwrap_or_default()),
        );
        map.insert(
            "counter_narrative".to_string(),
            json!(row_opt_string(&row, 4)?.unwrap_or_default()),
        );
    } else {
        let fallback = load_section_value(db, "narrative_map").await?;
        if fallback.is_object() {
            return Ok(fallback);
        }
    }

    let item_rows = query_all(
        db,
        "SELECT item_type, body FROM narrative_map_items ORDER BY item_type, item_order",
    )
    .await?;
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in item_rows {
        grouped
            .entry(row_string(&row, 0)?)
            .or_default()
            .push(row_string(&row, 1)?);
    }
    map.insert(
        "agreements".to_string(),
        json!(grouped.remove("agreement").unwrap_or_default()),
    );
    map.insert(
        "cruxes".to_string(),
        json!(grouped.remove("crux").unwrap_or_default()),
    );
    Ok(Value::Object(map))
}

async fn load_financial_math(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let rows = query_all(
        db,
        "SELECT id, block_type, title, body, source_note, payload
         FROM content_blocks
         WHERE section_key = 'financial_math'
         ORDER BY block_order",
    )
    .await?;
    if rows.is_empty() {
        return load_section_value(db, "financial_math").await;
    }

    let mut blocks = Vec::new();
    for row in rows {
        let id = row_i64(&row, 0)?;
        let mut block = match row_opt_string(&row, 5)?
            .and_then(|payload| serde_json::from_str::<Value>(&payload).ok())
        {
            Some(Value::Object(object)) => object,
            _ => Map::new(),
        };
        block.insert("type".to_string(), json!(row_string(&row, 1)?));
        insert_if_some(&mut block, "title", row_opt_string(&row, 2)?);
        insert_if_some(&mut block, "body", row_opt_string(&row, 3)?);
        insert_if_some(&mut block, "source_note", row_opt_string(&row, 4)?);

        let metrics = load_content_block_metrics(db, id).await?;
        if !metrics.is_empty() {
            block.insert("metrics".to_string(), Value::Array(metrics));
        }
        let items = load_content_block_items(db, id).await?;
        if !items.is_empty() {
            block.insert("items".to_string(), json!(items));
        }
        blocks.push(Value::Object(block));
    }
    Ok(Value::Array(blocks))
}

async fn load_content_block_metrics(
    db: &sea_orm::DatabaseConnection,
    content_block_id: i64,
) -> Result<Vec<Value>> {
    let rows = query_all(
        db,
        &format!(
            "SELECT label, value, note
             FROM content_block_metrics
             WHERE content_block_id = {content_block_id}
             ORDER BY metric_order"
        ),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "label": row_string(&row, 0)?,
                "value": row_opt_string(&row, 1)?.unwrap_or_default(),
                "note": row_opt_string(&row, 2)?.unwrap_or_default(),
            }))
        })
        .collect()
}

async fn load_content_block_items(
    db: &sea_orm::DatabaseConnection,
    content_block_id: i64,
) -> Result<Vec<String>> {
    load_strings(
        db,
        &format!(
            "SELECT body FROM content_block_items
             WHERE content_block_id = {content_block_id}
             ORDER BY item_order"
        ),
    )
    .await
}

async fn historical_analogues_json(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let rows = query_all(
        db,
        "SELECT analogue, setup, lesson, why_it_can_mislead
         FROM historical_analogues ORDER BY analogue_order",
    )
    .await?;
    let analogues: Result<Vec<Value>> = rows
        .into_iter()
        .map(|row| {
            Ok(json!({
                "analogue": row_string(&row, 0)?,
                "narrative_type": "",
                "why_similar": row_opt_string(&row, 1)?.unwrap_or_default(),
                "how_it_played_out": "",
                "financial_pattern": "",
                "key_pivots": [],
                "lesson": row_opt_string(&row, 2)?.unwrap_or_default(),
                "why_misleading": row_opt_string(&row, 3)?.unwrap_or_default(),
                "source_notes": "",
            }))
        })
        .collect();
    Ok(Value::Array(analogues?))
}

async fn watch_items_json(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let rows = query_all(
        db,
        "SELECT title, description, signal_type FROM watch_items ORDER BY item_order",
    )
    .await?;
    let items: Result<Vec<Value>> = rows
        .into_iter()
        .map(|row| {
            Ok(json!({
                "signal": row_string(&row, 0)?,
                "why_it_matters": row_opt_string(&row, 1)?.unwrap_or_default(),
                "scenario_affected": row_opt_string(&row, 2)?.unwrap_or_default(),
                "bull_signal": "",
                "bear_signal": "",
            }))
        })
        .collect();
    Ok(Value::Array(items?))
}

fn company_json(stock: &StockInfo) -> Value {
    json!({
        "ticker": stock.ticker.clone(),
        "name": stock.company_name.clone().unwrap_or_default(),
        "exchange": stock.exchange.clone().unwrap_or_default(),
        "currency": stock.currency.clone().unwrap_or_else(|| "USD".to_string()),
        "sector": stock.sector.clone().unwrap_or_default(),
        "industry": stock.industry.clone().unwrap_or_default(),
    })
}

fn source_pack_json(sources: &[SourceRow], claims: &[ClaimRow]) -> Value {
    let rows = sources
        .iter()
        .map(|source| {
            let claims_supported: Vec<String> = claims
                .iter()
                .filter(|claim| claim.source_id == Some(source.id))
                .map(|claim| claim.claim.clone())
                .collect();
            json!({
                "source": source.title.clone(),
                "url": source.url.clone().unwrap_or_default(),
                "type": source.source_type.clone().unwrap_or_else(|| "Other".to_string()),
                "date": source.published_at.clone().unwrap_or_default(),
                "why_it_matters": source.why_it_matters.clone().or_else(|| source.notes.clone()).unwrap_or_default(),
                "claims_supported": claims_supported,
            })
        })
        .collect();
    Value::Array(rows)
}

fn claim_table_json(sources: &[SourceRow], claims: &[ClaimRow]) -> Value {
    let sources_by_id: HashMap<i64, &SourceRow> =
        sources.iter().map(|source| (source.id, source)).collect();
    let rows = claims
        .iter()
        .map(|claim| {
            let source = claim
                .source_id
                .and_then(|id| sources_by_id.get(&id).copied());
            json!({
                "claim": claim.claim.clone(),
                "source": source.map(|source| source.title.clone()).unwrap_or_default(),
                "date": source.and_then(|source| source.published_at.clone()).unwrap_or_default(),
                "type": claim.claim_type.clone().unwrap_or_else(|| "credibility".to_string()),
                "side": claim.side.clone().unwrap_or_else(|| "neutral".to_string()),
                "confidence": claim.confidence.clone().unwrap_or_else(|| "Medium".to_string()),
                "related_metric": claim.metric.clone().unwrap_or_default(),
            })
        })
        .collect();
    Value::Array(rows)
}

fn financial_snapshot_json(fundamentals: &Fundamentals) -> Value {
    let source_note = fundamentals
        .values()
        .filter_map(|metric| metric.source_note.clone())
        .collect::<Vec<_>>()
        .join("; ");
    json!({
        "current_share_price": format_optional_money(metric_value(fundamentals, "current_price")),
        "market_cap": format_optional_money(metric_value(fundamentals, "market_cap")),
        "ttm_revenue": format_optional_money(metric_value(fundamentals, "revenue_ttm")),
        "ttm_eps": format_optional_money(metric_value(fundamentals, "eps_ttm")),
        "gross_margin": format_optional_percent(metric_value(fundamentals, "gross_margin")),
        "operating_margin": format_optional_percent(metric_value(fundamentals, "operating_margin")),
        "net_margin": format_optional_percent(metric_value(fundamentals, "net_margin")),
        "cash": format_optional_money(metric_value(fundamentals, "cash")),
        "total_debt": format_optional_money(metric_value(fundamentals, "total_debt")),
        "trailing_pe": format_optional_multiple(metric_value(fundamentals, "trailing_pe")),
        "price_to_sales_ttm": format_optional_multiple(metric_value(fundamentals, "price_to_sales_ttm")),
        "source_note": source_note,
    })
}

fn historical_growth_json(observations: &[FundamentalObservationRow]) -> Value {
    if observations.is_empty() {
        return Value::Null;
    }
    let tracked_metrics = [
        "revenue_quarter",
        "revenue_ttm",
        "gross_margin",
        "operating_margin",
        "net_margin",
        "eps_quarter",
        "eps_ttm",
        "diluted_shares_quarter",
    ];
    let mut series = Map::new();
    for metric_key in tracked_metrics {
        let mut rows = observations
            .iter()
            .filter(|observation| observation.metric_key == metric_key)
            .filter(|observation| {
                matches!(
                    observation.period_type.as_str(),
                    "annual" | "quarter" | "ttm" | "instant"
                )
            })
            .map(|observation| {
                json!({
                    "label": observation.metric_label.clone(),
                    "period_type": observation.period_type.clone(),
                    "period_start": observation.period_start.clone(),
                    "period_end": observation.period_end.clone(),
                    "value": observation.value,
                    "unit": observation.unit.clone(),
                    "source_type": observation.source_type.clone(),
                    "source_note": observation.source_note.clone(),
                    "quality": observation.quality.clone(),
                    "is_derived": observation.is_derived,
                })
            })
            .collect::<Vec<_>>();
        if rows.len() > 12 {
            rows = rows.split_off(rows.len() - 12);
        }
        if !rows.is_empty() {
            series.insert(metric_key.to_string(), Value::Array(rows));
        }
    }
    if series.is_empty() {
        Value::Null
    } else {
        json!({
            "summary": "Historical growth is sourced from the normalized SEC/Yahoo observation timeline. Derived margins and TTM rows are period-aligned before inclusion.",
            "series": series,
        })
    }
}

fn data_quality_json(
    run_metadata: &RunMetadataRow,
    gaps: &[DataGapRow],
    flags: &[DataQualityFlagRow],
    fundamentals: &Fundamentals,
    observations: &[FundamentalObservationRow],
) -> Value {
    let required_metrics = [
        "current_price",
        "market_cap",
        "shares_outstanding",
        "revenue_ttm",
        "net_margin",
        "eps_ttm",
    ];
    let mut metric_coverage = Map::new();
    for metric_key in required_metrics {
        metric_coverage.insert(
            metric_key.to_string(),
            Value::Bool(metric_value(fundamentals, metric_key).is_some()),
        );
    }
    json!({
        "financial_fetch_status": run_metadata.financial_fetch_status.clone(),
        "financial_fetch_error": run_metadata.financial_fetch_error.clone(),
        "gaps": gaps.iter().map(|gap| json!({
            "gap_key": gap.gap_key.clone(),
            "description": gap.description.clone(),
            "status": gap.status.clone(),
        })).collect::<Vec<_>>(),
        "flags": flags.iter().map(|flag| json!({
            "flag_key": flag.flag_key.clone(),
            "severity": flag.severity.clone(),
            "description": flag.description.clone(),
            "metric_key": flag.metric_key.clone(),
            "period": flag.period.clone(),
        })).collect::<Vec<_>>(),
        "metric_coverage": metric_coverage,
        "observation_count": observations.len(),
    })
}

fn source_notes_and_limitations(stock: &StockInfo, fundamentals: &Fundamentals) -> Value {
    let mut notes = Vec::new();
    if let Some(note) = &stock.source_note {
        notes.push(note.clone());
    }
    for metric in fundamentals.values() {
        if let Some(note) = &metric.source_note {
            if !notes.contains(note) {
                notes.push(note.clone());
            }
        }
    }
    json!({
        "source_notes": notes,
        "projection_limitations": vec![PROJECTION_NOTE.to_string()],
    })
}

fn summarize_scenarios(scenario_data: &Value) -> Value {
    let mut summary = Map::new();
    for scenario in scenario_data
        .get("scenarios")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = scenario.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(periods) = scenario.get("periods").and_then(Value::as_array) else {
            continue;
        };
        let Some(terminal) = periods.last() else {
            continue;
        };
        let band = terminal
            .get("blended_price")
            .or_else(|| terminal.get("ps_implied_price"))
            .or_else(|| terminal.get("pe_implied_price"));
        let Some(band) = band else {
            continue;
        };
        summary.insert(
            slug_key(name),
            json!(format!(
                "{} illustrative band: {} / {} / {}.",
                terminal
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("terminal"),
                format_money(band.get("low").and_then(Value::as_f64)),
                format_money(band.get("median").and_then(Value::as_f64)),
                format_money(band.get("high").and_then(Value::as_f64)),
            )),
        );
    }
    Value::Object(summary)
}

async fn record_artifact(
    db: &sea_orm::DatabaseConnection,
    report_path: &Path,
    rendered_by: &str,
) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "INSERT INTO artifacts (artifact_type, path, created_at, notes)
             VALUES ('report_html', '{}', '{}', '{}')",
            sql_quote(&report_path.display().to_string()),
            sql_quote(&Utc::now().to_rfc3339()),
            sql_quote(&format!("Rendered by {rendered_by}")),
        ),
    )
    .await
}

async fn load_strings(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<Vec<String>> {
    let rows = query_all(db, sql).await?;
    rows.into_iter().map(|row| row_string(&row, 0)).collect()
}

async fn query_all(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<Vec<QueryResult>> {
    db.query_all(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("failed to execute SQL query: {err}\n{sql}")))
}

async fn query_one_required(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<QueryResult> {
    query_one_optional(db, sql)
        .await?
        .ok_or_else(|| Error::string(&format!("required SQL query returned no rows: {sql}")))
}

async fn query_one_optional(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
) -> Result<Option<QueryResult>> {
    db.query_one(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .map_err(|err| Error::string(&format!("failed to execute SQL query: {err}\n{sql}")))
}

fn row_string(row: &QueryResult, index: usize) -> Result<String> {
    row.try_get_by_index::<String>(index)
        .map_err(|err| Error::string(&format!("failed to read string column {index}: {err}")))
}

fn row_opt_string(row: &QueryResult, index: usize) -> Result<Option<String>> {
    row.try_get_by_index::<Option<String>>(index)
        .map_err(|err| {
            Error::string(&format!(
                "failed to read optional string column {index}: {err}"
            ))
        })
}

fn row_i64(row: &QueryResult, index: usize) -> Result<i64> {
    row.try_get_by_index::<i64>(index)
        .map_err(|err| Error::string(&format!("failed to read integer column {index}: {err}")))
}

fn row_opt_i64(row: &QueryResult, index: usize) -> Result<Option<i64>> {
    row.try_get_by_index::<Option<i64>>(index).map_err(|err| {
        Error::string(&format!(
            "failed to read optional integer column {index}: {err}"
        ))
    })
}

fn row_opt_f64(row: &QueryResult, index: usize) -> Result<Option<f64>> {
    row.try_get_by_index::<Option<f64>>(index).map_err(|err| {
        Error::string(&format!(
            "failed to read optional number column {index}: {err}"
        ))
    })
}

fn row_f64(row: &QueryResult, index: usize) -> Result<f64> {
    row.try_get_by_index::<f64>(index)
        .map_err(|err| Error::string(&format!("failed to read number column {index}: {err}")))
}


fn parse_body(body: Option<String>) -> Value {
    match body {
        Some(body) if !body.trim().is_empty() => {
            serde_json::from_str(&body).unwrap_or_else(|_| Value::String(body))
        }
        _ => Value::Null,
    }
}

fn insert_if_some(map: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        map.insert(key.to_string(), Value::String(value));
    }
}

fn metric_value(fundamentals: &Fundamentals, key: &str) -> Option<f64> {
    fundamentals.get(key).map(|metric| metric.value)
}

fn required_metric(fundamentals: &Fundamentals, key: &str) -> Result<f64> {
    metric_value(fundamentals, key)
        .ok_or_else(|| Error::string(&format!("fundamentals needs numeric metric_key '{key}'")))
}


fn round_optional(value: Option<f64>) -> Option<f64> {
    value.map(round_float)
}

fn round_float(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn format_optional_money(value: Option<f64>) -> Option<String> {
    value.map(|value| {
        if value.abs() >= 1_000_000_000.0 {
            format!("${:.1}B", value / 1_000_000_000.0)
        } else if value.abs() >= 1_000_000.0 {
            format!("${:.1}M", value / 1_000_000.0)
        } else {
            format!("${value:.2}")
        }
    })
}

fn format_optional_percent(value: Option<f64>) -> Option<String> {
    value.map(|value| format!("{:.1}%", value * 100.0))
}

fn format_optional_multiple(value: Option<f64>) -> Option<String> {
    value.map(|value| format!("{value:.1}x"))
}

fn format_money(value: Option<f64>) -> String {
    value
        .map(|value| format!("${value:.0}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn slug_key(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
