//! Deterministic scenario roll-forward, valuation bands, and Monte Carlo persistence.
//! Used by `scenario_generation` lane; consumed by `report_artifacts` / `scenario_artifacts`.

use crate::services::{
    scenario_projection_calendar::load_calendar,
    workspace_financial_store::WorkspaceFinancialStore,
    workspace_sql::{execute_sql, scalar_i64, sql_number, sql_quote, sql_value},
};
use crate::workspace::DailyPriceBar;
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult, Statement};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

const P10_P90_Z_SCORE: f64 = 1.281_551_565_544_600_4;
const PROJECTION_NOTE: &str = "Scenario projections are illustrative and assumption-driven. They are not predictions, price targets, or investment advice.";
const TTM_QUARTERS: usize = 4;
const VALUATION_MULTIPLE_BASIS: &str = "ttm";
const DEFAULT_BLEND_WEIGHT: f64 = 0.5;

#[derive(Debug, Clone)]
struct FundamentalMetric {
    value: f64,
    period: Option<String>,
    source_note: Option<String>,
}

type Fundamentals = HashMap<String, FundamentalMetric>;

#[derive(Debug, Clone, Default)]
struct PeriodMeta {
    period_order: i64,
    is_historical: bool,
}

#[derive(Debug, Clone, Default)]
struct QuarterActuals {
    revenue: Option<f64>,
    net_income: Option<f64>,
    gross_profit: Option<f64>,
    diluted_shares: Option<f64>,
    market_price: Option<Band>,
}

#[derive(Debug, Clone, Default)]
struct ProjectionContext {
    historical_quarters: usize,
    forward_quarters: usize,
    historical_anchor_end: Option<String>,
    terminal_period_end: Option<String>,
    period_meta: HashMap<String, PeriodMeta>,
    quarter_actuals: HashMap<String, QuarterActuals>,
}

#[derive(Debug, Clone)]
pub struct ScenarioPeriodRow {
    pub period_order: i64,
    pub label: String,
    pub period_end: Option<String>,
    pub period_type: Option<String>,
    pub revenue: Option<f64>,
    pub revenue_growth: Option<f64>,
    pub diluted_shares: Option<f64>,
    pub gross_margin: Option<f64>,
    pub operating_margin: Option<f64>,
    pub net_margin: Option<f64>,
    pub net_income: Option<f64>,
    pub eps: Option<f64>,
    pub ps_low: Option<f64>,
    pub ps_median: Option<f64>,
    pub ps_high: Option<f64>,
    pub pe_low: Option<f64>,
    pub pe_median: Option<f64>,
    pub pe_high: Option<f64>,
    pub blend_ps_weight: f64,
    pub blend_pe_weight: f64,
    pub source_note: Option<String>,
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
    periods: Vec<ScenarioPeriodRow>,
}

#[derive(Debug, Clone)]
struct ScenarioOutput {
    id: i64,
    name: String,
    probability: Option<f64>,
    terminal_band: Option<Band>,
}

#[derive(Debug, Clone, Copy)]
struct Band {
    low: f64,
    median: f64,
    high: f64,
}

#[derive(Debug, Clone, Copy)]
struct BlendWeights {
    ps: f64,
    pe: f64,
}

#[derive(Debug, Clone)]
struct MonteCarloConfig {
    iterations: usize,
    seed: u64,
    bins: usize,
}

#[derive(Debug, Clone)]
struct MonteCarloResult {
    iterations: usize,
    seed: u64,
    bins: usize,
    price_field: Option<String>,
    probability_basis: Option<String>,
    normal_distribution_basis: Option<String>,
    methodology: Option<String>,
    summary: BTreeMap<String, f64>,
    histogram: Vec<HistogramBin>,
    scenario_probabilities: Vec<ScenarioProbability>,
}

#[derive(Debug, Clone)]
struct HistogramBin {
    low: f64,
    high: f64,
    midpoint: f64,
    count: usize,
    probability: f64,
}

#[derive(Debug, Clone)]
struct ScenarioProbability {
    scenario_id: i64,
    name: String,
    input_probability: Option<f64>,
    normalized_probability: f64,
    sample_count: usize,
    observed_probability: f64,
}

#[derive(Debug, Clone)]
struct SamplingSpec {
    scenario_id: i64,
    name: String,
    input_probability: Option<f64>,
    raw_probability: f64,
    normalized_probability: f64,
    band: Band,
}

#[derive(Debug, Clone)]
struct StockInfo {
    ticker: String,
    company_name: Option<String>,
    currency: Option<String>,
}

/// Whether Monte Carlo summary rows exist for this workspace.
pub async fn monte_carlo_is_persisted(db: &sea_orm::DatabaseConnection) -> Result<bool> {
    Ok(scalar_i64(db, "SELECT COUNT(*) AS count FROM monte_carlo_summary").await? > 0)
}

/// Build scenario roll-forward JSON without sampling or persisting Monte Carlo.
pub async fn build_scenario_data_json(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let stock = load_stock_info(db).await?;
    let mut fundamentals = load_fundamentals(db).await?;
    enhance_baseline_from_av(db, &mut fundamentals).await?;
    enhance_spot_market_metrics(db, &mut fundamentals).await?;
    let scenarios = load_scenarios(db).await?;
    if scenarios.is_empty() {
        return Err(Error::string("no scenario_assumptions rows to project"));
    }
    for scenario in &scenarios {
        if scenario.periods.is_empty() {
            return Err(Error::string(&format!(
                "scenario '{}' has no scenario_periods rows",
                scenario.name
            )));
        }
    }
    let config = load_monte_carlo_config(db).await?;
    let projection_context = load_projection_context(db).await?;
    build_scenario_data(
        &stock,
        &fundamentals,
        &scenarios,
        &config,
        &projection_context,
    )
}

