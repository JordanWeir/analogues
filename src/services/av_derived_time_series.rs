use crate::{
    services::av_fundamental_deriver::AV_SOURCE_TYPE,
    workspace::{AvRawFact, DailyPriceBar, FundamentalObservation, StarterFundamentals},
};
use chrono::NaiveDate;

const QUARTER_GAP_MIN_DAYS: i64 = 80;
const QUARTER_GAP_MAX_DAYS: i64 = 100;

#[derive(Debug, Clone)]
pub struct AvDerivedTimeSeries;

#[derive(Debug, Clone)]
struct AvTtmWindow {
    period_end: String,
    period_start: String,
    revenue: f64,
    net_income: f64,
    gross_profit: Option<f64>,
    operating_income: Option<f64>,
}

#[derive(Debug, Clone, Default)]
struct QuarterPriceStats {
    open: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    close: Option<f64>,
}

impl AvDerivedTimeSeries {
    pub fn derive(
        raw_facts: &[AvRawFact],
        daily_bars: &[DailyPriceBar],
        currency: Option<&str>,
    ) -> (Vec<FundamentalObservation>, Vec<String>) {
        let mut observations = Vec::new();
        let mut quality_flags = Vec::new();
        let unit = currency.unwrap_or("USD");
        let default_shares = default_shares_outstanding(raw_facts);

        let quarter_ends = sorted_quarter_ends(raw_facts);
        if quarter_ends.is_empty() {
            quality_flags.push("av_derived_time_series_no_quarterly_periods".to_string());
            return (observations, quality_flags);
        }

        let ttm_windows = build_ttm_windows(raw_facts, &quarter_ends);
        if ttm_windows.is_empty() {
            quality_flags.push("av_derived_time_series_no_contiguous_ttm_windows".to_string());
        }

        let ttm_by_end: std::collections::BTreeMap<String, AvTtmWindow> = ttm_windows
            .iter()
            .map(|window| (window.period_end.clone(), window.clone()))
            .collect();

        for (index, period_end) in quarter_ends.iter().enumerate() {
            let prev_period_end = quarter_ends.get(index + 1).map(String::as_str);
            let shares = shares_for_quarter(raw_facts, period_end, default_shares);
            let revenue = quarterly_value(raw_facts, "totalRevenue", period_end);
            let net_income = quarterly_value(raw_facts, "netIncome", period_end);

            if let Some(shares) = shares {
                observations.push(derived_observation(
                    "diluted_shares_quarter",
                    "Diluted shares",
                    "balance_sheet",
                    "quarter",
                    period_end,
                    prev_period_end,
                    shares,
                    "shares",
                    "Quarterly share count from Alpha Vantage balance sheet or overview fallback.",
                    Some("shares_outstanding"),
                ));
            }

            if let (Some(revenue), Some(shares)) = (revenue, shares) {
                if let Some(value) = ratio(Some(revenue), Some(shares)) {
                    observations.push(derived_observation(
                        "revenue_per_share_quarter",
                        "Revenue per share",
                        "income_statement",
                        "quarter",
                        period_end,
                        prev_period_end,
                        value,
                        unit,
                        "Quarterly revenue divided by shares outstanding.",
                        Some("revenue"),
                    ));
                }
            }

            if quarterly_value(raw_facts, "reportedEPS", period_end).is_none() {
                if let (Some(net_income), Some(shares)) = (net_income, shares) {
                    if let Some(value) = ratio(Some(net_income), Some(shares)) {
                        observations.push(derived_observation(
                            "eps_quarter",
                            "Reported EPS",
                            "income_statement",
                            "quarter",
                            period_end,
                            prev_period_end,
                            value,
                            unit,
                            "Quarterly EPS derived from net income divided by shares.",
                            Some("eps"),
                        ));
                    }
                }
            }

            if let Some(window) = ttm_by_end.get(period_end) {
                let shares = shares.or(default_shares);
                append_ttm_observations(&mut observations, window, shares, unit);

                if let Some(shares) = shares {
                    let ttm_eps = ratio(Some(window.net_income), Some(shares));
                    let revenue_per_share_ttm =
                        ratio(Some(window.revenue), Some(shares));
                    append_valuation_band_observations(
                        &mut observations,
                        period_end,
                        prev_period_end,
                        ttm_eps,
                        revenue_per_share_ttm,
                        daily_bars,
                    );
                }
            }

            if let Some(stats) = quarter_price_stats(daily_bars, period_end, prev_period_end) {
                append_quarter_price_observations(
                    &mut observations,
                    period_end,
                    prev_period_end,
                    &stats,
                    unit,
                );
            } else if !daily_bars.is_empty() {
                quality_flags.push(format!(
                    "av_quarter_price_hloc_unavailable_for_{period_end}"
                ));
            }
        }

        (observations, quality_flags)
    }

