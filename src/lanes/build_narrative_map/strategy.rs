use crate::agents::narrative_researcher::NarrativeResearcherConfig;

#[derive(Debug, Clone)]
pub enum NarrativeMapStrategy {
    Agent(NarrativeResearcherConfig),
    #[cfg(test)]
    Fixture,
}

impl NarrativeMapStrategy {
    pub fn agent_defaults(_workspace_sqlite: std::path::PathBuf) -> Self {
        Self::Agent(NarrativeResearcherConfig::default())
    }
}