/// Load persisted Monte Carlo outputs as report-ready JSON.
pub async fn load_persisted_monte_carlo_json(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let summary = query_one_required(
        db,
        "SELECT iterations, seed, bins, price_field, probability_basis,
                normal_distribution_basis, methodology,
                summary_min, summary_p10, summary_p25, summary_median, summary_mean,
                summary_p75, summary_p90, summary_max, summary_stdev
         FROM monte_carlo_summary WHERE id = 1",
    )
    .await?;

    let mut summary_map = BTreeMap::new();
    for (index, key) in [
        "min", "p10", "p25", "median", "mean", "p75", "p90", "max", "stdev",
    ]
    .iter()
    .enumerate()
    {
        if let Some(value) = row_opt_f64(&summary, 7 + index)? {
            summary_map.insert((*key).to_string(), value);
        }
    }

    let histogram_rows = query_all(
        db,
        "SELECT low, high, midpoint, count, probability
         FROM monte_carlo_histogram_bins ORDER BY bin_order",
    )
    .await?;
    let histogram = histogram_rows
        .iter()
        .map(|row| {
            Ok(json!({
                "low": row_f64(row, 0)?,
                "high": row_f64(row, 1)?,
                "midpoint": row_f64(row, 2)?,
                "count": row_i64(row, 3)?,
                "probability": row_f64(row, 4)?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    let probability_rows = query_all(
        db,
        "SELECT p.scenario_id, s.name, p.input_probability, p.normalized_probability,
                p.sample_count, p.observed_probability
         FROM monte_carlo_scenario_probabilities p
         JOIN scenario_assumptions s ON s.id = p.scenario_id
         ORDER BY s.scenario_order",
    )
    .await?;
    let scenario_probabilities = probability_rows
        .iter()
        .map(|row| {
            Ok(json!({
                "name": row_string(row, 1)?,
                "input_probability": row_opt_f64(row, 2)?,
                "normalized_probability": row_f64(row, 3)?,
                "sample_count": row_i64(row, 4)?,
                "observed_probability": row_f64(row, 5)?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(json!({
        "iterations": row_i64(&summary, 0)? as usize,
        "seed": row_i64(&summary, 1)? as u64,
        "bins": row_i64(&summary, 2)? as usize,
        "price_field": row_opt_string(&summary, 3)?,
        "probability_basis": row_opt_string(&summary, 4)?,
        "normal_distribution_basis": row_opt_string(&summary, 5)?,
        "methodology": row_opt_string(&summary, 6)?,
        "summary": summary_map,
        "histogram": histogram,
        "scenario_probabilities": scenario_probabilities,
    }))
}

/// Scenario projection data with Monte Carlo, reading persisted outputs when present.
pub async fn scenario_data_with_monte_carlo(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    if monte_carlo_is_persisted(db).await? {
        let mut scenario_data = build_scenario_data_json(db).await?;
        let monte_carlo = load_persisted_monte_carlo_json(db).await?;
        if let Some(obj) = scenario_data.as_object_mut() {
            obj.insert("monte_carlo".to_string(), monte_carlo);
        }
        Ok(scenario_data)
    } else {
        compute_and_persist_monte_carlo(db).await
    }
}

/// Compute scenario valuation bands and persist Monte Carlo outputs.
pub async fn compute_and_persist_monte_carlo(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let scenario_data = build_scenario_data_json(db).await?;
    let scenarios = load_scenarios(db).await?;
    let config = load_monte_carlo_config(db).await?;
    let scenario_outputs = scenario_outputs_from_value(&scenario_data, &scenarios);
    let monte_carlo = build_monte_carlo(&config, &scenario_outputs);
    persist_monte_carlo(db, &monte_carlo).await?;

    let mut scenario_data_map = scenario_data.as_object().cloned().unwrap_or_default();
    scenario_data_map.insert("monte_carlo".to_string(), monte_carlo_to_json(&monte_carlo));
    Ok(Value::Object(scenario_data_map))
}

async fn enhance_baseline_from_av(
    db: &sea_orm::DatabaseConnection,
    fundamentals: &mut Fundamentals,
) -> Result<()> {
    let ttm = av_trailing_revenue_ttm(db).await?;
    if let Some(revenue) = ttm {
        fundamentals.insert(
            "revenue_ttm".to_string(),
            FundamentalMetric {
                value: revenue,
                period: fundamentals
                    .get("revenue_ttm")
                    .and_then(|m| m.period.clone()),
                source_note: Some(
                    "AlphaVantage sum of last 4 quarterly totalRevenue (preferred baseline)"
                        .to_string(),
                ),
            },
        );
    }
    if let Some(shares) = av_latest_diluted_shares(db).await? {
        fundamentals.insert(
            "shares_outstanding".to_string(),
            FundamentalMetric {
                value: shares,
                period: fundamentals
                    .get("shares_outstanding")
                    .and_then(|m| m.period.clone()),
                source_note: Some("AlphaVantage latest quarterly weightedAverageShsOutDil".to_string()),
            },
        );
    }
    Ok(())
}

async fn enhance_spot_market_metrics(
    db: &sea_orm::DatabaseConnection,
    fundamentals: &mut Fundamentals,
) -> Result<()> {
    use crate::services::workspace_financial_store::resolve_spot_market_snapshot;

    let snapshot = resolve_spot_market_snapshot(
        db,
        fundamentals.get("current_price").map(|metric| metric.value),
        fundamentals.get("market_cap").map(|metric| metric.value),
        fundamentals
            .get("shares_outstanding")
            .map(|metric| metric.value),
    )
    .await?;

    if !fundamentals.contains_key("current_price") {
        if let Some(current_price) = snapshot.current_price {
            fundamentals.insert(
                "current_price".to_string(),
                FundamentalMetric {
                    value: current_price,
                    period: snapshot.current_price_period,
                    source_note: snapshot.current_price_source_note,
                },
            );
        }
    }

    if !fundamentals.contains_key("market_cap") {
        if let Some(market_cap) = snapshot.market_cap {
            fundamentals.insert(
                "market_cap".to_string(),
                FundamentalMetric {
                    value: market_cap,
                    period: None,
                    source_note: snapshot.market_cap_source_note,
                },
            );
        }
    }

    Ok(())
}

async fn av_trailing_revenue_ttm(db: &sea_orm::DatabaseConnection) -> Result<Option<f64>> {
    let rows = query_all(
        db,
        "SELECT metric_value FROM av_raw_facts
         WHERE report_type = 'quarterly' AND period_type = 'quarter'
           AND field_name = 'totalRevenue'
         ORDER BY period_end DESC
         LIMIT 4",
    )
    .await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let sum: f64 = rows
        .iter()
        .filter_map(|row| row_opt_f64(row, 0).ok().flatten())
        .sum();
    if sum > 0.0 {
        Ok(Some(sum))
    } else {
        Ok(None)
    }
}

async fn av_latest_diluted_shares(db: &sea_orm::DatabaseConnection) -> Result<Option<f64>> {
    let row = query_one_optional(
        db,
        "SELECT metric_value FROM av_raw_facts
         WHERE report_type = 'quarterly' AND period_type = 'quarter'
           AND field_name = 'weightedAverageShsOutDil'
         ORDER BY period_end DESC
         LIMIT 1",
    )
    .await?;
    Ok(row.and_then(|row| row_opt_f64(&row, 0).ok().flatten()))
}

fn build_scenario_data(
    stock: &StockInfo,
    fundamentals: &Fundamentals,
    scenarios: &[ScenarioInput],
    config: &MonteCarloConfig,
    projection_context: &ProjectionContext,
) -> Result<Value> {
    let baseline_revenue = required_metric(fundamentals, "revenue_ttm")?;
    let baseline_shares = required_metric(fundamentals, "shares_outstanding")?;
    let baseline_margin = metric_value(fundamentals, "net_margin");
    let baseline_eps = metric_value(fundamentals, "eps_ttm");
    let base_year = fundamentals
        .get("revenue_ttm")
        .and_then(|metric| metric.period.clone())
        .unwrap_or_else(|| "Current".to_string());

    let mut scenario_values = Vec::new();
    for scenario in scenarios {
        scenario_values.push(build_scenario_json(
            scenario,
            baseline_revenue,
            baseline_shares,
            baseline_margin,
            baseline_eps,
            projection_context,
        )?);
    }

    Ok(json!({
        "company": stock.company_name.clone().unwrap_or_default(),
        "ticker": stock.ticker.clone(),
        "currency": stock.currency.clone().unwrap_or_else(|| "USD".to_string()),
        "generated_at": Utc::now().to_rfc3339(),
        "projection_note": PROJECTION_NOTE,
        "valuation_multiple_basis": VALUATION_MULTIPLE_BASIS,
        "base_year": base_year,
        "current_price": metric_value(fundamentals, "current_price"),
        "projection_calendar": projection_calendar_json(projection_context),
        "historical_periods": build_historical_periods_json(scenarios, projection_context),
        "baseline": {
            "revenue": baseline_revenue,
            "diluted_shares": baseline_shares,
            "net_margin": baseline_margin,
            "eps": baseline_eps,
            "period": base_year,
            "source_note": fundamentals.get("revenue_ttm").and_then(|m| m.source_note.clone()),
        },
        "scenarios": scenario_values,
        "monte_carlo": {
            "iterations": config.iterations,
            "seed": config.seed,
            "bins": config.bins,
        },
    }))
}

#[derive(Debug, Clone, Copy)]
struct QuarterWindowEntry {
    revenue: f64,
    net_income: f64,
}

fn ttm_valuation_inputs(
    window: &[QuarterWindowEntry],
    diluted_shares: f64,
) -> Option<(f64, f64, f64)> {
    if window.len() < TTM_QUARTERS || diluted_shares <= 0.0 {
        return None;
    }
    let revenue_ttm: f64 = window.iter().map(|entry| entry.revenue).sum();
    let net_income_ttm: f64 = window.iter().map(|entry| entry.net_income).sum();
    Some((
        revenue_ttm,
        revenue_ttm / diluted_shares,
        net_income_ttm / diluted_shares,
    ))
}

fn build_scenario_json(
    scenario: &ScenarioInput,
    baseline_revenue: f64,
    baseline_shares: f64,
    baseline_margin: Option<f64>,
    baseline_eps: Option<f64>,
    projection_context: &ProjectionContext,
) -> Result<Value> {
    let mut periods = Vec::new();
    let mut previous_revenue = baseline_revenue;
    let mut previous_shares = baseline_shares;
    let mut previous_margin = baseline_margin;
    let mut previous_eps = baseline_eps;
    let mut quarter_window: Vec<QuarterWindowEntry> = Vec::with_capacity(TTM_QUARTERS);

    for period in &scenario.periods {
        let period_end = period.period_end.clone().unwrap_or_default();
        let meta = projection_context
            .period_meta
            .get(&period_end)
            .cloned()
            .unwrap_or(PeriodMeta {
                period_order: period.period_order,
                is_historical: false,
            });
        let is_historical = meta.is_historical;
        let period_kind = if is_historical {
            "historical"
        } else {
            "projected"
        };
        let actuals = projection_context.quarter_actuals.get(&period_end);

        let mut revenue = period.revenue;
        let mut diluted_shares = period.diluted_shares;
        let mut net_income = period.net_income;
        let mut gross_margin = period.gross_margin;
        let mut net_margin = period.net_margin;
        let mut eps = period.eps;

        if is_historical {
            if let Some(actuals) = actuals {
                revenue = revenue.or(actuals.revenue);
                net_income = net_income.or(actuals.net_income);
                diluted_shares = diluted_shares.or(actuals.diluted_shares);
                if gross_margin.is_none() {
                    gross_margin = ratio(actuals.gross_profit, actuals.revenue);
                }
                if net_margin.is_none() {
                    net_margin = ratio(actuals.net_income, actuals.revenue);
                }
                if eps.is_none() {
                    eps = ratio(actuals.net_income, actuals.diluted_shares);
                }
            }
        }

        let revenue = match (revenue, period.revenue_growth) {
            (Some(revenue), _) => revenue,
            (None, Some(growth)) => previous_revenue * (1.0 + growth),
            (None, None) => {
                return Err(Error::string(&format!(
                    "scenario '{}' period '{}' needs revenue or revenue_growth",
                    scenario.name, period.label
                )));
            }
        };
        let revenue_growth = period
            .revenue_growth
            .or_else(|| growth_rate(Some(revenue), Some(previous_revenue)));
        let diluted_shares = diluted_shares.unwrap_or(previous_shares);
        let net_margin = net_margin.or(previous_margin);
        let net_income = net_income.or_else(|| net_margin.map(|margin| revenue * margin));
        let eps = eps
            .or_else(|| net_income.map(|income| income / diluted_shares))
            .or(previous_eps);
        let ps_multiple = band_from_parts(period.ps_low, period.ps_median, period.ps_high);
        let pe_multiple = band_from_parts(period.pe_low, period.pe_median, period.pe_high);
        let revenue_per_share = revenue / diluted_shares;

        if let Some(net_income) = net_income {
            quarter_window.push(QuarterWindowEntry {
                revenue,
                net_income,
            });
            if quarter_window.len() > TTM_QUARTERS {
                quarter_window.remove(0);
            }
        }

        let (revenue_ttm, revenue_per_share_ttm, eps_ttm) =
            match ttm_valuation_inputs(&quarter_window, diluted_shares) {
                Some((revenue_ttm, revenue_per_share_ttm, eps_ttm)) => {
                    (Some(revenue_ttm), Some(revenue_per_share_ttm), Some(eps_ttm))
                }
                None => (None, None, None),
            };
        let (blend_weights, blend_rationale) = resolve_blend_weights(
            period.blend_ps_weight,
            period.blend_pe_weight,
            eps_ttm,
            ps_multiple,
            pe_multiple,
        )?;
        let ps_implied_price = apply_multiple(revenue_per_share_ttm, ps_multiple);
        let pe_implied_price = apply_multiple(pe_eps_usable(eps_ttm), pe_multiple);
        let blended_price = blend_bands(ps_implied_price, pe_implied_price, blend_weights);
        let multiple_basis = if (ps_multiple.is_some() || pe_multiple.is_some())
            && (ps_implied_price.is_some() || pe_implied_price.is_some())
        {
            Some(VALUATION_MULTIPLE_BASIS)
        } else {
            None
        };
        let market_price = if is_historical {
            actuals.and_then(|actuals| actuals.market_price)
        } else {
            None
        };
        let chart_price = if is_historical {
            market_price.or(blended_price)
        } else {
            blended_price
        };

        periods.push(json!({
            "period_order": meta.period_order,
            "label": period.label,
            "period_end": period.period_end,
            "period_type": period.period_type,
            "is_historical": is_historical,
            "period_kind": period_kind,
            "revenue_growth": round_optional(revenue_growth),
            "revenue": round_float(revenue),
            "diluted_shares": round_float(diluted_shares),
            "revenue_per_share": round_float(revenue_per_share),
            "revenue_ttm": round_optional(revenue_ttm),
            "revenue_per_share_ttm": round_optional(revenue_per_share_ttm),
            "gross_margin": gross_margin,
            "operating_margin": period.operating_margin,
            "net_margin": net_margin,
            "net_income": round_optional(net_income),
            "eps": round_optional(eps),
            "eps_ttm": round_optional(eps_ttm),
            "multiple_basis": multiple_basis,
            "ps_multiple": ps_multiple.map(|band| band.to_json()),
            "pe_multiple": pe_multiple.map(|band| band.to_json()),
            "blend_weights": json!({ "ps": blend_weights.ps, "pe": blend_weights.pe }),
            "blend_rationale": blend_rationale,
            "market_price": market_price.map(|band| band.to_json()),
            "ps_implied_price": ps_implied_price.map(|band| band.to_json()),
            "pe_implied_price": pe_implied_price.map(|band| band.to_json()),
            "blended_price": blended_price.map(|band| band.to_json()),
            "chart_price": chart_price.map(|band| band.to_json()),
            "source_note": period.source_note.clone().or_else(|| {
                if is_historical {
                    Some("Alpha Vantage quarterly actual".to_string())
                } else {
                    None
                }
            }),
        }));

        previous_revenue = revenue;
        previous_shares = diluted_shares;
        previous_margin = net_margin;
        previous_eps = eps;
    }

    Ok(json!({
        "name": scenario.name,
        "stance": scenario.stance,
        "probability": scenario.probability,
        "description": scenario.description,
        "assumption_summary": scenario.assumption_summary.clone().unwrap_or_default(),
        "crux_assumptions": scenario.crux_assumptions,
        "sensitivities": scenario.sensitivities,
        "confirming_signals": scenario.confirming_signals,
        "breaking_signals": scenario.breaking_signals,
        "periods": periods,
    }))
}

fn scenario_outputs_from_value(
    scenario_data: &Value,
    scenario_inputs: &[ScenarioInput],
) -> Vec<ScenarioOutput> {
    scenario_data
        .get("scenarios")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, scenario)| {
            let periods = scenario.get("periods").and_then(Value::as_array)?;
            let terminal = periods.last()?;
            let terminal_band = terminal
                .get("blended_price")
                .or_else(|| terminal.get("ps_implied_price"))
                .or_else(|| terminal.get("pe_implied_price"))
                .and_then(Band::from_json);
            Some(ScenarioOutput {
                id: scenario_inputs
                    .get(index)
                    .map(|s| s.id)
                    .unwrap_or((index + 1) as i64),
                name: scenario
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Unnamed scenario")
                    .to_string(),
                probability: scenario.get("probability").and_then(Value::as_f64),
                terminal_band,
            })
        })
        .collect()
}

fn build_monte_carlo(config: &MonteCarloConfig, scenarios: &[ScenarioOutput]) -> MonteCarloResult {
    let mut specs = sampling_specs(scenarios);
    if specs.is_empty() {
        return MonteCarloResult {
            iterations: config.iterations,
            seed: config.seed,
            bins: config.bins,
            price_field: None,
            probability_basis: None,
            normal_distribution_basis: None,
            methodology: Some(
                "No terminal price bands were available for Monte Carlo sampling.".to_string(),
            ),
            summary: BTreeMap::new(),
            histogram: Vec::new(),
            scenario_probabilities: Vec::new(),
        };
    }

    let mut cumulative = Vec::new();
    let mut running = 0.0;
    for spec in &specs {
        running += spec.normalized_probability;
        cumulative.push((running, spec.clone()));
    }

    let mut rng = DeterministicRng::new(config.seed);
    let mut samples = Vec::with_capacity(config.iterations);
    let mut counts: HashMap<i64, usize> = specs.iter().map(|spec| (spec.scenario_id, 0)).collect();

    for _ in 0..config.iterations {
        let pick = rng.next_f64();
        let selected = cumulative
            .iter()
            .find(|(boundary, _)| pick <= *boundary)
            .map(|(_, spec)| spec)
            .unwrap_or_else(|| &cumulative[cumulative.len() - 1].1);
        let price = sample_from_price_band(&mut rng, selected.band);
        samples.push(price);
        *counts.entry(selected.scenario_id).or_insert(0) += 1;
    }

    let scenario_probabilities = specs
        .drain(..)
        .map(|spec| {
            let sample_count = *counts.get(&spec.scenario_id).unwrap_or(&0);
            ScenarioProbability {
                scenario_id: spec.scenario_id,
                name: spec.name,
                input_probability: spec.input_probability,
                normalized_probability: round_float(spec.normalized_probability),
                sample_count,
                observed_probability: round_float(sample_count as f64 / config.iterations as f64),
            }
        })
        .collect();

    MonteCarloResult {
        iterations: config.iterations,
        seed: config.seed,
        bins: config.bins,
        price_field: Some(
            "terminal blended price from TTM P/S and P/E multiples, falling back to P/S or P/E implied price"
                .to_string(),
        ),
        probability_basis: Some(
            "Scenario probabilities normalized across scenarios with terminal bands.".to_string(),
        ),
        normal_distribution_basis: Some(
            "Low/median/high terminal bands treated as P10/P50/P90 normal, floored at zero."
                .to_string(),
        ),
        methodology: None,
        summary: distribution_summary(&samples),
        histogram: histogram(&samples, config.bins),
        scenario_probabilities,
    }
}

fn sampling_specs(scenarios: &[ScenarioOutput]) -> Vec<SamplingSpec> {
    let mut specs: Vec<SamplingSpec> = scenarios
        .iter()
        .filter_map(|scenario| {
            let band = scenario.terminal_band?;
            Some(SamplingSpec {
                scenario_id: scenario.id,
                name: scenario.name.clone(),
                input_probability: scenario.probability,
                raw_probability: scenario.probability.filter(|v| *v > 0.0).unwrap_or(0.0),
                normalized_probability: 0.0,
                band,
            })
        })
        .collect();

    if specs.is_empty() {
        return specs;
    }
    let total: f64 = specs.iter().map(|s| s.raw_probability).sum();
    if total <= 0.0 {
        let equal = 1.0 / specs.len() as f64;
        for spec in &mut specs {
            spec.normalized_probability = equal;
        }
    } else {
        for spec in &mut specs {
            spec.normalized_probability = spec.raw_probability / total;
        }
    }
    specs
}

async fn persist_monte_carlo(
    db: &sea_orm::DatabaseConnection,
    monte_carlo: &MonteCarloResult,
) -> Result<()> {
    execute_sql(db, "DELETE FROM monte_carlo_histogram_bins").await?;
    execute_sql(db, "DELETE FROM monte_carlo_scenario_probabilities").await?;
    execute_sql(db, "DELETE FROM monte_carlo_summary").await?;

    execute_sql(
        db,
        &format!(
            "INSERT INTO monte_carlo_summary (
                id, iterations, seed, bins, price_field, probability_basis,
                normal_distribution_basis, methodology, summary_min, summary_p10, summary_p25,
                summary_median, summary_mean, summary_p75, summary_p90, summary_max,
                summary_stdev, generated_at
            ) VALUES (
                1, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, '{}'
            )",
            monte_carlo.iterations,
            monte_carlo.seed,
            monte_carlo.bins,
            sql_value(monte_carlo.price_field.as_deref()),
            sql_value(monte_carlo.probability_basis.as_deref()),
            sql_value(monte_carlo.normal_distribution_basis.as_deref()),
            sql_value(monte_carlo.methodology.as_deref()),
            sql_number(monte_carlo.summary.get("min").copied()),
            sql_number(monte_carlo.summary.get("p10").copied()),
            sql_number(monte_carlo.summary.get("p25").copied()),
            sql_number(monte_carlo.summary.get("median").copied()),
            sql_number(monte_carlo.summary.get("mean").copied()),
            sql_number(monte_carlo.summary.get("p75").copied()),
            sql_number(monte_carlo.summary.get("p90").copied()),
            sql_number(monte_carlo.summary.get("max").copied()),
            sql_number(monte_carlo.summary.get("stdev").copied()),
            sql_quote(&Utc::now().to_rfc3339()),
        ),
    )
    .await?;

    for (index, bin) in monte_carlo.histogram.iter().enumerate() {
        execute_sql(
            db,
            &format!(
                "INSERT INTO monte_carlo_histogram_bins (
                    bin_order, low, high, midpoint, count, probability
                ) VALUES ({}, {}, {}, {}, {}, {})",
                index + 1,
                bin.low,
                bin.high,
                bin.midpoint,
                bin.count,
                bin.probability,
            ),
        )
        .await?;
    }

    for probability in &monte_carlo.scenario_probabilities {
        execute_sql(
            db,
            &format!(
                "INSERT INTO monte_carlo_scenario_probabilities (
                    scenario_id, input_probability, normalized_probability,
                    sample_count, observed_probability
                ) VALUES ({}, {}, {}, {}, {})",
                probability.scenario_id,
                sql_number(probability.input_probability),
                probability.normalized_probability,
                probability.sample_count,
                probability.observed_probability,
            ),
        )
        .await?;
    }

    Ok(())
}

fn monte_carlo_to_json(monte_carlo: &MonteCarloResult) -> Value {
    json!({
        "iterations": monte_carlo.iterations,
        "seed": monte_carlo.seed,
        "bins": monte_carlo.bins,
        "summary": monte_carlo.summary,
        "histogram": monte_carlo.histogram.iter().map(|bin| json!({
            "low": bin.low, "high": bin.high, "midpoint": bin.midpoint,
            "count": bin.count, "probability": bin.probability,
        })).collect::<Vec<_>>(),
        "scenario_probabilities": monte_carlo.scenario_probabilities.iter().map(|p| json!({
            "name": p.name,
            "input_probability": p.input_probability,
            "normalized_probability": p.normalized_probability,
            "sample_count": p.sample_count,
            "observed_probability": p.observed_probability,
        })).collect::<Vec<_>>(),
    })
}

async fn load_stock_info(db: &sea_orm::DatabaseConnection) -> Result<StockInfo> {
    let row = query_one_required(
        db,
        "SELECT ticker, company_name, currency FROM stock_info WHERE id = 1",
    )
    .await?;
    Ok(StockInfo {
        ticker: row_string(&row, 0)?,
        company_name: row_opt_string(&row, 1)?,
        currency: row_opt_string(&row, 2)?,
    })
}

async fn load_fundamentals(db: &sea_orm::DatabaseConnection) -> Result<Fundamentals> {
    let rows = query_all(
        db,
        "SELECT metric_key, metric_value, period, source_note FROM fundamentals
         ORDER BY metric_key, CASE WHEN period IS NULL THEN 0 ELSE 1 END, period, updated_at",
    )
    .await?;
    let mut map = HashMap::new();
    for row in rows {
        let key = row_string(&row, 0)?;
        if map.contains_key(&key) {
            continue;
        }
        if let Some(value) = row_opt_f64(&row, 1)? {
            map.insert(
                key,
                FundamentalMetric {
                    value,
                    period: row_opt_string(&row, 2)?,
                    source_note: row_opt_string(&row, 3)?,
                },
            );
        }
    }
    Ok(map)
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

async fn load_crux_assumptions(db: &sea_orm::DatabaseConnection, scenario_id: i64) -> Result<Vec<Value>> {
    let rows = query_all(
        db,
        &format!(
            "SELECT crux_key, crux, assumption, impact, experiment_key
             FROM scenario_crux_assumptions WHERE scenario_id = {scenario_id} ORDER BY crux_order"
        ),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(json!({
                "crux_key": row_opt_string(&row, 0)?.unwrap_or_default(),
                "crux": row_string(&row, 1)?,
                "assumption": row_string(&row, 2)?,
                "impact": row_opt_string(&row, 3)?.unwrap_or_default(),
                "experiment_key": row_opt_string(&row, 4)?.unwrap_or_default(),
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
             WHERE scenario_id = {scenario_id} AND signal_type = '{signal_type}'
             ORDER BY signal_order"
        ),
    )
    .await
}

async fn load_scenario_periods(
    db: &sea_orm::DatabaseConnection,
    scenario_id: i64,
) -> Result<Vec<ScenarioPeriodRow>> {
    let rows = query_all(
        db,
        &format!(
            "SELECT period_order, label, period_end, period_type, revenue, revenue_growth, diluted_shares,
                    gross_margin, operating_margin, net_margin, net_income, eps,
                    ps_low, ps_median, ps_high, pe_low, pe_median, pe_high,
                    blend_ps_weight, blend_pe_weight, source_note
             FROM scenario_periods WHERE scenario_id = {scenario_id} ORDER BY period_order"
        ),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ScenarioPeriodRow {
                period_order: row_i64(&row, 0)?,
                label: row_string(&row, 1)?,
                period_end: row_opt_string(&row, 2)?,
                period_type: row_opt_string(&row, 3)?,
                revenue: row_opt_f64(&row, 4)?,
                revenue_growth: row_opt_f64(&row, 5)?,
                diluted_shares: row_opt_f64(&row, 6)?,
                gross_margin: row_opt_f64(&row, 7)?,
                operating_margin: row_opt_f64(&row, 8)?,
                net_margin: row_opt_f64(&row, 9)?,
                net_income: row_opt_f64(&row, 10)?,
                eps: row_opt_f64(&row, 11)?,
                ps_low: row_opt_f64(&row, 12)?,
                ps_median: row_opt_f64(&row, 13)?,
                ps_high: row_opt_f64(&row, 14)?,
                pe_low: row_opt_f64(&row, 15)?,
                pe_median: row_opt_f64(&row, 16)?,
                pe_high: row_opt_f64(&row, 17)?,
                blend_ps_weight: row_opt_f64(&row, 18)?.unwrap_or(0.5),
                blend_pe_weight: row_opt_f64(&row, 19)?.unwrap_or(0.5),
                source_note: row_opt_string(&row, 20)?,
            })
        })
        .collect()
}

async fn load_monte_carlo_config(db: &sea_orm::DatabaseConnection) -> Result<MonteCarloConfig> {
    let row = query_one_required(
        db,
        "SELECT iterations, seed, bins FROM monte_carlo_config WHERE id = 1",
    )
    .await?;
    Ok(MonteCarloConfig {
        iterations: row_i64(&row, 0)?.max(1) as usize,
        seed: row_i64(&row, 1)?.max(0) as u64,
        bins: row_i64(&row, 2)?.max(1) as usize,
    })
}

async fn load_strings(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<Vec<String>> {
    let rows = query_all(db, sql).await?;
    rows.into_iter().map(|row| row_string(&row, 0)).collect()
}

async fn query_all(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<Vec<QueryResult>> {
    db.query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}\n{sql}")))
}

async fn query_one_required(db: &sea_orm::DatabaseConnection, sql: &str) -> Result<QueryResult> {
    query_one_optional(db, sql)
        .await?
        .ok_or_else(|| Error::string(&format!("required query returned no rows: {sql}")))
}

async fn query_one_optional(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
) -> Result<Option<QueryResult>> {
    db.query_one(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}\n{sql}")))
}

fn row_string(row: &QueryResult, index: usize) -> Result<String> {
    row.try_get_by_index::<String>(index)
        .map_err(|err| Error::string(&format!("read string col {index}: {err}")))
}

fn row_opt_string(row: &QueryResult, index: usize) -> Result<Option<String>> {
    row.try_get_by_index::<Option<String>>(index)
        .map_err(|err| Error::string(&format!("read opt string col {index}: {err}")))
}

fn row_i64(row: &QueryResult, index: usize) -> Result<i64> {
    row.try_get_by_index::<i64>(index)
        .map_err(|err| Error::string(&format!("read i64 col {index}: {err}")))
}

fn row_f64(row: &QueryResult, index: usize) -> Result<f64> {
    row.try_get_by_index::<f64>(index)
        .map_err(|err| Error::string(&format!("read f64 col {index}: {err}")))
}

fn row_opt_f64(row: &QueryResult, index: usize) -> Result<Option<f64>> {
    row.try_get_by_index::<Option<f64>>(index)
        .map_err(|err| Error::string(&format!("read opt f64 col {index}: {err}")))
}

fn metric_value(fundamentals: &Fundamentals, key: &str) -> Option<f64> {
    fundamentals.get(key).map(|m| m.value)
}

fn required_metric(fundamentals: &Fundamentals, key: &str) -> Result<f64> {
    metric_value(fundamentals, key)
        .ok_or_else(|| Error::string(&format!("fundamentals needs numeric metric_key '{key}'")))
}

async fn load_projection_context(db: &sea_orm::DatabaseConnection) -> Result<ProjectionContext> {
    let Some(calendar) = load_calendar(db).await? else {
        return Ok(ProjectionContext::default());
    };

    let period_meta = calendar
        .periods
        .iter()
        .map(|period| {
            (
                period.period_end.clone(),
                PeriodMeta {
                    period_order: period.period_order,
                    is_historical: period.is_historical,
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let historical_ends = calendar
        .periods
        .iter()
        .filter(|period| period.is_historical)
        .map(|period| period.period_end.clone())
        .collect::<Vec<_>>();
    let quarter_actuals = load_quarter_actuals(db, &historical_ends).await?;

    Ok(ProjectionContext {
        historical_quarters: calendar.historical_quarters,
        forward_quarters: calendar.forward_quarters,
        historical_anchor_end: Some(calendar.historical_anchor_end.clone()),
        terminal_period_end: Some(calendar.terminal_period_end.clone()),
        period_meta,
        quarter_actuals,
    })
}

async fn load_quarter_actuals(
    db: &sea_orm::DatabaseConnection,
    period_ends: &[String],
) -> Result<HashMap<String, QuarterActuals>> {
    if period_ends.is_empty() {
        return Ok(HashMap::new());
    }

    let quoted_ends = period_ends
        .iter()
        .map(|period_end| sql_quote(period_end))
        .collect::<Vec<_>>()
        .join(", ");
    let rows = query_all(
        db,
        &format!(
            "SELECT period_end, field_name, metric_value
             FROM av_raw_facts
             WHERE report_type = 'quarterly'
               AND period_type = 'quarter'
               AND period_end IN ({quoted_ends})
               AND field_name IN (
                 'totalRevenue', 'netIncome', 'grossProfit',
                 'weightedAverageShsOutDil', 'commonStockSharesOutstanding'
               )"
        ),
    )
    .await?;

    let mut actuals = period_ends
        .iter()
        .map(|period_end| (period_end.clone(), QuarterActuals::default()))
        .collect::<HashMap<_, _>>();

    for row in rows {
        let period_end = row_string(&row, 0)?;
        let field_name = row_string(&row, 1)?;
        let value = row_opt_f64(&row, 2)?;
        let Some(entry) = actuals.get_mut(&period_end) else {
            continue;
        };
        match field_name.as_str() {
            "totalRevenue" => entry.revenue = value,
            "netIncome" => entry.net_income = value,
            "grossProfit" => entry.gross_profit = value,
            "weightedAverageShsOutDil" => {
                if entry.diluted_shares.is_none() {
                    entry.diluted_shares = value;
                }
            }
            "commonStockSharesOutstanding" => entry.diluted_shares = value.or(entry.diluted_shares),
            _ => {}
        }
    }

    let daily_bars = WorkspaceFinancialStore::new(db)
        .load_daily_price_bars()
        .await?;
    let mut sorted_ends = period_ends.to_vec();
    sorted_ends.sort();

    for (index, period_end) in sorted_ends.iter().enumerate() {
        let prev = index.checked_sub(1).map(|idx| sorted_ends[idx].as_str());
        if let Some(entry) = actuals.get_mut(period_end) {
            entry.market_price = quarter_close_band(&daily_bars, period_end, prev);
        }
    }

    Ok(actuals)
}

fn quarter_close_band(
    daily_bars: &[DailyPriceBar],
    period_end: &str,
    prev_period_end: Option<&str>,
) -> Option<Band> {
    let mut bars: Vec<&DailyPriceBar> = daily_bars
        .iter()
        .filter(|bar| {
            bar.trade_date.as_str() <= period_end
                && prev_period_end.is_none_or(|previous| bar.trade_date.as_str() > previous)
        })
        .collect();
    if bars.is_empty() {
        return None;
    }
    bars.sort_by(|left, right| left.trade_date.cmp(&right.trade_date));
    let low = bars.iter().map(|bar| bar.low).fold(f64::INFINITY, f64::min);
    let high = bars
        .iter()
        .map(|bar| bar.high)
        .fold(f64::NEG_INFINITY, f64::max);
    let close = bars.last()?.close;
    Some(Band {
        low: round_float(low),
        median: round_float(close),
        high: round_float(high),
    })
}

fn projection_calendar_json(context: &ProjectionContext) -> Value {
    if context.period_meta.is_empty() {
        return Value::Null;
    }
    json!({
        "historical_quarters": context.historical_quarters,
        "forward_quarters": context.forward_quarters,
        "historical_anchor_end": context.historical_anchor_end,
        "terminal_period_end": context.terminal_period_end,
    })
}

fn build_historical_periods_json(
    scenarios: &[ScenarioInput],
    projection_context: &ProjectionContext,
) -> Vec<Value> {
    let label_by_end = scenarios
        .first()
        .into_iter()
        .flat_map(|scenario| scenario.periods.iter())
        .filter_map(|period| {
            period
                .period_end
                .as_ref()
                .map(|period_end| (period_end.clone(), period.label.clone()))
        })
        .collect::<HashMap<_, _>>();

    let mut historical: Vec<Value> = projection_context
        .period_meta
        .iter()
        .filter(|(_, meta)| meta.is_historical)
        .map(|(period_end, meta)| {
            let actuals = projection_context
                .quarter_actuals
                .get(period_end)
                .cloned()
                .unwrap_or_default();
            let revenue = actuals.revenue;
            let net_income = actuals.net_income;
            let diluted_shares = actuals.diluted_shares;
            let eps = ratio(actuals.net_income, actuals.diluted_shares);
            json!({
                "period_order": meta.period_order,
                "period_end": period_end,
                "label": label_by_end.get(period_end).cloned().unwrap_or_else(|| period_end.clone()),
                "period_kind": "historical",
                "is_historical": true,
                "revenue": revenue.map(round_float),
                "net_income": round_optional(net_income),
                "diluted_shares": diluted_shares.map(round_float),
                "gross_margin": round_optional(ratio(actuals.gross_profit, actuals.revenue)),
                "net_margin": round_optional(ratio(actuals.net_income, actuals.revenue)),
                "eps": round_optional(eps),
                "market_price": actuals.market_price.map(|band| band.to_json()),
                "chart_price": actuals.market_price.map(|band| band.to_json()),
                "source_note": "Alpha Vantage quarterly actual with quarter close from daily_price_bars",
            })
        })
        .collect();
    historical.sort_by_key(|period| period.get("period_order").and_then(Value::as_i64).unwrap_or(0));
    historical
}

fn band_from_parts(low: Option<f64>, median: Option<f64>, high: Option<f64>) -> Option<Band> {
    let median = median?;
    Some(Band {
        low: low.unwrap_or(median),
        median,
        high: high.unwrap_or(median),
    })
}

fn pe_eps_usable(eps_ttm: Option<f64>) -> Option<f64> {
    eps_ttm.filter(|eps| *eps > 0.0)
}

fn resolve_blend_weights(
    ps_weight: f64,
    pe_weight: f64,
    eps_ttm: Option<f64>,
    ps_multiple: Option<Band>,
    pe_multiple: Option<Band>,
) -> Result<(BlendWeights, Option<String>)> {
    let ps_available = ps_multiple.is_some();
    let pe_available = pe_multiple.is_some() && pe_eps_usable(eps_ttm).is_some();

    let (weights, rationale) = match (ps_available, pe_available) {
        (true, true) => (
            normalize_weights(ps_weight, pe_weight)?,
            None,
        ),
        (true, false) => {
            let rationale = if pe_multiple.is_some() && pe_eps_usable(eps_ttm).is_none() {
                Some(
                    "P/E implied price suppressed: TTM EPS is zero or negative; using P/S only."
                        .to_string(),
                )
            } else if pe_weight > 0.0 {
                Some(
                    "P/E blend unused: no usable P/E multiple on this period; using P/S only."
                        .to_string(),
                )
            } else {
                None
            };
            (BlendWeights { ps: 1.0, pe: 0.0 }, rationale)
        }
        (false, true) => {
            let rationale = if ps_weight > 0.0 {
                Some("P/S implied price unavailable; using P/E only.".to_string())
            } else {
                None
            };
            (BlendWeights { ps: 0.0, pe: 1.0 }, rationale)
        }
        (false, false) => (
            BlendWeights {
                ps: DEFAULT_BLEND_WEIGHT,
                pe: DEFAULT_BLEND_WEIGHT,
            },
            None,
        ),
    };

    Ok((weights, rationale))
}

fn normalize_weights(ps_weight: f64, pe_weight: f64) -> Result<BlendWeights> {
    let total = ps_weight + pe_weight;
    if total <= 0.0 {
        return Err(Error::string("blend weights must sum to a positive value"));
    }
    Ok(BlendWeights {
        ps: ps_weight / total,
        pe: pe_weight / total,
    })
}

fn apply_multiple(base_value: Option<f64>, multiple: Option<Band>) -> Option<Band> {
    let base_value = base_value?;
    let multiple = multiple?;
    Some(Band {
        low: round_float(base_value * multiple.low),
        median: round_float(base_value * multiple.median),
        high: round_float(base_value * multiple.high),
    })
}

fn blend_bands(
    ps_price: Option<Band>,
    pe_price: Option<Band>,
    weights: BlendWeights,
) -> Option<Band> {
    match (ps_price, pe_price) {
        (None, None) => None,
        (Some(ps), None) => Some(ps),
        (None, Some(pe)) => Some(pe),
        (Some(ps), Some(pe)) => Some(Band {
            low: round_float(ps.low * weights.ps + pe.low * weights.pe),
            median: round_float(ps.median * weights.ps + pe.median * weights.pe),
            high: round_float(ps.high * weights.ps + pe.high * weights.pe),
        }),
    }
}

impl Band {
    fn to_json(self) -> Value {
        json!({ "low": self.low, "median": self.median, "high": self.high })
    }

    fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            low: value.get("low")?.as_f64()?,
            median: value.get("median")?.as_f64()?,
            high: value.get("high")?.as_f64()?,
        })
    }
}

fn sample_from_price_band(rng: &mut DeterministicRng, band: Band) -> f64 {
    let spread = (band.median - band.low)
        .abs()
        .max((band.high - band.median).abs());
    if spread == 0.0 {
        return band.median.max(0.0);
    }
    let sigma = spread / P10_P90_Z_SCORE;
    (band.median + sigma * rng.next_standard_normal()).max(0.0)
}

fn distribution_summary(samples: &[f64]) -> BTreeMap<String, f64> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance =
        samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / samples.len() as f64;
    BTreeMap::from([
        ("min".to_string(), round_float(sorted[0])),
        ("p10".to_string(), round_float(percentile(&sorted, 0.10))),
        ("p25".to_string(), round_float(percentile(&sorted, 0.25))),
        ("median".to_string(), round_float(percentile(&sorted, 0.50))),
        ("mean".to_string(), round_float(mean)),
        ("p75".to_string(), round_float(percentile(&sorted, 0.75))),
        ("p90".to_string(), round_float(percentile(&sorted, 0.90))),
        (
            "max".to_string(),
            round_float(sorted[sorted.len() - 1]),
        ),
        ("stdev".to_string(), round_float(variance.sqrt())),
    ])
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let position = (sorted.len() - 1) as f64 * fraction;
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(sorted.len() - 1);
    let weight = position - lower as f64;
    sorted[lower] * (1.0 - weight) + sorted[upper] * weight
}

fn histogram(samples: &[f64], bins: usize) -> Vec<HistogramBin> {
    let minimum = samples.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if minimum == maximum {
        return vec![HistogramBin {
            low: round_float(minimum),
            high: round_float(maximum),
            midpoint: round_float(minimum),
            count: samples.len(),
            probability: 1.0,
        }];
    }
    let width = (maximum - minimum) / bins as f64;
    let mut counts = vec![0usize; bins];
    for sample in samples {
        let index = (((sample - minimum) / width).floor() as usize).min(bins - 1);
        counts[index] += 1;
    }
    counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| HistogramBin {
            low: round_float(minimum + index as f64 * width),
            high: round_float(minimum + (index + 1) as f64 * width),
            midpoint: round_float(minimum + (index as f64 + 0.5) * width),
            count,
            probability: round_float(count as f64 / samples.len() as f64),
        })
        .collect()
}

#[derive(Debug, Clone)]
struct DeterministicRng {
    state: u64,
    spare_normal: Option<f64>,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed.max(1),
            spare_normal: None,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.state
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn next_standard_normal(&mut self) -> f64 {
        if let Some(spare) = self.spare_normal.take() {
            return spare;
        }
        let mut u1 = self.next_f64();
        while u1 <= f64::MIN_POSITIVE {
            u1 = self.next_f64();
        }
        let u2 = self.next_f64();
        let mag = (-2.0 * u1.ln()).sqrt();
        let z0 = mag * (2.0 * std::f64::consts::PI * u2).cos();
        let z1 = mag * (2.0 * std::f64::consts::PI * u2).sin();
        self.spare_normal = Some(z1);
        z0
    }
}

fn growth_rate(value: Option<f64>, previous: Option<f64>) -> Option<f64> {
    match (value, previous) {
        (Some(value), Some(previous)) if previous != 0.0 => Some((value / previous) - 1.0),
        _ => None,
    }
}

fn ratio(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    match (numerator, denominator) {
        (Some(numerator), Some(denominator)) if denominator != 0.0 => Some(numerator / denominator),
        _ => None,
    }
}

fn round_float(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn round_optional(value: Option<f64>) -> Option<f64> {
    value.map(round_float)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter_period(
        label: &str,
        revenue: f64,
        net_income: f64,
        shares: f64,
        ps_median: Option<f64>,
        pe_median: Option<f64>,
    ) -> ScenarioPeriodRow {
        ScenarioPeriodRow {
            period_order: 0,
            label: label.to_string(),
            period_end: None,
            period_type: Some("quarter".to_string()),
            revenue: Some(revenue),
            revenue_growth: None,
            diluted_shares: Some(shares),
            gross_margin: None,
            operating_margin: None,
            net_margin: Some(net_income / revenue),
            net_income: Some(net_income),
            eps: Some(net_income / shares),
            ps_low: ps_median,
            ps_median,
            ps_high: ps_median,
            pe_low: pe_median,
            pe_median,
            pe_high: pe_median,
            blend_ps_weight: 0.5,
            blend_pe_weight: 0.5,
            source_note: None,
        }
    }

    fn empty_projection_context() -> ProjectionContext {
        ProjectionContext::default()
    }

    #[test]
    fn suppresses_pe_blend_when_ttm_eps_is_non_positive() {
        let scenario = ScenarioInput {
            id: 1,
            name: "loss path".to_string(),
            stance: "bearish".to_string(),
            probability: Some(1.0),
            description: String::new(),
            assumption_summary: None,
            crux_assumptions: Vec::new(),
            sensitivities: Vec::new(),
            confirming_signals: Vec::new(),
            breaking_signals: Vec::new(),
            periods: vec![
                quarter_period("Q1", 100.0, -10.0, 10.0, Some(2.0), Some(10.0)),
                quarter_period("Q2", 110.0, -11.0, 10.0, Some(2.0), Some(10.0)),
                quarter_period("Q3", 120.0, -12.0, 10.0, Some(2.0), Some(10.0)),
                quarter_period("Q4", 130.0, -13.0, 10.0, Some(2.0), Some(10.0)),
            ],
        };

        let value = build_scenario_json(
            &scenario,
            100.0,
            10.0,
            Some(-0.1),
            Some(-1.0),
            &empty_projection_context(),
        )
            .expect("scenario json");
        let terminal = value["periods"]
            .as_array()
            .and_then(|periods| periods.last())
            .expect("terminal period");

        assert!(terminal["pe_implied_price"].is_null());
        assert_eq!(terminal["blend_weights"]["ps"].as_f64(), Some(1.0));
        assert_eq!(terminal["blend_weights"]["pe"].as_f64(), Some(0.0));
        assert!(terminal["blend_rationale"]
            .as_str()
            .is_some_and(|note| note.contains("TTM EPS")));
        assert_eq!(
            terminal["blended_price"]["median"].as_f64(),
            terminal["ps_implied_price"]["median"].as_f64()
        );
    }

    #[test]
    fn applies_valuation_multiples_to_each_period_with_ttm_window() {
        let scenario = ScenarioInput {
            id: 1,
            name: "test".to_string(),
            stance: "neutral".to_string(),
            probability: Some(1.0),
            description: String::new(),
            assumption_summary: None,
            crux_assumptions: Vec::new(),
            sensitivities: Vec::new(),
            confirming_signals: Vec::new(),
            breaking_signals: Vec::new(),
            periods: vec![
                quarter_period("Q1", 100.0, 10.0, 10.0, Some(2.0), Some(10.0)),
                quarter_period("Q2", 110.0, 11.0, 10.0, Some(2.0), Some(10.0)),
                quarter_period("Q3", 120.0, 12.0, 10.0, Some(2.0), Some(10.0)),
                quarter_period("Q4", 130.0, 13.0, 10.0, Some(2.0), Some(10.0)),
            ],
        };

        let value = build_scenario_json(
            &scenario,
            100.0,
            10.0,
            Some(0.1),
            Some(1.0),
            &empty_projection_context(),
        )
            .expect("scenario json");
        let periods = value["periods"].as_array().expect("periods");

        assert_eq!(periods.len(), 4);
        assert!(periods[0]["blended_price"].is_null());
        assert!(periods[2]["blended_price"].is_null());
        assert!(periods[3]["blended_price"]["median"].is_number());
    }

    #[test]
    fn applies_valuation_multiples_to_ttm_per_share_metrics() {
        let scenario = ScenarioInput {
            id: 1,
            name: "test".to_string(),
            stance: "neutral".to_string(),
            probability: Some(1.0),
            description: String::new(),
            assumption_summary: None,
            crux_assumptions: Vec::new(),
            sensitivities: Vec::new(),
            confirming_signals: Vec::new(),
            breaking_signals: Vec::new(),
            periods: vec![
                quarter_period("Q1", 100.0, 10.0, 10.0, None, None),
                quarter_period("Q2", 110.0, 11.0, 10.0, None, None),
                quarter_period("Q3", 120.0, 12.0, 10.0, None, None),
                quarter_period("Q4", 130.0, 13.0, 10.0, Some(2.0), Some(10.0)),
            ],
        };

        let value = build_scenario_json(
            &scenario,
            440.0,
            10.0,
            Some(0.1),
            Some(4.4),
            &empty_projection_context(),
        )
            .expect("scenario json");
        let terminal = value["periods"]
            .as_array()
            .and_then(|periods| periods.last())
            .expect("terminal period");

        assert_eq!(terminal["eps"].as_f64(), Some(1.3));
        assert_eq!(terminal["eps_ttm"].as_f64(), Some(4.6));
        assert_eq!(terminal["revenue_per_share_ttm"].as_f64(), Some(46.0));
        assert_eq!(terminal["multiple_basis"].as_str(), Some(VALUATION_MULTIPLE_BASIS));
        assert_eq!(terminal["pe_implied_price"]["median"].as_f64(), Some(46.0));
        assert_eq!(terminal["ps_implied_price"]["median"].as_f64(), Some(92.0));
        assert_eq!(terminal["blended_price"]["median"].as_f64(), Some(69.0));
    }

    #[test]
    fn defers_implied_prices_until_four_quarters_are_available() {
        let scenario = ScenarioInput {
            id: 1,
            name: "test".to_string(),
            stance: "neutral".to_string(),
            probability: Some(1.0),
            description: String::new(),
            assumption_summary: None,
            crux_assumptions: Vec::new(),
            sensitivities: Vec::new(),
            confirming_signals: Vec::new(),
            breaking_signals: Vec::new(),
            periods: vec![quarter_period("Q1", 100.0, 10.0, 10.0, Some(2.0), Some(10.0))],
        };

        let value = build_scenario_json(
            &scenario,
            100.0,
            10.0,
            Some(0.1),
            Some(1.0),
            &empty_projection_context(),
        )
            .expect("scenario json");
        let terminal = &value["periods"][0];

        assert!(terminal["eps_ttm"].is_null());
        assert!(terminal["pe_implied_price"].is_null());
        assert!(terminal["ps_implied_price"].is_null());
        assert!(terminal["blended_price"].is_null());
    }

    #[test]
    fn ttm_window_uses_trailing_four_quarters_only() {
        let window = vec![
            QuarterWindowEntry {
                revenue: 1.0,
                net_income: 0.1,
            },
            QuarterWindowEntry {
                revenue: 2.0,
                net_income: 0.2,
            },
            QuarterWindowEntry {
                revenue: 3.0,
                net_income: 0.3,
            },
            QuarterWindowEntry {
                revenue: 4.0,
                net_income: 0.4,
            },
            QuarterWindowEntry {
                revenue: 100.0,
                net_income: 10.0,
            },
        ];
        let (revenue_ttm, revenue_per_share_ttm, eps_ttm) =
            ttm_valuation_inputs(&window[1..], 10.0).expect("ttm inputs");

        assert_eq!(revenue_ttm, 109.0);
        assert_eq!(revenue_per_share_ttm, 10.9);
        assert_eq!(eps_ttm, 1.09);
    }
}
