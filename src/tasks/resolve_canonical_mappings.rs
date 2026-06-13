use crate::{
    services::{
        workspace_phases::{resolve_av_canonical_mappings_on_workspace, resolve_sqlite_path},
        workspace_store::WorkspaceStore,
    },
};
use loco_rs::prelude::*;

pub struct ResolveCanonicalMappings;

#[async_trait]
impl Task for ResolveCanonicalMappings {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "resolveCanonicalMappings".to_string(),
            detail: "Resolve deterministic Alpha Vantage canonical metric mappings (phase 3)"
                .to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let sqlite_path = resolve_sqlite_path(vars)?;
        let handle = WorkspaceStore.open_workspace(&sqlite_path).await?;
        let resolution = resolve_av_canonical_mappings_on_workspace(&handle).await?;
        handle.close().await?;

        println!(
            "Resolved {} Alpha Vantage canonical mappings for {}",
            resolution.mappings.len(),
            sqlite_path.display(),
        );

        Ok(())
    }
}
