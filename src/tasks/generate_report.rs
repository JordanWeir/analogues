use crate::services::workspace_sql::{execute_sql, sql_number, sql_quote, sql_value};
use chrono::{NaiveDate, Utc};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, QueryResult, Statement};
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_REPORT_ROOT: &str = "reports/stock-narrative-research";
const RUN_DB_FILENAME: &str = "run.sqlite";
const REPORT_TEMPLATE_PATH: &str = ".agents/skills/stock-agent2/templates/report.html.j2";
const PROJECTION_NOTE: &str = "These scenario projections are illustrative and assumption-driven. They are not predictions, price targets, or investment advice. They show how different narrative outcomes could translate into financial assumptions and valuation ranges.";
const P10_P90_Z_SCORE: f64 = 1.281_551_565_544_600_4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateReportRequest {
    pub ticker: String,
    pub date: String,
    pub index: Option<u32>,
    pub base_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct WorkspacePaths {
    sqlite_path: PathBuf,
    generated_dir: PathBuf,
}

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

pub struct GenerateReport;

#[async_trait]
impl Task for GenerateReport {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "generateReport".to_string(),
            detail: "Calculate scenario output and render the stock narrative report".to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let request = GenerateReportRequest::from_vars(vars)?;
        let output = generate_report(&request).await?;

        println!("Generated report: {}", output.display());

        Ok(())
    }
}

impl GenerateReportRequest {
    pub fn from_vars(vars: &task::Vars) -> Result<Self> {
        let ticker = vars
            .cli
            .get("ticker")
            .or_else(|| vars.cli.get("symbol"))
            .map(String::as_str)
            .ok_or_else(|| {
                Error::string("generateReport requires ticker:<SYMBOL>, for example ticker:MSFT")
            })?;

        let date = vars
            .cli
            .get("date")
            .cloned()
            .unwrap_or_else(|| Utc::now().date_naive().format("%Y-%m-%d").to_string());
        validate_date(&date)?;

        let index = vars
            .cli
            .get("index")
            .map(|value| {
                value
                    .parse::<u32>()
                    .map_err(|_| Error::string("index must be a positive integer"))
                    .and_then(|index| {
                        if index == 0 {
                            Err(Error::string("index must be a positive integer"))
                        } else {
                            Ok(index)
                        }
                    })
            })
            .transpose()?;

        let base_dir = vars
            .cli
            .get("base_dir")
            .map_or_else(|| PathBuf::from(DEFAULT_REPORT_ROOT), PathBuf::from);

        Ok(Self {
            ticker: normalize_ticker(ticker)?,
            date,
            index,
            base_dir,
        })
    }
}

