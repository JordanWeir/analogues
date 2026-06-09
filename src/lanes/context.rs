use crate::services::workspace_store::WorkspaceHandle;
use std::collections::BTreeMap;

/// Shared execution context passed to every lane in a linear research run.
///
/// Lanes orchestrate work against the workspace SQLite database; deterministic
/// domain logic stays in `src/services/`.
pub struct LaneContext {
    pub workspace: WorkspaceHandle,
    pub config: LaneConfig,
}

impl LaneContext {
    pub fn new(workspace: WorkspaceHandle, config: LaneConfig) -> Self {
        Self { workspace, config }
    }

    pub fn run_slug(&self) -> &str {
        &self.workspace.paths.run_slug
    }

    pub fn ticker(&self) -> &str {
        &self.config.ticker
    }
}

/// Per-run configuration carried through the linear pipeline.
#[derive(Debug, Clone, Default)]
pub struct LaneConfig {
    pub ticker: String,
    #[allow(dead_code)]
    pub extra: BTreeMap<String, String>,
}

impl LaneConfig {
    pub fn new(ticker: impl Into<String>) -> Self {
        Self {
            ticker: ticker.into(),
            extra: BTreeMap::new(),
        }
    }
}
