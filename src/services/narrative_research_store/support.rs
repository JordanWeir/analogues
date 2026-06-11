use super::read::{load_narrative_item_bodies, load_narrative_map_fields};
use crate::{
    agents::narrative_researcher::{
        types::CaptureClaimInput,
        validate::ValidationError,
    },
    services::workspace_sql::{execute_sql, scalar_i64, sql_literal, sql_quote},
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

pub(crate) fn validation_error(err: ValidationError) -> Error {
    Error::string(&err.to_string())
}

pub(crate) fn narrative_side_column(side: &str) -> Result<&'static str> {
    match side {
        "dominant" => Ok("dominant"),
        "bull" => Ok("bull"),
        "bear" => Ok("bear"),
        "consensus" => Ok("consensus"),
        "counter_narrative" => Ok("counter_narrative"),
        other => Err(Error::string(&format!("unknown narrative side: {other}"))),
    }
}

pub(crate) async fn ensure_narrative_map_row(db: &impl ConnectionTrait) -> Result<()> {
    let count = scalar_i64(db, "SELECT COUNT(*) AS count FROM narrative_map").await?;
    if count == 0 {
        execute_sql(db, "INSERT INTO narrative_map (id) VALUES (1)").await?;
    }
    Ok(())
}

pub(crate) async fn upsert_section_body(
    db: &impl ConnectionTrait,
    section_key: &str,
    title: Option<&str>,
    body: &str,
    status: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    execute_sql(
        db,
        &format!(
            "UPDATE sections SET status = '{}', title = {}, body = '{}', updated_at = '{}'
             WHERE section_key = '{}'",
            sql_quote(status),
            sql_literal(title),
            sql_quote(body),
            sql_quote(&now),
            sql_quote(section_key),
        ),
    )
    .await?;
    Ok(())
}

pub(crate) async fn sync_narrative_map_section(db: &impl ConnectionTrait) -> Result<()> {
    let map = load_narrative_map_fields(db).await?;
    let agreements = load_narrative_item_bodies(db, "agreement").await?;
    let cruxes = load_narrative_item_bodies(db, "crux").await?;

    let body = serde_json::json!({
        "dominant": map.dominant.unwrap_or_default(),
        "bull": map.bull.unwrap_or_default(),
        "bear": map.bear.unwrap_or_default(),
        "consensus": map.consensus.unwrap_or_default(),
        "counter_narrative": map.counter_narrative.unwrap_or_default(),
        "agreements": agreements,
        "cruxes": cruxes,
    });
    upsert_section_body(db, "narrative_map", None, &body.to_string(), "draft").await
}

pub(crate) async fn find_existing_source_id(
    db: &impl ConnectionTrait,
    source: &crate::agents::narrative_researcher::types::CaptureSourceInput,
) -> Result<Option<i64>> {
    if let Some(url) = source.url.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        let row = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT id FROM sources WHERE TRIM(url) = '{}' ORDER BY id LIMIT 1",
                    sql_quote(url),
                ),
            ))
            .await
            .map_err(|err| Error::string(&format!("find source by url failed: {err}")))?;
        if let Some(row) = row {
            return row
                .try_get::<i64>("", "id")
                .map(Some)
                .map_err(|err| Error::string(&format!("parse source id: {err}")));
        }
    }

    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT id FROM sources WHERE title = '{}' ORDER BY id LIMIT 1",
                sql_quote(source.title.trim()),
            ),
        ))
        .await
        .map_err(|err| Error::string(&format!("find source by title failed: {err}")))?;
    row.map(|row| row.try_get::<i64>("", "id"))
        .transpose()
        .map_err(|err| Error::string(&format!("parse source id: {err}")))
}

pub(crate) async fn claim_already_exists(
    db: &impl ConnectionTrait,
    claim: &str,
    source_id: Option<i64>,
) -> Result<bool> {
    let count = if let Some(source_id) = source_id {
        scalar_i64(
            db,
            &format!(
                "SELECT COUNT(*) AS count FROM claims
                 WHERE claim = '{}' AND source_id = {}",
                sql_quote(claim.trim()),
                source_id,
            ),
        )
        .await?
    } else {
        scalar_i64(
            db,
            &format!(
                "SELECT COUNT(*) AS count FROM claims
                 WHERE claim = '{}' AND source_id IS NULL",
                sql_quote(claim.trim()),
            ),
        )
        .await?
    };
    Ok(count > 0)
}

pub(crate) async fn narrative_item_exists(
    db: &impl ConnectionTrait,
    item_type: &str,
    body: &str,
) -> Result<bool> {
    let count = scalar_i64(
        db,
        &format!(
            "SELECT COUNT(*) AS count FROM narrative_map_items
             WHERE item_type = '{}' AND body = '{}'",
            sql_quote(item_type),
            sql_quote(body.trim()),
        ),
    )
    .await?;
    Ok(count > 0)
}

pub(crate) async fn resolve_source_id(
    db: &impl ConnectionTrait,
    claim: &CaptureClaimInput,
) -> Result<Option<i64>> {
    if let Some(id) = claim.source_id {
        return Ok(Some(id));
    }
    let title = claim
        .source_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(title) = title else {
        return Ok(None);
    };
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT id FROM sources WHERE title = '{}' ORDER BY id DESC LIMIT 1",
                sql_quote(title),
            ),
        ))
        .await
        .map_err(|err| Error::string(&format!("resolve source_id failed: {err}")))?;
    row.map(|row| row.try_get::<i64>("", "id"))
        .transpose()
        .map_err(|err| Error::string(&format!("parse source id: {err}")))
}

