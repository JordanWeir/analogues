use crate::services::{
    canonical_mapping::ConceptMappingStrategy,
    workspace_store::{normalize_ticker, validate_date},
};
use loco_rs::prelude::*;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitWorkspaceRequest {
    pub ticker: String,
    pub date: String,
    pub base_dir: PathBuf,
    pub fetch_financials: bool,
    /// `None` ingests SEC raw facts and concept catalog only (phases 1–2).
    pub mapping_strategy: Option<ConceptMappingStrategy>,
    /// Run `build_narrative_map` after catalog when financial ingest is enabled.
    pub build_narrative_map: bool,
    /// Run crux identification and mechanics experiments after narrative map.
    pub build_financial_analysis: bool,
    /// Run scenario blueprint fan-out and Monte Carlo after financial analysis.
    pub build_scenario_generation: bool,
}

impl Default for InitWorkspaceRequest {
    fn default() -> Self {
        Self {
            ticker: String::new(),
            date: String::new(),
            base_dir: PathBuf::from("reports/stock-narrative-research"),
            fetch_financials: true,
            mapping_strategy: Some(ConceptMappingStrategy::CandidateScoring),
            build_narrative_map: true,
            build_financial_analysis: true,
            build_scenario_generation: true,
        }
    }
}

impl InitWorkspaceRequest {
    pub fn runs_narrative_map(&self) -> bool {
        self.build_narrative_map || self.build_financial_analysis || self.build_scenario_generation
    }

    pub fn normalized(&self) -> Result<Self> {
        validate_date(&self.date)?;
        Ok(Self {
            ticker: normalize_ticker(&self.ticker)?,
            date: self.date.clone(),
            base_dir: self.base_dir.clone(),
            fetch_financials: self.fetch_financials,
            mapping_strategy: self.mapping_strategy,
            build_narrative_map: self.build_narrative_map,
            build_financial_analysis: self.build_financial_analysis,
            build_scenario_generation: self.build_scenario_generation,
        })
    }
}
