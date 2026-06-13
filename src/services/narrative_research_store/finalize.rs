use super::{read, support, NarrativeResearchStore};
use crate::{
    agents::narrative_researcher::{
        types::{NarrativeWorkspaceSnapshot, OFFICIAL_SOURCE_TYPES, WORKSPACE_FILING_LAG_DAYS},
        validate::{validate_workspace_filing_lag_sources, validate_workspace_ready},
    },
    services::workspace_sql::{execute_sql, scalar_i64, sql_literal, sql_quote},
};
use chrono::{DateTime, NaiveDate, Utc};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
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

        validate_workspace_ready(
            snapshot.source_count,
            snapshot.claim_count,
            bull_claims,
            bear_claims,
            snapshot.agreement_count,
            board.map.dominant.as_deref(),
            board.map.bull.as_deref(),
            board.map.bear.as_deref(),
            board.map.consensus.as_deref(),
            snapshot.crux_count,
            snapshot.orientation_captured,
            snapshot
                .sections_captured
                .iter()
                .any(|k| k == "business_model"),
            snapshot.sections_captured.iter().any(|k| k == "why_now"),
        )
        .map_err(support::validation_error)?;

        let workspace_filing_lag = workspace_sec_filing_lag(self.db).await?;
        let official_source_count = count_official_sources(self.db).await?;
        validate_workspace_filing_lag_sources(workspace_filing_lag, official_source_count)
            .map_err(support::validation_error)?;

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

async fn workspace_sec_filing_lag(db: &impl ConnectionTrait) -> Result<bool> {
    let max_filed_at =
        scalar_opt_string(db, "SELECT MAX(filed_at) AS value FROM sec_raw_facts", "value")
            .await?;
    let run_created_at = scalar_opt_string(
        db,
        "SELECT created_at AS value FROM run_metadata WHERE id = 1",
        "value",
    )
    .await?;

    let (Some(max_filed_at), Some(run_created_at)) = (max_filed_at, run_created_at) else {
        return Ok(false);
    };

    let max_filed = parse_workspace_timestamp(&max_filed_at)?;
    let run_at = parse_workspace_timestamp(&run_created_at)?;
    let lag_days = (run_at - max_filed).num_days();
    Ok(lag_days > WORKSPACE_FILING_LAG_DAYS)
}

async fn scalar_opt_string(
    db: &impl ConnectionTrait,
    sql: &str,
    column: &str,
) -> Result<Option<String>> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            sql.to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))?;
    Ok(row.and_then(|row| row.try_get::<String>("", column).ok()))
}

async fn count_official_sources(db: &impl ConnectionTrait) -> Result<i64> {
    let types = OFFICIAL_SOURCE_TYPES
        .iter()
        .map(|value| sql_literal(Some(value)))
        .collect::<Vec<_>>()
        .join(", ");
    scalar_i64(
        db,
        &format!("SELECT COUNT(*) AS count FROM sources WHERE source_type IN ({types})"),
    )
    .await
}

fn parse_workspace_timestamp(value: &str) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }
    if let Ok(parsed) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(parsed
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| Error::string("invalid date timestamp"))?
            .and_utc());
    }
    Err(Error::string(&format!("unsupported timestamp format: {value}")))
}
