use super::{read, support, NarrativeResearchStore};
use crate::{
    agents::narrative_researcher::{
        types::NarrativeWorkspaceSnapshot,
        validate::validate_workspace_ready,
    },
    services::workspace_sql::{execute_sql, scalar_i64, sql_quote},
};
use chrono::Utc;
use loco_rs::prelude::*;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct FinalizeOutcome {
    pub snapshot: NarrativeWorkspaceSnapshot,
}

impl FinalizeOutcome {
    pub fn into_response(self) -> Value {
        json!({
            "status": "complete",
            "workspace": self.snapshot,
        })
    }
}

impl<'a> NarrativeResearchStore<'a> {
    pub async fn finalize(&self) -> Result<FinalizeOutcome> {
        let board = read::load_board(self.db).await?;
        let snapshot = read::summarize_board(&board);

        validate_workspace_ready(
            snapshot.source_count,
            snapshot.claim_count,
            board.map.dominant.as_deref(),
            board.map.bull.as_deref(),
            board.map.bear.as_deref(),
            board.map.consensus.as_deref(),
            snapshot.crux_count,
            snapshot.orientation_captured,
            snapshot.sections_captured.iter().any(|k| k == "business_model"),
            snapshot.sections_captured.iter().any(|k| k == "why_now"),
        )
        .map_err(support::validation_error)?;

        let bull_claims = scalar_i64(
            self.db,
            "SELECT COUNT(*) AS count FROM claims WHERE side = 'bull'",
        )
        .await?;
        let bear_claims = scalar_i64(
            self.db,
            "SELECT COUNT(*) AS count FROM claims WHERE side = 'bear'",
        )
        .await?;
        if bull_claims == 0 || bear_claims == 0 {
            return Err(Error::string(
                "need at least one bull claim and one bear claim before finalize",
            ));
        }

        let now = Utc::now().to_rfc3339();
        for section_key in ["orientation", "business_model", "why_now", "narrative_map"] {
            execute_sql(
                self.db,
                &format!(
                    "UPDATE sections SET status = 'draft', updated_at = '{}'
                     WHERE section_key = '{}'",
                    sql_quote(&now),
                    sql_quote(section_key),
                ),
            )
            .await?;
        }

        Ok(FinalizeOutcome { snapshot })
    }
}
