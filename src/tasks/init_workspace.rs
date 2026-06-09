use crate::{
    lanes::{
        context::{LaneConfig, LaneContext},
        init_pipeline::lanes_for_request,
        runner::LinearRunner,
    },
    services::workspace_store::{
        normalize_ticker, validate_date, WorkspaceStore, DEFAULT_REPORT_ROOT,
    },
    workspace::WorkspacePaths,
};
use chrono::Utc;
use loco_rs::prelude::*;
use std::path::PathBuf;

pub use crate::{
    services::canonical_mapping::ConceptMappingStrategy, workspace::InitWorkspaceRequest,
};

pub struct InitWorkspace;

#[async_trait]
impl Task for InitWorkspace {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "initWorkspace".to_string(),
            detail: "Initialize a stock research workspace and run SQLite database".to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let request = InitWorkspaceRequest::from_vars(vars)?;
        let paths = initialize_workspace(&request).await?;

        println!(
            "Created stock research workspace: {}",
            paths.workspace_dir.display()
        );
        println!("Initialized run database: {}", paths.sqlite_path.display());

        Ok(())
    }
}

impl InitWorkspaceRequest {
    pub fn from_vars(vars: &task::Vars) -> Result<Self> {
        let ticker = vars
            .cli
            .get("ticker")
            .or_else(|| vars.cli.get("symbol"))
            .map(String::as_str)
            .ok_or_else(|| {
                Error::string("initWorkspace requires ticker:<SYMBOL>, for example ticker:MSFT")
            })?;

        let date = vars
            .cli
            .get("date")
            .cloned()
            .unwrap_or_else(|| Utc::now().date_naive().format("%Y-%m-%d").to_string());

        validate_date(&date)?;

        let base_dir = vars
            .cli
            .get("base_dir")
            .map_or_else(|| PathBuf::from(DEFAULT_REPORT_ROOT), PathBuf::from);
        let fetch_financials = vars
            .cli
            .get("fetch_financials")
            .map(|value| !matches!(value.as_str(), "false" | "0" | "no" | "skip"))
            .unwrap_or(true);
        let mapping_strategy = vars
            .cli
            .get("mapping_strategy")
            .or_else(|| vars.cli.get("concept_mapping_strategy"))
            .map_or(
                Ok(Some(ConceptMappingStrategy::CandidateScoring)),
                |value| crate::services::canonical_mapping::ConceptMappingStrategy::from_var(value),
            )?;

        Ok(Self {
            ticker: normalize_ticker(ticker)?,
            date,
            base_dir,
            fetch_financials,
            mapping_strategy,
        })
    }
}

pub async fn initialize_workspace(request: &InitWorkspaceRequest) -> Result<WorkspacePaths> {
    let normalized_request = request.normalized()?;
    let store = WorkspaceStore;
    let handle = store.create_workspace(&normalized_request).await?;
    let mut ctx = LaneContext::new(handle, LaneConfig::new(&normalized_request.ticker));

    let report = LinearRunner::new(lanes_for_request(&normalized_request))
        .run(&mut ctx)
        .await?;

    if report.stopped_early {
        let reason = report
            .stop_reason
            .unwrap_or_else(|| "linear research pipeline stopped early".to_string());
        let _ = ctx.workspace.close().await;
        return Err(Error::string(&reason));
    }

    let paths = ctx.workspace.paths.clone();
    ctx.workspace.close().await?;

    Ok(paths)
}
