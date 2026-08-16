use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::{fs, path::Path};

pub struct WorkspaceCheckpointStore;

impl WorkspaceCheckpointStore {
    /// Copy the open workspace database into `checkpoints/{lane_name}.sqlite`.
    pub async fn save_lane_checkpoint(
        db: &sea_orm::DatabaseConnection,
        checkpoints_dir: &Path,
        lane_name: &str,
    ) -> Result<()> {
        fs::create_dir_all(checkpoints_dir).map_err(|err| {
            Error::string(&format!(
                "failed to create checkpoints directory {}: {err}",
                checkpoints_dir.display()
            ))
        })?;

        let dest = checkpoints_dir.join(format!("{lane_name}.sqlite"));
        if dest.exists() {
            fs::remove_file(&dest).map_err(|err| {
                Error::string(&format!(
                    "failed to replace existing checkpoint {}: {err}",
                    dest.display()
                ))
            })?;
        }

        let escaped = dest.to_string_lossy().replace('\'', "''");
        db.execute(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("VACUUM INTO '{escaped}'"),
        ))
        .await
        .map_err(|err| {
            Error::string(&format!(
                "failed to save checkpoint for lane {lane_name}: {err}"
            ))
        })?;

        Ok(())
    }
}