    pub fn apply_latest_ttm_to_starter(
        starter: &mut StarterFundamentals,
        raw_facts: &[AvRawFact],
    ) {
        let quarter_ends = sorted_quarter_ends(raw_facts);
        let Some(latest) = build_ttm_windows(raw_facts, &quarter_ends).into_iter().next() else {
            return;
        };

        starter.fundamental_period_end = Some(latest.period_end.clone());
        starter.revenue_ttm = Some(latest.revenue);
        starter.net_income_ttm = Some(latest.net_income);
        starter.gross_profit_ttm = latest.gross_profit;
        starter.operating_income_ttm = latest.operating_income;
        starter.gross_margin = ratio(latest.gross_profit, Some(latest.revenue));
        starter.operating_margin = ratio(latest.operating_income, Some(latest.revenue));
        starter.net_margin = ratio(Some(latest.net_income), Some(latest.revenue));

        let shares = shares_for_quarter(raw_facts, &latest.period_end, default_shares_outstanding(raw_facts));
        starter.eps_ttm = ratio(Some(latest.net_income), shares);
    }
}

fn append_ttm_observations(
    observations: &mut Vec<FundamentalObservation>,
    window: &AvTtmWindow,
    shares: Option<f64>,
    unit: &str,
) {
    observations.push(derived_observation(
        "revenue_ttm",
        "Revenue TTM",
        "income_statement",
        "ttm",
        &window.period_end,
        Some(&window.period_start),
        window.revenue,
        unit,
        "Revenue summed across four contiguous Alpha Vantage quarters.",
        Some("revenue"),
    ));
    observations.push(derived_observation(
        "net_income_ttm",
        "Net income TTM",
        "income_statement",
        "ttm",
        &window.period_end,
        Some(&window.period_start),
        window.net_income,
        unit,
        "Net income summed across four contiguous Alpha Vantage quarters.",
        Some("net_income"),
    ));
    if let Some(gross_profit) = window.gross_profit {
        observations.push(derived_observation(
            "gross_profit_ttm",
            "Gross profit TTM",
            "income_statement",
            "ttm",
            &window.period_end,
            Some(&window.period_start),
            gross_profit,
            unit,
            "Gross profit summed across four contiguous Alpha Vantage quarters.",
            Some("gross_profit"),
        ));
    }
    if let Some(operating_income) = window.operating_income {
        observations.push(derived_observation(
            "operating_income_ttm",
            "Operating income TTM",
            "income_statement",
            "ttm",
            &window.period_end,
            Some(&window.period_start),
            operating_income,
            unit,
            "Operating income summed across four contiguous Alpha Vantage quarters.",
            Some("operating_income"),
        ));
    }
    if let Some(shares) = shares {
        if let Some(value) = ratio(Some(window.net_income), Some(shares)) {
            observations.push(derived_observation(
                "eps_ttm",
                "EPS TTM",
                "income_statement",
                "ttm",
                &window.period_end,
                Some(&window.period_start),
                value,
                unit,
                "TTM net income divided by quarter-end shares outstanding.",
                Some("eps"),
            ));
        }
        if let Some(value) = ratio(Some(window.revenue), Some(shares)) {
            observations.push(derived_observation(
                "revenue_per_share_ttm",
                "Revenue per share TTM",
                "income_statement",
                "ttm",
                &window.period_end,
                Some(&window.period_start),
                value,
                unit,
                "TTM revenue divided by quarter-end shares outstanding.",
                Some("revenue"),
            ));
        }
    }
}

fn append_quarter_price_observations(
    observations: &mut Vec<FundamentalObservation>,
    period_end: &str,
    prev_period_end: Option<&str>,
    stats: &QuarterPriceStats,
    unit: &str,
) {
    for (metric_key, label, value) in [
        ("price_quarter_open", "Quarter open price", stats.open),
        ("price_quarter_high", "Quarter high price", stats.high),
        ("price_quarter_low", "Quarter low price", stats.low),
        ("price_quarter_close", "Quarter close price", stats.close),
    ] {
        let Some(value) = value else {
            continue;
        };
        observations.push(derived_observation(
            metric_key,
            label,
            "market",
            "quarter",
            period_end,
            prev_period_end,
            value,
            unit,
            "Aggregated from daily_price_bars within the fiscal quarter window.",
            None,
        ));
    }
}

