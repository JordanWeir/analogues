use crate::services::{
    report_artifacts::render_and_persist_report,
    workspace_store::{normalize_ticker, sqlite_uri, validate_date, WorkspaceStore, RUN_DB_FILENAME},
};
use chrono::{NaiveDate, Utc};
use loco_rs::prelude::*;
use sea_orm::Database;
use std::path::PathBuf;

const DEFAULT_REPORT_ROOT: &str = "reports/stock-narrative-research";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateReportRequest {
    pub ticker: String,
    pub date: String,
    pub index: Option<u32>,
    pub base_dir: PathBuf,
}

pub struct GenerateReport;

#[async_trait]
impl Task for GenerateReport {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "generateReport".to_string(),
            detail: "Compile scenario artifacts and render the stock narrative report".to_string(),
        }
    }

    async fn run(&self, _app_context: &AppContext, vars: &task::Vars) -> Result<()> {
        let request = GenerateReportRequest::from_vars(vars)?;
        let output = generate_report(&request).await?;

        println!("Generated report: {}", output.display());

        Ok(())
    }
}

impl GenerateReportRequest {
    pub fn from_vars(vars: &task::Vars) -> Result<Self> {
        let ticker = vars
            .cli
            .get("ticker")
            .or_else(|| vars.cli.get("symbol"))
            .map(String::as_str)
            .ok_or_else(|| {
                Error::string("generateReport requires ticker:<SYMBOL>, for example ticker:MSFT")
            })?;

        let date = vars
            .cli
            .get("date")
            .cloned()
            .unwrap_or_else(|| Utc::now().date_naive().format("%Y-%m-%d").to_string());
        validate_date(&date)?;

        let index = vars
            .cli
            .get("index")
            .map(|value| {
                value
                    .parse::<u32>()
                    .map_err(|_| Error::string("index must be a positive integer"))
                    .and_then(|index| {
                        if index == 0 {
                            Err(Error::string("index must be a positive integer"))
                        } else {
                            Ok(index)
                        }
                    })
            })
            .transpose()?;

        let base_dir = vars
            .cli
            .get("base_dir")
            .map_or_else(|| PathBuf::from(DEFAULT_REPORT_ROOT), PathBuf::from);

        Ok(Self {
            ticker: normalize_ticker(ticker)?,
            date,
            index,
            base_dir,
        })
    }
}

pub async fn generate_report(request: &GenerateReportRequest) -> Result<PathBuf> {
    let paths = resolve_workspace_paths(request)?;

    let db = Database::connect(sqlite_uri(&paths.sqlite_path))
        .await
        .map_err(|err| Error::string(&format!("failed to open run SQLite database: {err}")))?;

    let result = render_and_persist_report(
        &db,
        &paths.generated_dir,
        "generateReport",
    )
    .await;

    let close_result = db
        .close()
        .await
        .map_err(|err| Error::string(&format!("failed to close run SQLite database: {err}")));

    let report_path = result?;
    close_result?;
    Ok(report_path)
}

fn resolve_workspace_paths(request: &GenerateReportRequest) -> Result<crate::workspace::WorkspacePaths> {
    let paths = match request.index {
        Some(index) => {
            let workspace_dir = request
                .base_dir
                .join(format!("{}-{}-{}", request.ticker, request.date, index));
            crate::workspace::WorkspacePaths {
                run_slug: format!("{}-{}-{}", request.ticker, request.date, index),
                workspace_dir: workspace_dir.clone(),
                sqlite_path: workspace_dir.join(RUN_DB_FILENAME),
                generated_dir: workspace_dir.join("generated"),
            }
        }
        None => WorkspaceStore.resolve_latest(&request.base_dir, &request.ticker, &request.date)?,
    };

    if !paths.sqlite_path.is_file() {
        return Err(Error::string(&format!(
            "run SQLite database does not exist: {}",
            paths.sqlite_path.display()
        )));
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_date_accepts_iso_format() {
        assert!(NaiveDate::parse_from_str("2026-06-04", "%Y-%m-%d").is_ok());
    }
}
