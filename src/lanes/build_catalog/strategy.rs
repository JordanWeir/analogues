use crate::{
    agents::fundamental_catalog_manager::FundamentalCatalogManagerConfig,
    services::canonical_mapping::ConceptMappingStrategy,
};

#[derive(Debug, Clone)]
pub enum CatalogResolutionStrategy {
    Deterministic,
    Agent(FundamentalCatalogManagerConfig),
}

impl CatalogResolutionStrategy {
    pub fn from_mapping_strategy(strategy: ConceptMappingStrategy) -> Self {
        match strategy {
            ConceptMappingStrategy::CandidateScoring => Self::Deterministic,
            ConceptMappingStrategy::LlmReviewed => {
                Self::Agent(FundamentalCatalogManagerConfig::default())
            }
        }
    }

    pub fn mapping_strategy(&self) -> ConceptMappingStrategy {
        match self {
            Self::Deterministic => ConceptMappingStrategy::CandidateScoring,
            Self::Agent(_) => ConceptMappingStrategy::LlmReviewed,
        }
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
    fn maps_candidate_scoring_to_deterministic() {
        let strategy = CatalogResolutionStrategy::from_mapping_strategy(
            ConceptMappingStrategy::CandidateScoring,
        );
        assert!(matches!(strategy, CatalogResolutionStrategy::Deterministic));
        assert_eq!(
            strategy.mapping_strategy(),
            ConceptMappingStrategy::CandidateScoring
        );
        assert!(strategy.agent_config().is_none());
    }

    #[test]
    fn maps_llm_reviewed_to_agent() {
        let strategy =
            CatalogResolutionStrategy::from_mapping_strategy(ConceptMappingStrategy::LlmReviewed);
        assert!(matches!(strategy, CatalogResolutionStrategy::Agent(_)));
        assert_eq!(
            strategy.mapping_strategy(),
            ConceptMappingStrategy::LlmReviewed
        );
        assert!(strategy.agent_config().is_some());
    }
}
