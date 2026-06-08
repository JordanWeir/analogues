use crate::services::{
    workspace_phases::{derive_starter_fundamentals_on_workspace, resolve_sqlite_path},
    workspace_store::WorkspaceStore,
};
use loco_rs::prelude::*;

pub struct DeriveStarterFundamentals;

#[async_trait]
impl Task for DeriveStarterFundamentals {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "deriveStarterFundamentals".to_string(),
            detail: "Derive starter fundamentals from active canonical mappings (phase 4)"
                .to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let sqlite_path = resolve_sqlite_path(vars)?;
        let handle = WorkspaceStore.open_workspace(&sqlite_path).await?;
        let run = derive_starter_fundamentals_on_workspace(&handle).await?;
        handle.close().await?;

        println!(
            "Derived starter fundamentals for {} ({} observations, gaps: {})",
            sqlite_path.display(),
            run.all_observations().len(),
            if run.gaps.is_empty() {
                "none".to_string()
            } else {
                run.gaps.join(", ")
            },
        );

        Ok(())
    }
}