fn append_valuation_band_observations(
    observations: &mut Vec<FundamentalObservation>,
    period_end: &str,
    prev_period_end: Option<&str>,
    ttm_eps: Option<f64>,
    revenue_per_share_ttm: Option<f64>,
    daily_bars: &[DailyPriceBar],
) {
    let bars = quarter_price_bars(daily_bars, period_end, prev_period_end);
    if let Some(ttm_eps) = ttm_eps.filter(|eps| *eps > 0.0) {
        if let Some((min_pe, max_pe)) = ratio_band(&bars, |close| close / ttm_eps) {
            for (metric_key, label, value) in [
                ("pe_quarter_min", "Minimum quarter P/E", min_pe),
                ("pe_quarter_max", "Maximum quarter P/E", max_pe),
            ] {
                observations.push(derived_observation(
                    metric_key,
                    label,
                    "valuation",
                    "quarter",
                    period_end,
                    prev_period_end,
                    value,
                    "ratio",
                    "Daily close divided by TTM EPS; undefined when TTM EPS is non-positive.",
                    None,
                ));
            }
        }
    }

    if let Some(revenue_per_share_ttm) = revenue_per_share_ttm.filter(|value| *value > 0.0) {
        if let Some((min_ps, max_ps)) =
            ratio_band(&bars, |close| close / revenue_per_share_ttm)
        {
            for (metric_key, label, value) in [
                (
                    "price_to_revenue_quarter_min",
                    "Minimum quarter price/revenue",
                    min_ps,
                ),
                (
                    "price_to_revenue_quarter_max",
                    "Maximum quarter price/revenue",
                    max_ps,
                ),
            ] {
                observations.push(derived_observation(
                    metric_key,
                    label,
                    "valuation",
                    "quarter",
                    period_end,
                    prev_period_end,
                    value,
                    "ratio",
                    "Daily close divided by TTM revenue per share.",
                    None,
                ));
            }
        }
    }
}

fn derived_observation(
    metric_key: &str,
    metric_label: &str,
    statement_type: &str,
    period_type: &str,
    period_end: &str,
    period_start: Option<&str>,
    value: f64,
    unit: &str,
    source_note: &str,
    canonical_key: Option<&str>,
) -> FundamentalObservation {
    FundamentalObservation {
        canonical_key: canonical_key.map(str::to_string),
        metric_key: metric_key.to_string(),
        metric_label: metric_label.to_string(),
        statement_type: statement_type.to_string(),
        period_type: period_type.to_string(),
        period_start: period_start.map(str::to_string),
        period_end: Some(period_end.to_string()),
        as_of_date: Some(period_end.to_string()),
        filed_at: None,
        fiscal_year: None,
        fiscal_period: None,
        value,
        unit: Some(unit.to_string()),
        source_type: AV_SOURCE_TYPE.to_string(),
        source_note: Some(source_note.to_string()),
        concept_name: None,
        form: None,
        accession: None,
        quality: Some("av_derived".to_string()),
        is_derived: true,
    }
}

fn sorted_quarter_ends(raw_facts: &[AvRawFact]) -> Vec<String> {
    let mut ends: Vec<String> = raw_facts
        .iter()
        .filter(|fact| fact.period_type == "quarter" && fact.report_type == "quarterly")
        .filter(|fact| fact.field_name == "totalRevenue")
        .map(|fact| fact.period_end.clone())
        .collect();
    ends.sort_by(|left, right| right.cmp(left));
    ends.dedup();
    ends
}

fn build_ttm_windows(raw_facts: &[AvRawFact], quarter_ends: &[String]) -> Vec<AvTtmWindow> {
    let mut windows = Vec::new();
    for start_index in 0..quarter_ends.len().saturating_sub(3) {
        let window_ends: Vec<&str> = quarter_ends[start_index..start_index + 4]
            .iter()
            .map(String::as_str)
            .collect();
        if !is_contiguous_av_quarter_window(&window_ends) {
            continue;
        }
        let Some(revenue) = sum_quarterly_values(raw_facts, "totalRevenue", &window_ends) else {
            continue;
        };
        let Some(net_income) = sum_quarterly_values(raw_facts, "netIncome", &window_ends) else {
            continue;
        };
        windows.push(AvTtmWindow {
            period_end: window_ends[0].to_string(),
            period_start: window_ends[3].to_string(),
            revenue,
            net_income,
            gross_profit: sum_quarterly_values(raw_facts, "grossProfit", &window_ends),
            operating_income: sum_quarterly_values(raw_facts, "operatingIncome", &window_ends),
        });
    }
    windows
}