pub async fn generate_report(request: &GenerateReportRequest) -> Result<PathBuf> {
    let paths = resolve_workspace_paths(request)?;
    fs::create_dir_all(&paths.generated_dir).map_err(|err| {
        Error::string(&format!(
            "failed to create generated directory {}: {err}",
            paths.generated_dir.display()
        ))
    })?;

    let db = Database::connect(sqlite_uri(&paths.sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open run SQLite database: {err}")))?;

    let result: Result<PathBuf> = async {
        let report = compile_report_payload(&db).await?;
        let report_path = paths.generated_dir.join("report.html");
        let html = render_report(&report)?;
        fs::write(&report_path, html).map_err(|err| {
            Error::string(&format!(
                "failed to write report HTML {}: {err}",
                report_path.display()
            ))
        })?;
        record_artifact(&db, &report_path).await?;
        Ok(report_path)
    }
    .await;

    let close_result = db
        .close()
        .await
        .map_err(|err| Error::string(&format!("failed to close run SQLite database: {err}")));

    let report_path = result?;
    close_result?;
    Ok(report_path)
}

async fn compile_report_payload(db: &sea_orm::DatabaseConnection) -> Result<Value> {
    let stock = load_stock_info(db).await?;
    let fundamentals = load_fundamentals(db).await?;
    let observations = load_fundamental_observations(db).await?;
    let run_metadata = load_run_metadata(db).await?;
    let data_gaps = load_data_gaps(db).await?;
    let quality_flags = load_data_quality_flags(db).await?;
    let sources = load_sources(db).await?;
    let claims = load_claims(db).await?;
    let scenarios = load_scenarios(db).await?;
    let monte_carlo_config = load_monte_carlo_config(db).await?;

    validate_report_inputs(&fundamentals, &sources, &claims, &scenarios)?;

    let scenario_data =
        build_scenario_data(&stock, &fundamentals, &scenarios, &monte_carlo_config)?;
    let scenario_outputs = scenario_outputs_from_value(&scenario_data, &scenarios);
    let monte_carlo = build_monte_carlo(&monte_carlo_config, &scenario_outputs);
    persist_monte_carlo(db, &monte_carlo).await?;

    let mut scenario_data_map = scenario_data.as_object().cloned().unwrap_or_default();
    scenario_data_map.insert("monte_carlo".to_string(), monte_carlo.to_json());
    let scenario_data = Value::Object(scenario_data_map);

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
            if period.ps_median.is_none() {
                errors.push(format!(
                    "scenario '{}' period '{}' needs ps_median",
                    scenario.name, period.label
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::string(&format!(
            "generateReport cannot render yet:\n- {}",
            errors.join("\n- ")
        )))
    }
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
    let baseline_source_note = fundamentals
        .get("revenue_ttm")
        .and_then(|metric| metric.source_note.clone());
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
            "source_note": baseline_source_note,
        },
        "scenarios": scenario_values,
        "monte_carlo": {
            "iterations": config.iterations,
            "seed": config.seed,
            "bins": config.bins,
            "summary": {},
            "histogram": [],
            "scenario_probabilities": [],
        },
        "source_notes": stock.source_note.iter().cloned().collect::<Vec<_>>(),
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
        let previous_net_income = previous_margin.map(|margin| previous_revenue * margin);
        let net_income_growth = growth_rate(net_income, previous_net_income);
        let eps_growth = growth_rate(eps, previous_eps);
        let revenue_per_share = revenue / diluted_shares;
        let ps_multiple = band_from_parts(period.ps_low, period.ps_median, period.ps_high)
            .ok_or_else(|| {
                Error::string(&format!(
                    "scenario '{}' period '{}' needs ps_median",
                    scenario.name, period.label
                ))
            })?;
        let pe_multiple = band_from_parts(period.pe_low, period.pe_median, period.pe_high);
        let blend_weights = normalize_weights(period.blend_ps_weight, period.blend_pe_weight)?;
        let ps_implied_price = apply_multiple(Some(revenue_per_share), Some(ps_multiple));
        let pe_implied_price = apply_multiple(eps, pe_multiple);
        let blended_price = blend_bands(ps_implied_price, pe_implied_price, blend_weights);

        periods.push(json!({
            "label": period.label.clone(),
            "revenue_growth": round_optional(revenue_growth),
            "revenue": round_float(revenue),
            "diluted_shares": round_float(diluted_shares),
            "revenue_per_share": round_float(revenue_per_share),
            "gross_margin": period.gross_margin,
            "operating_margin": period.operating_margin,
            "net_margin": net_margin,
            "net_income": round_optional(net_income),
            "net_income_growth": round_optional(net_income_growth),
            "eps": round_optional(eps),
            "eps_growth": round_optional(eps_growth),
            "ps_multiple": ps_multiple.to_json(),
            "pe_multiple": pe_multiple.map(|band| band.to_json()),
            "blend_weights": {"ps": blend_weights.ps, "pe": blend_weights.pe},
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
        "name": scenario.name.clone(),
        "stance": scenario.stance.clone(),
        "probability": scenario.probability,
        "description": scenario.description.clone(),
        "assumption_summary": scenario.assumption_summary.clone().unwrap_or_default(),
        "crux_assumptions": scenario.crux_assumptions.clone(),
        "sensitivities": scenario.sensitivities.clone(),
        "confirming_signals": scenario.confirming_signals.clone(),
        "breaking_signals": scenario.breaking_signals.clone(),
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
                    .map(|scenario| scenario.id)
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
        price_field: Some("terminal blended price, falling back to P/S or P/E implied price".to_string()),
        probability_basis: Some("Scenario probabilities are normalized across scenarios with terminal price bands; equal weights are used only when no positive probabilities are supplied.".to_string()),
        normal_distribution_basis: Some("Each scenario's low / median / high terminal band is treated as an approximate P10 / P50 / P90 normal distribution, floored at zero.".to_string()),
        methodology: None,
        summary: distribution_summary(&samples),
        histogram: histogram(&samples, config.bins),
        scenario_probabilities,
    }
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

fn sampling_specs(scenarios: &[ScenarioOutput]) -> Vec<SamplingSpec> {
    let mut specs: Vec<SamplingSpec> = scenarios
        .iter()
        .filter_map(|scenario| {
            let band = scenario.terminal_band?;
            Some(SamplingSpec {
                scenario_id: scenario.id,
                name: scenario.name.clone(),
                input_probability: scenario.probability,
                raw_probability: scenario
                    .probability
                    .filter(|value| *value > 0.0)
                    .unwrap_or(0.0),
                normalized_probability: 0.0,
                band,
            })
        })
        .collect();

    if specs.is_empty() {
        return specs;
    }
    let total_probability: f64 = specs.iter().map(|spec| spec.raw_probability).sum();
    if total_probability <= 0.0 {
        let equal_probability = 1.0 / specs.len() as f64;
        for spec in &mut specs {
            spec.normalized_probability = equal_probability;
        }
    } else {
        for spec in &mut specs {
            spec.normalized_probability = spec.raw_probability / total_probability;
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
        "revenue",
        "revenue_ttm",
        "gross_margin",
        "operating_margin",
        "net_margin",
        "eps",
        "eps_ttm",
        "diluted_shares",
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

async fn record_artifact(db: &sea_orm::DatabaseConnection, report_path: &Path) -> Result<()> {
    execute_sql(
        db,
        &format!(
            "INSERT INTO artifacts (artifact_type, path, created_at, notes)
             VALUES ('report_html', '{}', '{}', 'Rendered by generateReport')",
            sql_quote(&report_path.display().to_string()),
            sql_quote(&Utc::now().to_rfc3339()),
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

fn resolve_workspace_paths(request: &GenerateReportRequest) -> Result<WorkspacePaths> {
    let workspace_dir = match request.index {
        Some(index) => request
            .base_dir
            .join(format!("{}-{}-{}", request.ticker, request.date, index)),
        None => latest_workspace_dir(&request.base_dir, &request.ticker, &request.date)?,
    };
    let sqlite_path = workspace_dir.join(RUN_DB_FILENAME);
    let generated_dir = workspace_dir.join("generated");
    if !sqlite_path.is_file() {
        return Err(Error::string(&format!(
            "run SQLite database does not exist: {}",
            sqlite_path.display()
        )));
    }
    Ok(WorkspacePaths {
        sqlite_path,
        generated_dir,
    })
}

fn latest_workspace_dir(base_dir: &Path, ticker: &str, date: &str) -> Result<PathBuf> {
    let prefix = format!("{ticker}-{date}-");
    let mut candidates = Vec::new();
    for entry in fs::read_dir(base_dir).map_err(|err| {
        Error::string(&format!(
            "failed to read report root {}: {err}",
            base_dir.display()
        ))
    })? {
        let entry = entry.map_err(|err| Error::string(&format!("failed to read entry: {err}")))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(raw_index) = file_name.strip_prefix(&prefix) else {
            continue;
        };
        if let Ok(index) = raw_index.parse::<u32>() {
            candidates.push((index, entry.path()));
        }
    }
    candidates.sort_by_key(|(index, _)| *index);
    candidates
        .pop()
        .map(|(_, path)| path)
        .ok_or_else(|| Error::string(&format!("no run directory found for {ticker} on {date}")))
}

fn normalize_ticker(raw: &str) -> Result<String> {
    let ticker = raw.trim().to_uppercase();
    if ticker.is_empty() {
        return Err(Error::string("ticker cannot be empty"));
    }
    let valid = ticker
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));
    if !valid {
        return Err(Error::string(
            "ticker can only contain ASCII letters, numbers, dots, and hyphens",
        ));
    }
    Ok(ticker)
}

fn validate_date(date: &str) -> Result<()> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| Error::string("date must use YYYY-MM-DD format, for example date:2026-06-04"))
}

fn sqlite_uri(path: &Path) -> String {
    let normalized_path = path.to_string_lossy().replace('\\', "/");
    format!("sqlite://{normalized_path}?mode=rw")
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
        return Err(Error::string(
            "scenario period blend weights must sum to a positive value",
        ));
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
        (Some(ps_price), None) => Some(ps_price),
        (None, Some(pe_price)) => Some(pe_price),
        (Some(ps_price), Some(pe_price)) => Some(Band {
            low: round_float(ps_price.low * weights.ps + pe_price.low * weights.pe),
            median: round_float(ps_price.median * weights.ps + pe_price.median * weights.pe),
            high: round_float(ps_price.high * weights.ps + pe_price.high * weights.pe),
        }),
    }
}

impl Band {
    fn to_json(self) -> Value {
        json!({
            "low": self.low,
            "median": self.median,
            "high": self.high,
        })
    }

    fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            low: value.get("low")?.as_f64()?,
            median: value.get("median")?.as_f64()?,
            high: value.get("high")?.as_f64()?,
        })
    }
}

impl MonteCarloResult {
    fn to_json(&self) -> Value {
        json!({
            "iterations": self.iterations,
            "seed": self.seed,
            "bins": self.bins,
            "price_field": self.price_field,
            "probability_basis": self.probability_basis,
            "normal_distribution_basis": self.normal_distribution_basis,
            "methodology": self.methodology,
            "summary": self.summary,
            "histogram": self.histogram.iter().map(|bin| {
                json!({
                    "low": bin.low,
                    "high": bin.high,
                    "midpoint": bin.midpoint,
                    "count": bin.count,
                    "probability": bin.probability,
                })
            }).collect::<Vec<_>>(),
            "scenario_probabilities": self.scenario_probabilities.iter().map(|probability| {
                json!({
                    "name": probability.name,
                    "input_probability": probability.input_probability,
                    "normalized_probability": probability.normalized_probability,
                    "sample_count": probability.sample_count,
                    "observed_probability": probability.observed_probability,
                })
            }).collect::<Vec<_>>(),
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
    let mut sorted_samples = samples.to_vec();
    sorted_samples.sort_by(|left, right| left.total_cmp(right));
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    let variance = samples
        .iter()
        .map(|sample| (sample - mean).powi(2))
        .sum::<f64>()
        / samples.len() as f64;
    BTreeMap::from([
        ("min".to_string(), round_float(sorted_samples[0])),
        (
            "p10".to_string(),
            round_float(percentile(&sorted_samples, 0.10)),
        ),
        (
            "p25".to_string(),
            round_float(percentile(&sorted_samples, 0.25)),
        ),
        (
            "median".to_string(),
            round_float(percentile(&sorted_samples, 0.50)),
        ),
        ("mean".to_string(), round_float(mean)),
        (
            "p75".to_string(),
            round_float(percentile(&sorted_samples, 0.75)),
        ),
        (
            "p90".to_string(),
            round_float(percentile(&sorted_samples, 0.90)),
        ),
        (
            "max".to_string(),
            round_float(sorted_samples[sorted_samples.len() - 1]),
        ),
        ("stdev".to_string(), round_float(variance.sqrt())),
    ])
}

fn percentile(sorted_values: &[f64], fraction: f64) -> f64 {
    if sorted_values.len() == 1 {
        return sorted_values[0];
    }
    let position = (sorted_values.len() - 1) as f64 * fraction;
    let lower_index = position.floor() as usize;
    let upper_index = (lower_index + 1).min(sorted_values.len() - 1);
    let weight = position - lower_index as f64;
    sorted_values[lower_index] * (1.0 - weight) + sorted_values[upper_index] * weight
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
        let value = self.next_u64() >> 11;
        value as f64 / ((1u64 << 53) as f64)
    }

    fn next_standard_normal(&mut self) -> f64 {
        if let Some(value) = self.spare_normal.take() {
            return value;
        }
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        let radius = (-2.0 * u1.ln()).sqrt();
        let theta = std::f64::consts::TAU * u2;
        self.spare_normal = Some(radius * theta.sin());
        radius * theta.cos()
    }
}

fn growth_rate(value: Option<f64>, previous_value: Option<f64>) -> Option<f64> {
    match (value, previous_value) {
        (Some(value), Some(previous_value)) if previous_value != 0.0 => {
            Some((value / previous_value) - 1.0)
        }
        _ => None,
    }
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
