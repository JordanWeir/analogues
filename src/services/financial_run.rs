use crate::{
    services::{
        canonical_mapping::CanonicalResolutionResult,
        workspace_financial_store::FundamentalInsert,
    },
    workspace::{
        DerivedFundamentals, FundamentalObservation, MarketHeadlines, MarketQuoteSnapshot,
        SecIngestionResult, StarterFundamentals,
    },
};
use chrono::Utc;

/// Composed financial pipeline state (phases 1–4 + market).
#[derive(Debug, Clone, Default)]
pub struct FinancialRun {
    pub ticker: String,
    pub fetched_at: String,
    pub currency: Option<String>,
    pub company_name: Option<String>,
    pub data_sources: Vec<String>,
    pub source_notes: Vec<String>,
    pub quality_flags: Vec<String>,
    pub fundamental_source: Option<String>,
    pub gaps: Vec<String>,
    pub ingest: Option<SecIngestionResult>,
    pub market: Option<MarketQuoteSnapshot>,
    pub resolution: Option<CanonicalResolutionResult>,
    pub derived: Option<DerivedFundamentals>,
}

impl FinancialRun {
    pub(crate) fn new(ticker: &str) -> Self {
        Self {
            ticker: ticker.to_string(),
            fetched_at: Utc::now().to_rfc3339(),
            ..Self::default()
        }
    }

    pub(crate) fn merge_market(&mut self, market: MarketQuoteSnapshot) {
        self.fetched_at = market.fetched_at.clone();
        merge_string(&mut self.currency, market.currency.clone(), false);
        merge_string(&mut self.company_name, market.company_name.clone(), false);
        extend_unique(&mut self.data_sources, market.data_sources.clone());
        extend_unique(&mut self.source_notes, market.source_notes.clone());
        self.market = Some(market);
    }

    pub(crate) fn merge_sec_layers(&mut self, other: FinancialRun, overwrite: bool) {
        if let Some(ingest) = other.ingest {
            if overwrite || self.ingest.is_none() {
                self.ingest = Some(ingest);
            }
        }
        if let Some(resolution) = other.resolution {
            if overwrite || self.resolution.is_none() {
                self.resolution = Some(resolution);
            }
        }
        if let Some(derived) = other.derived {
            if overwrite || self.derived.is_none() {
                self.derived = Some(derived);
            }
        }
        merge_string(&mut self.company_name, other.company_name, overwrite);
        merge_string(
            &mut self.fundamental_source,
            other.fundamental_source,
            overwrite,
        );
        extend_unique(&mut self.data_sources, other.data_sources);
        extend_unique(&mut self.source_notes, other.source_notes);
        extend_unique(&mut self.quality_flags, other.quality_flags);
        if overwrite {
            self.fetched_at = other.fetched_at;
        }
    }

    pub(crate) fn apply_ingest(&mut self, ingest: SecIngestionResult) {
        self.fetched_at = ingest.fetched_at.clone();
        merge_string(&mut self.company_name, ingest.company_name.clone(), true);
        self.fundamental_source = Some(ingest.source_provider.clone());
        self.data_sources.push(ingest.source_provider.clone());
        self.source_notes.push(
            "Ingested SEC Company Facts raw data and materialized concept catalog entries."
                .to_string(),
        );
        self.ingest = Some(ingest);
    }

    pub(crate) fn apply_resolution(
        &mut self,
        resolution: CanonicalResolutionResult,
    ) {
        extend_unique(&mut self.quality_flags, resolution.quality_flags.clone());
        self.resolution = Some(resolution);
    }

    pub(crate) fn apply_derived(&mut self, derived: DerivedFundamentals) {
        extend_unique(&mut self.quality_flags, derived.quality_flags.clone());
        extend_unique(&mut self.source_notes, derived.source_notes.clone());
        self.derived = Some(derived);
    }

    fn market_headlines(&self) -> MarketHeadlines {
        self.market
            .as_ref()
            .map(|market| market.headlines.clone())
            .unwrap_or_default()
    }