fn is_contiguous_av_quarter_window(period_ends: &[&str]) -> bool {
    if period_ends.len() != 4 {
        return false;
    }
    let mut previous = period_ends[0];
    for period_end in &period_ends[1..] {
        let Some(gap_days) = days_between(period_end, previous) else {
            return false;
        };
        if !(QUARTER_GAP_MIN_DAYS..=QUARTER_GAP_MAX_DAYS).contains(&gap_days) {
            return false;
        }
        previous = period_end;
    }
    true
}

fn sum_quarterly_values(
    raw_facts: &[AvRawFact],
    field_name: &str,
    period_ends: &[&str],
) -> Option<f64> {
    let mut total = 0.0;
    for period_end in period_ends {
        let value = quarterly_value(raw_facts, field_name, period_end)?;
        total += value;
    }
    Some(total)
}

fn quarterly_value(raw_facts: &[AvRawFact], field_name: &str, period_end: &str) -> Option<f64> {
    raw_facts
        .iter()
        .filter(|fact| fact.field_name == field_name)
        .filter(|fact| fact.period_type == "quarter" && fact.report_type == "quarterly")
        .filter(|fact| fact.period_end == period_end)
        .map(|fact| fact.value)
        .next()
}

fn default_shares_outstanding(raw_facts: &[AvRawFact]) -> Option<f64> {
    raw_facts
        .iter()
        .filter(|fact| fact.field_name == "SharesOutstanding")
        .max_by(|left, right| left.period_end.cmp(&right.period_end))
        .map(|fact| fact.value)
}

fn shares_for_quarter(
    raw_facts: &[AvRawFact],
    period_end: &str,
    fallback: Option<f64>,
) -> Option<f64> {
    raw_facts
        .iter()
        .filter(|fact| fact.endpoint == "BALANCE_SHEET")
        .filter(|fact| fact.field_name == "commonStockSharesOutstanding")
        .filter(|fact| fact.period_type == "quarter" && fact.report_type == "quarterly")
        .filter(|fact| fact.period_end == period_end)
        .map(|fact| fact.value)
        .next()
        .or(fallback)
}

fn quarter_price_bars<'a>(
    daily_bars: &'a [DailyPriceBar],
    period_end: &str,
    prev_period_end: Option<&str>,
) -> Vec<&'a DailyPriceBar> {
    let mut bars: Vec<&DailyPriceBar> = daily_bars
        .iter()
        .filter(|bar| {
            bar.trade_date.as_str() <= period_end
                && prev_period_end.is_none_or(|previous| bar.trade_date.as_str() > previous)
        })
        .collect();
    bars.sort_by(|left, right| left.trade_date.cmp(&right.trade_date));
    bars
}

fn quarter_price_stats(
    daily_bars: &[DailyPriceBar],
    period_end: &str,
    prev_period_end: Option<&str>,
) -> Option<QuarterPriceStats> {
    let bars = quarter_price_bars(daily_bars, period_end, prev_period_end);
    if bars.is_empty() {
        return None;
    }
    Some(QuarterPriceStats {
        open: bars.first().map(|bar| bar.open),
        high: bars.iter().map(|bar| bar.high).reduce(f64::max),
        low: bars.iter().map(|bar| bar.low).reduce(f64::min),
        close: bars.last().map(|bar| bar.close),
    })
}

fn ratio_band<F>(bars: &[&DailyPriceBar], ratio_for_close: F) -> Option<(f64, f64)>
where
    F: Fn(f64) -> f64,
{
    if bars.is_empty() {
        return None;
    }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for bar in bars {
        let value = ratio_for_close(bar.close);
        if value.is_finite() {
            min = min.min(value);
            max = max.max(value);
        }
    }
    (min.is_finite() && max.is_finite()).then_some((min, max))
}

