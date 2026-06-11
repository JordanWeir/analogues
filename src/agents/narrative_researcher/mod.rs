mod agent;
mod config;
pub mod preamble;
pub mod research_workspace;
pub mod types;
pub mod validate;

pub use agent::{build_user_prompt, NarrativeResearcherAgent, NarrativeResearchRunResult};
pub use config::NarrativeResearcherConfig;
pub use preamble::AGENT_PREAMBLE;
pub use research_workspace::{narrative_research_golden_path, workspace_schema_hint};
pub use types::*;
pub use validate::{validate_workspace_ready, ValidationError};

pub const WORKER_NAME: &str = "narrative_researcher";