    fn market_headlines_mut(&mut self) -> &mut MarketHeadlines {
        if self.market.is_none() {
            self.market = Some(MarketQuoteSnapshot {
                ticker: self.ticker.clone(),
                fetched_at: self.fetched_at.clone(),
                currency: self.currency.clone(),
                company_name: self.company_name.clone(),
                headlines: MarketHeadlines::default(),
                observations: Vec::new(),
                data_sources: Vec::new(),
                source_notes: Vec::new(),
            });
        }
        &mut self.market.as_mut().expect("market layer").headlines
    }

    pub(crate) fn starter(&self) -> StarterFundamentals {
        self.derived
            .as_ref()
            .map(|derived| derived.starter.clone())
            .unwrap_or_default()
    }

    fn starter_mut(&mut self) -> &mut StarterFundamentals {
        if self.derived.is_none() {
            self.derived = Some(DerivedFundamentals::default());
        }
        &mut self.derived.as_mut().expect("derived layer").starter
    }

    pub(crate) fn all_observations(&self) -> Vec<FundamentalObservation> {
        let mut observations = Vec::new();
        if let Some(market) = &self.market {
            observations.extend(market.observations.clone());
        }
        if let Some(derived) = &self.derived {
            observations.extend(derived.observations.clone());
        }
        observations
    }

    pub(crate) fn compute_derived_metrics(&mut self) {
        let current_price = self.market_headlines().current_price;
        let starter = self.starter();
        if self.market_headlines().market_cap.is_none() {
            let market_cap = multiply(current_price, starter.shares_outstanding);
            if market_cap.is_some() {
                self.market_headlines_mut().market_cap = market_cap;
                self.push_quality_flag("market_cap_derived_from_mixed_frequency_price_and_shares");
            }
        }
        let starter = self.starter_mut();
        if starter.gross_margin.is_none() {
            starter.gross_margin = ratio(starter.gross_profit_ttm, starter.revenue_ttm);
        }
        if starter.operating_margin.is_none() {
            starter.operating_margin = ratio(starter.operating_income_ttm, starter.revenue_ttm);
        }
        if starter.net_margin.is_none() {
            starter.net_margin = ratio(starter.net_income_ttm, starter.revenue_ttm);
        }
        if starter.eps_ttm.is_none() {
            starter.eps_ttm = ratio(starter.net_income_ttm, starter.shares_outstanding);
        }
        let eps_ttm = self.starter().eps_ttm;
        if self.market_headlines().trailing_pe.is_none() {
            let trailing_pe = ratio(current_price, eps_ttm);
            if trailing_pe.is_some() {
                self.market_headlines_mut().trailing_pe = trailing_pe;
                self.push_quality_flag(
                    "trailing_pe_uses_market_price_and_latest_filing_period_eps",
                );
            }
        }
        let market_cap = self.market_headlines().market_cap;
        let revenue_ttm = self.starter().revenue_ttm;
        if self.market_headlines().price_to_sales_ttm.is_none() {
            let price_to_sales = ratio(market_cap, revenue_ttm);
            if price_to_sales.is_some() {
                self.market_headlines_mut().price_to_sales_ttm = price_to_sales;
                self.push_quality_flag(
                    "price_to_sales_ttm_uses_market_cap_and_latest_filing_period_revenue",
                );
            }
        }
    }

    fn push_quality_flag(&mut self, flag: &str) {
        if !self.quality_flags.iter().any(|existing| existing == flag) {
            self.quality_flags.push(flag.to_string());
        }
    }

    pub(crate) fn mark_gaps(&mut self) {
        let headlines = self.market_headlines();
        let starter = self.starter();
        let required = [
            (
                "current_price",
                "current share price",
                headlines.current_price,
            ),
            ("market_cap", "market cap", headlines.market_cap),
            (
                "shares_outstanding",
                "share count",
                starter.shares_outstanding,
            ),
            ("revenue_ttm", "revenue", starter.revenue_ttm),
            ("net_margin", "net margin", starter.net_margin),
            ("eps_ttm", "EPS", starter.eps_ttm),
        ];
        self.gaps = required
            .iter()
            .filter_map(|(_, label, value)| value.is_none().then(|| (*label).to_string()))
            .collect();
    }
}

