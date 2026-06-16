use crate::{
    agents::fundamental_catalog_manager::FundamentalCatalogManagerConfig,
    services::canonical_mapping::ConceptMappingStrategy,
};

#[derive(Debug, Clone)]
pub enum CatalogResolutionStrategy {
    Deterministic,
    #[allow(dead_code)]
    Agent(FundamentalCatalogManagerConfig),
}

impl CatalogResolutionStrategy {
    pub fn from_mapping_strategy(strategy: ConceptMappingStrategy) -> Self {
        match strategy {
            ConceptMappingStrategy::CandidateScoring | ConceptMappingStrategy::LlmReviewed => {
                Self::Deterministic
            }
        }
    }

    pub fn mapping_strategy(&self) -> ConceptMappingStrategy {
        ConceptMappingStrategy::CandidateScoring
    }

    pub fn agent_config(&self) -> Option<&FundamentalCatalogManagerConfig> {
        match self {
            Self::Agent(config) => Some(config),
            Self::Deterministic => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_legacy_strategies_to_deterministic_av() {
        let strategy = CatalogResolutionStrategy::from_mapping_strategy(
            ConceptMappingStrategy::CandidateScoring,
        );
        assert!(matches!(strategy, CatalogResolutionStrategy::Deterministic));
    }
}
