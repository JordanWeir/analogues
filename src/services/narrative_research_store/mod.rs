mod board;
mod capture;
mod finalize;
mod read;
mod support;

use loco_rs::prelude::*;
use sea_orm::{Database, DatabaseConnection};
use std::path::Path;

pub use finalize::FinalizeOutcome;

pub struct NarrativeResearchStore<'a> {
    pub(super) db: &'a DatabaseConnection,
}

impl<'a> NarrativeResearchStore<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn connect(path: &Path) -> Result<DatabaseConnection> {
        Database::connect(crate::services::workspace_store::sqlite_uri(path))
            .await
            .map_err(|err| Error::string(&format!("failed to open workspace sqlite: {err}")))
    }
}