impl FinancialRun {
    pub(crate) fn fundamental_metrics(&self) -> Vec<FundamentalInsert<'_>> {
        let headlines = self.market_headlines();
        let starter = self.starter();
        let period = starter.fundamental_period_end.clone();
        let fundamental_source = self.fundamental_source.clone();
        vec![
            FundamentalInsert {
                key: "current_price",
                label: "Current price",
                value: headlines.current_price,
                text: None,
                unit: self.currency.as_deref(),
                period: None,
                source_note: Some("Yahoo chart endpoint".to_string()),
            },
            FundamentalInsert {
                key: "market_cap",
                label: "Market cap",
                value: headlines.market_cap,
                text: None,
                unit: self.currency.as_deref(),
                period: None,
                source_note: Some(
                    "Derived from price and shares when unavailable directly.".to_string(),
                ),
            },
            FundamentalInsert {
                key: "shares_outstanding",
                label: "Shares outstanding",
                value: starter.shares_outstanding,
                text: None,
                unit: Some("shares"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "revenue_ttm",
                label: "Revenue TTM",
                value: starter.revenue_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "net_income_ttm",
                label: "Net income TTM",
                value: starter.net_income_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "gross_profit_ttm",
                label: "Gross profit TTM",
                value: starter.gross_profit_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "operating_income_ttm",
                label: "Operating income TTM",
                value: starter.operating_income_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "gross_margin",
                label: "Gross margin",
                value: starter.gross_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "operating_margin",
                label: "Operating margin",
                value: starter.operating_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "net_margin",
                label: "Net margin",
                value: starter.net_margin,
                text: None,
                unit: Some("ratio"),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "eps_ttm",
                label: "EPS TTM",
                value: starter.eps_ttm,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "trailing_pe",
                label: "Trailing P/E",
                value: headlines.trailing_pe,
                text: None,
                unit: Some("multiple"),
                period: None,
                source_note: Some(
                    "Derived from current price and EPS when unavailable directly.".to_string(),
                ),
            },
            FundamentalInsert {
                key: "price_to_sales_ttm",
                label: "Price to sales TTM",
                value: headlines.price_to_sales_ttm,
                text: None,
                unit: Some("multiple"),
                period: None,
                source_note: Some(
                    "Derived from market cap and revenue when unavailable directly.".to_string(),
                ),
            },
            FundamentalInsert {
                key: "cash",
                label: "Cash and equivalents",
                value: starter.cash,
                text: None,
                unit: self.currency.as_deref(),
                period: period.clone(),
                source_note: fundamental_source.clone(),
            },
            FundamentalInsert {
                key: "total_debt",
                label: "Total debt",
                value: starter.total_debt,
                text: None,
                unit: self.currency.as_deref(),
                period,
                source_note: fundamental_source,
            },
        ]
        .into_iter()
        .filter(|metric| metric.value.is_some() || metric.text.is_some())
        .collect()
    }
}

fn merge_string(target: &mut Option<String>, update: Option<String>, overwrite: bool) {
    if update.is_some() && (overwrite || target.is_none()) {
        *target = update;
    }
}

fn extend_unique(target: &mut Vec<String>, updates: Vec<String>) {
    for update in updates {
        if !target.contains(&update) {
            target.push(update);
        }
    }
}

fn ratio(numerator: Option<f64>, denominator: Option<f64>) -> Option<f64> {
    match (numerator, denominator) {
        (Some(numerator), Some(denominator)) if denominator != 0.0 => Some(numerator / denominator),
        _ => None,
    }
}

fn multiply(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    Some(left? * right?)
}
