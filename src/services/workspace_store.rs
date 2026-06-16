use crate::workspace::{
    seed_database, InitWorkspaceRequest, WorkspacePaths, SCHEMA_MIGRATION_STATEMENTS,
    SCHEMA_STATEMENTS,
};
use chrono::NaiveDate;
use loco_rs::prelude::*;
use sea_orm::{Database, DatabaseBackend, Statement};
use std::{fs, path::Path};

pub struct WorkspaceHandle {
    pub paths: WorkspacePaths,
    pub schema_version: i64,
    db: sea_orm::DatabaseConnection,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceStore;

pub const DEFAULT_REPORT_ROOT: &str = "reports/stock-narrative-research";
pub const RUN_DB_FILENAME: &str = "run.sqlite";
pub const SCHEMA_VERSION: i64 = 4;

pub async fn execute_schema(db: &sea_orm::DatabaseConnection) -> Result<()> {
    for statement in SCHEMA_STATEMENTS {
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            (*statement).to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("failed to apply run schema: {err}")))?;
    }

    for statement in SCHEMA_MIGRATION_STATEMENTS {
        if let Err(err) = db
            .execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                (*statement).to_string(),
            ))
            .await
        {
            let message = err.to_string();
            if !message.contains("duplicate column name") {
                return Err(Error::string(&format!(
                    "failed to apply schema migration: {message}"
                )));
            }
        }
    }

    Ok(())
}

pub fn normalize_ticker(raw: &str) -> Result<String> {
    let ticker = raw.trim().to_uppercase();
    if ticker.is_empty() {
        return Err(Error::string("ticker cannot be empty"));
    }

    let valid = ticker
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));

    if !valid {
        return Err(Error::string(
            "ticker can only contain ASCII letters, numbers, dots, and hyphens",
        ));
    }

    Ok(ticker)
}

pub fn validate_date(date: &str) -> Result<()> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| Error::string("date must use YYYY-MM-DD format, for example date:2026-06-04"))
}

pub fn sqlite_uri(path: &Path) -> String {
    let normalized_path = path.to_string_lossy().replace('\\', "/");
    format!("sqlite://{normalized_path}?mode=rwc")
}

impl WorkspaceHandle {
    pub fn connection(&self) -> &sea_orm::DatabaseConnection {
        &self.db
    }

    pub async fn close(self) -> Result<()> {
        self.db
            .close()
            .await
            .map_err(|err| Error::string(&format!("failed to close run SQLite database: {err}")))
    }
}

