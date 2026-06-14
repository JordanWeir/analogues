//! Deterministic scenario roll-forward, valuation bands, and Monte Carlo persistence.
//! Used by `scenario_generation` lane; `generate_report` may converge here later.

use crate::services::workspace_sql::{execute_sql, sql_number, sql_quote, sql_value};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, QueryResult, Statement};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

const P10_P90_Z_SCORE: f64 = 1.281_551_565_544_600_4;
const PROJECTION_NOTE: &str = "Scenario projections are illustrative and assumption-driven. They are not predictions, price targets, or investment advice.";

#[derive(Debug, Clone)]
struct FundamentalMetric {
    value: f64,
    period: Option<String>,
    source_note: Option<String>,
}

type Fundamentals = HashMap<String, FundamentalMetric>;

#[derive(Debug, Clone)]
pub struct ScenarioPeriodRow {
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

/// Compute scenario valuation bands and persist Monte Carlo outputs.
pub async fn compute_and_persist_monte_carlo(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let stock = load_stock_info(db).await?;
    let mut fundamentals = load_fundamentals(db).await?;
    enhance_baseline_from_av(db, &mut fundamentals).await?;
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
    let scenario_data = build_scenario_data(&stock, &fundamentals, &scenarios, &config)?;
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
        )?);
    }

    Ok(json!({
        "company": stock.company_name.clone().unwrap_or_default(),
        "ticker": stock.ticker.clone(),
        "currency": stock.currency.clone().unwrap_or_else(|| "USD".to_string()),
        "generated_at": Utc::now().to_rfc3339(),
        "projection_note": PROJECTION_NOTE,
        "base_year": base_year,
        "current_price": metric_value(fundamentals, "current_price"),
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

fn build_scenario_json(
    scenario: &ScenarioInput,
    baseline_revenue: f64,
    baseline_shares: f64,
    baseline_margin: Option<f64>,
    baseline_eps: Option<f64>,
) -> Result<Value> {
    let mut periods = Vec::new();
    let mut previous_revenue = baseline_revenue;
    let mut previous_shares = baseline_shares;
    let mut previous_margin = baseline_margin;
    let mut previous_eps = baseline_eps;

    for period in &scenario.periods {
        let revenue = match (period.revenue, period.revenue_growth) {
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
        let diluted_shares = period.diluted_shares.unwrap_or(previous_shares);
        let net_margin = period.net_margin.or(previous_margin);
        let net_income = period
            .net_income
            .or_else(|| net_margin.map(|margin| revenue * margin));
        let eps = period
            .eps
            .or_else(|| net_income.map(|income| income / diluted_shares))
            .or(previous_eps);
        let ps_multiple = band_from_parts(period.ps_low, period.ps_median, period.ps_high);
        let pe_multiple = band_from_parts(period.pe_low, period.pe_median, period.pe_high);
        let blend_weights = normalize_weights(period.blend_ps_weight, period.blend_pe_weight)?;
        let revenue_per_share = revenue / diluted_shares;
        let ps_implied_price = apply_multiple(Some(revenue_per_share), ps_multiple);
        let pe_implied_price = apply_multiple(eps, pe_multiple);
        let blended_price = blend_bands(ps_implied_price, pe_implied_price, blend_weights);

        periods.push(json!({
            "label": period.label,
            "period_end": period.period_end,
            "period_type": period.period_type,
            "revenue_growth": round_optional(revenue_growth),
            "revenue": round_float(revenue),
            "diluted_shares": round_float(diluted_shares),
            "revenue_per_share": round_float(revenue_per_share),
            "gross_margin": period.gross_margin,
            "operating_margin": period.operating_margin,
            "net_margin": net_margin,
            "net_income": round_optional(net_income),
            "eps": round_optional(eps),
            "ps_multiple": ps_multiple.map(|band| band.to_json()),
            "pe_multiple": pe_multiple.map(|band| band.to_json()),
            "ps_implied_price": ps_implied_price.map(|band| band.to_json()),
            "pe_implied_price": pe_implied_price.map(|band| band.to_json()),
            "blended_price": blended_price.map(|band| band.to_json()),
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
            "terminal blended price, falling back to P/S or P/E implied price".to_string(),
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
        let value = row_opt_f64(&row, 1)?
            .ok_or_else(|| Error::string(&format!("fundamentals metric '{key}' is not numeric")))?;
        map.insert(
            key,
            FundamentalMetric {
                value,
                period: row_opt_string(&row, 2)?,
                source_note: row_opt_string(&row, 3)?,
            },
        );
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
            "SELECT label, period_end, period_type, revenue, revenue_growth, diluted_shares,
                    gross_margin, operating_margin, net_margin, net_income, eps,
                    ps_low, ps_median, ps_high, pe_low, pe_median, pe_high,
                    blend_ps_weight, blend_pe_weight
             FROM scenario_periods WHERE scenario_id = {scenario_id} ORDER BY period_order"
        ),
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(ScenarioPeriodRow {
                label: row_string(&row, 0)?,
                period_end: row_opt_string(&row, 1)?,
                period_type: row_opt_string(&row, 2)?,
                revenue: row_opt_f64(&row, 3)?,
                revenue_growth: row_opt_f64(&row, 4)?,
                diluted_shares: row_opt_f64(&row, 5)?,
                gross_margin: row_opt_f64(&row, 6)?,
                operating_margin: row_opt_f64(&row, 7)?,
                net_margin: row_opt_f64(&row, 8)?,
                net_income: row_opt_f64(&row, 9)?,
                eps: row_opt_f64(&row, 10)?,
                ps_low: row_opt_f64(&row, 11)?,
                ps_median: row_opt_f64(&row, 12)?,
                ps_high: row_opt_f64(&row, 13)?,
                pe_low: row_opt_f64(&row, 14)?,
                pe_median: row_opt_f64(&row, 15)?,
                pe_high: row_opt_f64(&row, 16)?,
                blend_ps_weight: row_opt_f64(&row, 17)?.unwrap_or(0.5),
                blend_pe_weight: row_opt_f64(&row, 18)?.unwrap_or(0.5),
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

fn band_from_parts(low: Option<f64>, median: Option<f64>, high: Option<f64>) -> Option<Band> {
    let median = median?;
    Some(Band {
        low: low.unwrap_or(median),
        median,
        high: high.unwrap_or(median),
    })
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

fn round_float(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn round_optional(value: Option<f64>) -> Option<f64> {
    value.map(round_float)
}