fn days_between(start: &str, end: &str) -> Option<i64> {
    let start = NaiveDate::parse_from_str(start, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").ok()?;
    Some((end - start).num_days())
}

fn ratio(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    match (numerator, denominator) {
        (Some(numerator), Some(denominator)) if denominator != 0.0 => Some(numerator / denominator),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::AvRawFact;

    fn quarter_fact(field_name: &str, value: f64, period_end: &str) -> AvRawFact {
        AvRawFact {
            endpoint: "INCOME_STATEMENT".to_string(),
            report_type: "quarterly".to_string(),
            field_name: field_name.to_string(),
            label: None,
            period_end: period_end.to_string(),
            period_type: "quarter".to_string(),
            unit: "USD".to_string(),
            currency: Some("USD".to_string()),
            value,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-13T00:00:00Z".to_string(),
        }
    }

    fn balance_shares(value: f64, period_end: &str) -> AvRawFact {
        AvRawFact {
            endpoint: "BALANCE_SHEET".to_string(),
            report_type: "quarterly".to_string(),
            field_name: "commonStockSharesOutstanding".to_string(),
            label: None,
            period_end: period_end.to_string(),
            period_type: "quarter".to_string(),
            unit: "shares".to_string(),
            currency: None,
            value,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-13T00:00:00Z".to_string(),
        }
    }

    fn four_quarter_facts() -> Vec<AvRawFact> {
        let quarters = [
            ("2024-03-31", 20.0, 2.0),
            ("2024-06-30", 22.0, 2.2),
            ("2024-09-30", 24.0, 2.4),
            ("2024-12-31", 26.0, 2.6),
        ];
        let mut facts = Vec::new();
        for (period_end, revenue, net_income) in quarters {
            facts.push(quarter_fact("totalRevenue", revenue, period_end));
            facts.push(quarter_fact("netIncome", net_income, period_end));
            facts.push(balance_shares(10.0, period_end));
        }
        facts.push(AvRawFact {
            endpoint: "OVERVIEW".to_string(),
            report_type: "overview".to_string(),
            field_name: "SharesOutstanding".to_string(),
            label: None,
            period_end: "2024-12-31".to_string(),
            period_type: "instant".to_string(),
            unit: "shares".to_string(),
            currency: None,
            value: 10.0,
            raw_json: "{}".to_string(),
            fetched_at: "2026-06-13T00:00:00Z".to_string(),
        });
        facts
    }

    #[test]
    fn builds_ttm_window_from_four_contiguous_quarters() {
        let facts = four_quarter_facts();
        let ends = sorted_quarter_ends(&facts);
        let windows = build_ttm_windows(&facts, &ends);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].revenue, 92.0);
        assert_eq!(windows[0].net_income, 9.2);
        assert_eq!(windows[0].period_end, "2024-12-31");
    }

    #[test]
    fn derives_eps_revenue_per_share_and_ttm_observations() {
        let facts = four_quarter_facts();
        let bars = vec![
            DailyPriceBar {
                trade_date: "2024-10-01".to_string(),
                open: 90.0,
                high: 100.0,
                low: 85.0,
                close: 95.0,
                volume: 1_000.0,
                adjusted_close: None,
            },
            DailyPriceBar {
                trade_date: "2024-12-31".to_string(),
                open: 100.0,
                high: 110.0,
                low: 95.0,
                close: 105.0,
                volume: 1_000.0,
                adjusted_close: None,
            },
        ];
        let (observations, _) = AvDerivedTimeSeries::derive(&facts, &bars, Some("USD"));
        assert!(observations.iter().any(|observation| {
            observation.metric_key == "eps_ttm" && (observation.value - 0.92).abs() < 1e-6
        }));
        assert!(observations.iter().any(|observation| {
            observation.metric_key == "revenue_per_share_quarter"
                && observation.period_end.as_deref() == Some("2024-12-31")
                && observation.value == 2.6
        }));
        assert!(observations
            .iter()
            .any(|observation| observation.metric_key == "price_quarter_high"));
        assert!(observations
            .iter()
            .any(|observation| observation.metric_key == "pe_quarter_max"));
    }

    #[test]
    fn skips_pe_band_when_ttm_eps_is_non_positive() {
        let mut facts = four_quarter_facts();
        for fact in facts.iter_mut().filter(|fact| fact.field_name == "netIncome") {
            fact.value = -1.0;
        }
        let bars = vec![DailyPriceBar {
            trade_date: "2024-12-31".to_string(),
            open: 100.0,
            high: 110.0,
            low: 95.0,
            close: 105.0,
            volume: 1_000.0,
            adjusted_close: None,
        }];
        let (observations, _) = AvDerivedTimeSeries::derive(&facts, &bars, Some("USD"));
        assert!(!observations
            .iter()
            .any(|observation| observation.metric_key.starts_with("pe_quarter")));
    }

    #[test]
    fn apply_latest_ttm_updates_starter_headlines() {
        let facts = four_quarter_facts();
        let mut starter = StarterFundamentals::default();
        AvDerivedTimeSeries::apply_latest_ttm_to_starter(&mut starter, &facts);
        assert_eq!(starter.revenue_ttm, Some(92.0));
        assert_eq!(starter.net_income_ttm, Some(9.2));
        assert!((starter.eps_ttm.unwrap() - 0.92).abs() < 1e-6);
    }
}