impl WorkspaceStore {
    pub async fn open_workspace(&self, sqlite_path: &Path) -> Result<WorkspaceHandle> {
        if !sqlite_path.is_file() {
            return Err(Error::string(&format!(
                "run SQLite database does not exist: {}",
                sqlite_path.display()
            )));
        }

        let workspace_dir = sqlite_path.parent().ok_or_else(|| {
            Error::string(&format!(
                "workspace path has no parent directory: {}",
                sqlite_path.display()
            ))
        })?;
        let run_slug = workspace_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                Error::string(&format!(
                    "workspace directory has no name: {}",
                    workspace_dir.display()
                ))
            })?
            .to_string();
        let paths = WorkspacePaths {
            run_slug,
            workspace_dir: workspace_dir.to_path_buf(),
            sqlite_path: sqlite_path.to_path_buf(),
            generated_dir: workspace_dir.join("generated"),
        };

        let db = Database::connect(sqlite_uri(&paths.sqlite_path))
            .await
            .map_err(|err| Error::string(&format!("failed to open run SQLite database: {err}")))?;

        Ok(WorkspaceHandle {
            paths,
            schema_version: SCHEMA_VERSION,
            db,
        })
    }

    pub async fn create_workspace(
        &self,
        request: &InitWorkspaceRequest,
    ) -> Result<WorkspaceHandle> {
        fs::create_dir_all(&request.base_dir).map_err(|err| {
            Error::string(&format!(
                "failed to create report root {}: {err}",
                request.base_dir.display()
            ))
        })?;

        let paths = next_workspace_paths(request)?;
        fs::create_dir(&paths.workspace_dir).map_err(|err| {
            Error::string(&format!(
                "failed to create workspace {}: {err}",
                paths.workspace_dir.display()
            ))
        })?;
        ensure_generated_dir(&paths)?;

        let db = Database::connect(sqlite_uri(&paths.sqlite_path))
            .await
            .map_err(|err| Error::string(&format!("failed to open run SQLite database: {err}")))?;

        let setup_result = async {
            self.apply_schema(&db).await?;
            self.seed_workspace(&db, request, &paths).await
        }
        .await;
        if let Err(err) = setup_result {
            let _ = db.close().await;
            return Err(err);
        }

        Ok(WorkspaceHandle {
            paths,
            schema_version: SCHEMA_VERSION,
            db,
        })
    }

    pub fn resolve_latest(
        &self,
        base_dir: &Path,
        ticker: &str,
        date: &str,
    ) -> Result<WorkspacePaths> {
        validate_date(date)?;
        let ticker = normalize_ticker(ticker)?;
        let prefix = format!("{ticker}-{date}-");
        let mut latest: Option<(u32, WorkspacePaths)> = None;

        for entry in fs::read_dir(base_dir).map_err(|err| {
            Error::string(&format!(
                "failed to read workspace root {}: {err}",
                base_dir.display()
            ))
        })? {
            let entry = entry.map_err(|err| {
                Error::string(&format!(
                    "failed to read workspace entry under {}: {err}",
                    base_dir.display()
                ))
            })?;
            let file_name = entry.file_name();
            let Some(run_slug) = file_name.to_str() else {
                continue;
            };
            let Some(index) = run_slug
                .strip_prefix(&prefix)
                .and_then(|suffix| suffix.parse::<u32>().ok())
            else {
                continue;
            };
            let workspace_dir = entry.path();
            if !workspace_dir.is_dir() {
                continue;
            }
            let paths = WorkspacePaths {
                run_slug: run_slug.to_string(),
                sqlite_path: workspace_dir.join(RUN_DB_FILENAME),
                generated_dir: workspace_dir.join("generated"),
                workspace_dir,
            };
            if latest
                .as_ref()
                .is_none_or(|(latest_index, _)| index > *latest_index)
            {
                latest = Some((index, paths));
            }
        }

        latest
            .map(|(_, paths)| paths)
            .ok_or_else(|| Error::string(&format!("no workspace found for {ticker} on {date}")))
    }

    async fn apply_schema(&self, db: &sea_orm::DatabaseConnection) -> Result<()> {
        execute_schema(db).await
    }

    async fn seed_workspace(
        &self,
        db: &sea_orm::DatabaseConnection,
        request: &InitWorkspaceRequest,
        paths: &WorkspacePaths,
    ) -> Result<()> {
        seed_database(db, request, paths).await
    }
}

fn ensure_generated_dir(paths: &WorkspacePaths) -> Result<()> {
    fs::create_dir_all(&paths.generated_dir).map_err(|err| {
        Error::string(&format!(
            "failed to create generated directory {}: {err}",
            paths.generated_dir.display()
        ))
    })
}

fn next_workspace_paths(request: &InitWorkspaceRequest) -> Result<WorkspacePaths> {
    for index in 1..10_000 {
        let run_slug = format!("{}-{}-{}", request.ticker, request.date, index);
        let workspace_dir = request.base_dir.join(&run_slug);
        if !workspace_dir.exists() {
            let sqlite_path = workspace_dir.join(RUN_DB_FILENAME);
            let generated_dir = workspace_dir.join("generated");
            return Ok(WorkspacePaths {
                run_slug,
                workspace_dir,
                sqlite_path,
                generated_dir,
            });
        }
    }

    Err(Error::string(&format!(
        "could not allocate a workspace for {} on {}",
        request.ticker, request.date
    )))
}
