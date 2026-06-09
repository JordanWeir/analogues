use crate::services::narrative_research::NarrativeResearchService;

#[derive(Debug, Clone)]
pub enum NarrativeMapStrategy {
    Agent(NarrativeResearchService),
    #[cfg(test)]
    Fixture,
}

impl NarrativeMapStrategy {
    pub fn agent_defaults(workspace_sqlite: std::path::PathBuf) -> Self {
        Self::Agent(NarrativeResearchService::agent_defaults(workspace_sqlite))
    }
}
