use crate::services::{
    workspace_phases::{
        mapping_strategy_from_vars, resolve_canonical_mappings_on_workspace, resolve_sqlite_path,
    },
    workspace_store::WorkspaceStore,
};
use loco_rs::prelude::*;

pub struct ResolveCanonicalMappings;

#[async_trait]
impl Task for ResolveCanonicalMappings {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "resolveCanonicalMappings".to_string(),
            detail: "Resolve canonical metric mappings against an existing workspace (phase 3)"
                .to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let sqlite_path = resolve_sqlite_path(vars)?;
        let strategy = mapping_strategy_from_vars(vars)?;
        let handle = WorkspaceStore.open_workspace(&sqlite_path).await?;
        let resolution = resolve_canonical_mappings_on_workspace(&handle, strategy).await?;
        handle.close().await?;

        println!(
            "Resolved {} canonical mappings for {} using {}",
            resolution.mappings.len(),
            sqlite_path.display(),
            resolution.strategy_id,
        );

        Ok(())
    }
}
