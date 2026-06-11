use crate::services::workspace_sql::{
    execute_sql, last_insert_rowid, scalar_i64, sql_literal, sql_quote,
};
use crate::agents::narrative_researcher::{
    types::{
        CaptureClaimInput, CaptureNarrativeItemsInput, CaptureNarrativeSideInput,
        CaptureOrientationInput, CaptureResearchGapInput, CaptureSectionInput, CaptureSourceInput,
        NarrativeWorkspaceSnapshot,
    },
    validate::{
        validate_claim, validate_narrative_items, validate_narrative_side, validate_orientation,
        validate_research_gap, validate_section, validate_source, validate_workspace_ready,
        ValidationError,
    },
};
use chrono::Utc;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use serde_json::json;
use std::path::Path;

pub struct NarrativeResearchStore;

impl NarrativeResearchStore {
    pub async fn connect(path: &Path) -> Result<DatabaseConnection> {
        Database::connect(crate::services::workspace_store::sqlite_uri(path))
            .await
            .map_err(|err| Error::string(&format!("failed to open workspace sqlite: {err}")))
    }

    /// Load durable narrative state already on the board for agent context (no deletes).
    pub async fn load_existing_context(db: &impl ConnectionTrait) -> Result<serde_json::Value> {
        let sources = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, title, url, source_type, published_at, why_it_matters, notes
                 FROM sources ORDER BY id"
                    .to_string(),
            ))
            .await
            .map_err(|err| Error::string(&format!("load sources failed: {err}")))?;
        let source_rows: Vec<serde_json::Value> = sources
            .into_iter()
            .map(|row| {
                Ok(json!({
                    "id": row.try_get::<i64>("", "id")?,
                    "title": row.try_get::<String>("", "title").ok(),
                    "url": row.try_get::<String>("", "url").ok(),
                    "source_type": row.try_get::<String>("", "source_type").ok(),
                    "published_at": row.try_get::<String>("", "published_at").ok(),
                    "why_it_matters": row.try_get::<String>("", "why_it_matters").ok(),
                    "notes": row.try_get::<String>("", "notes").ok(),
                }))
            })
            .collect::<Result<_>>()?;

        let claims = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, claim, source_id, claim_type, side, confidence, metric, notes
                 FROM claims ORDER BY id"
                    .to_string(),
            ))
            .await
            .map_err(|err| Error::string(&format!("load claims failed: {err}")))?;
        let claim_rows: Vec<serde_json::Value> = claims
            .into_iter()
            .map(|row| {
                Ok(json!({
                    "id": row.try_get::<i64>("", "id")?,
                    "claim": row.try_get::<String>("", "claim")?,
                    "source_id": row.try_get::<i64>("", "source_id").ok(),
                    "claim_type": row.try_get::<String>("", "claim_type").ok(),
                    "side": row.try_get::<String>("", "side").ok(),
                    "confidence": row.try_get::<String>("", "confidence").ok(),
                    "metric": row.try_get::<String>("", "metric").ok(),
                    "notes": row.try_get::<String>("", "notes").ok(),
                }))
            })
            .collect::<Result<_>>()?;

        let map = load_narrative_map_fields(db).await?;
        let item_rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, item_type, item_order, body FROM narrative_map_items ORDER BY item_type, item_order"
                    .to_string(),
            ))
            .await
            .map_err(|err| Error::string(&format!("load narrative items failed: {err}")))?;
        let mut agreements = Vec::new();
        let mut cruxes = Vec::new();
        for row in item_rows {
            let item = json!({
                "id": row.try_get::<i64>("", "id")?,
                "item_order": row.try_get::<i64>("", "item_order")?,
                "body": row.try_get::<String>("", "body")?,
            });
            match row.try_get::<String>("", "item_type").ok().as_deref() {
                Some("agreement") => agreements.push(item),
                Some("crux") => cruxes.push(item),
                _ => {}
            }
        }

        let mut sections = serde_json::Map::new();
        for section_key in ["orientation", "business_model", "why_now", "narrative_map"] {
            let row = db
                .query_one(Statement::from_string(
                    DatabaseBackend::Sqlite,
                    format!(
                        "SELECT status, title, body FROM sections WHERE section_key = '{}'",
                        sql_quote(section_key),
                    ),
                ))
                .await
                .map_err(|err| Error::string(&format!("load section {section_key} failed: {err}")))?;
            if let Some(row) = row {
                sections.insert(
                    section_key.to_string(),
                    json!({
                        "status": row.try_get::<String>("", "status").ok(),
                        "title": row.try_get::<String>("", "title").ok(),
                        "body": row.try_get::<String>("", "body").ok(),
                    }),
                );
            }
        }

        let gaps = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT gap_key, description, status FROM data_gaps
                 WHERE gap_key LIKE 'narrative_%' ORDER BY gap_key"
                    .to_string(),
            ))
            .await
            .map_err(|err| Error::string(&format!("load narrative gaps failed: {err}")))?;
        let gap_rows: Vec<serde_json::Value> = gaps
            .into_iter()
            .map(|row| {
                Ok(json!({
                    "gap_key": row.try_get::<String>("", "gap_key")?,
                    "description": row.try_get::<String>("", "description")?,
                    "status": row.try_get::<String>("", "status").ok(),
                }))
            })
            .collect::<Result<_>>()?;

        Ok(json!({
            "sources": source_rows,
            "claims": claim_rows,
            "narrative_map": {
                "dominant": map.dominant,
                "bull": map.bull,
                "bear": map.bear,
                "consensus": map.consensus,
                "counter_narrative": map.counter_narrative,
            },
            "narrative_map_items": {
                "agreements": agreements,
                "cruxes": cruxes,
            },
            "sections": sections,
            "research_gaps": gap_rows,
        }))
    }

    pub async fn capture_sources(
        db: &impl ConnectionTrait,
        sources: Vec<CaptureSourceInput>,
    ) -> Result<serde_json::Value> {
        if sources.is_empty() {
            return Err(Error::string("capture_sources requires at least one source"));
        }
        for source in &sources {
            validate_source(source).map_err(validation_error)?;
        }

        let now = Utc::now().to_rfc3339();
        let mut captured = Vec::new();
        for source in sources {
            if let Some(existing_id) = find_existing_source_id(db, &source).await? {
                captured.push(json!({
                    "id": existing_id,
                    "title": source.title,
                    "status": "already_exists",
                }));
                continue;
            }

            execute_sql(
                db,
                &format!(
                    "INSERT INTO sources (title, url, source_type, published_at, accessed_at, why_it_matters, notes)
                     VALUES ('{}', {}, {}, {}, {}, '{}', {})",
                    sql_quote(&source.title),
                    sql_literal(source.url.as_deref()),
                    sql_literal(Some(&source.source_type)),
                    sql_literal(source.published_at.as_deref()),
                    sql_literal(source.accessed_at.as_deref().or(Some(now.as_str()))),
                    sql_quote(&source.why_it_matters),
                    sql_literal(source.notes.as_deref()),
                ),
            )
            .await?;
            let id = last_insert_rowid(db).await?;
            captured.push(json!({
                "id": id,
                "title": source.title,
                "status": "inserted",
            }));
        }

        let snapshot = Self::snapshot(db).await?;
        Ok(json!({
            "captured": captured,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_claims(
        db: &impl ConnectionTrait,
        claims: Vec<CaptureClaimInput>,
    ) -> Result<serde_json::Value> {
        if claims.is_empty() {
            return Err(Error::string("capture_claims requires at least one claim"));
        }

        let mut inserted = 0_i64;
        let mut skipped = 0_i64;
        for claim in claims {
            if claim.confidence == "inference" {
                validate_claim_relaxed(&claim)?;
            } else {
                validate_claim(&claim).map_err(validation_error)?;
            }
            let source_id = resolve_source_id(db, &claim).await?;
            if claim_already_exists(db, &claim.claim, source_id).await? {
                skipped += 1;
                continue;
            }
            execute_sql(
                db,
                &format!(
                    "INSERT INTO claims (claim, source_id, claim_type, side, confidence, metric, notes)
                     VALUES ('{}', {}, {}, {}, {}, {}, {})",
                    sql_quote(&claim.claim),
                    source_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "NULL".to_string()),
                    sql_literal(Some(&claim.claim_type)),
                    sql_literal(Some(&claim.side)),
                    sql_literal(Some(&claim.confidence)),
                    sql_literal(claim.metric.as_deref()),
                    sql_literal(claim.notes.as_deref()),
                ),
            )
            .await?;
            inserted += 1;
        }

        let snapshot = Self::snapshot(db).await?;
        Ok(json!({
            "claims_added": inserted,
            "claims_skipped_duplicate": skipped,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_narrative_side(
        db: &impl ConnectionTrait,
        input: CaptureNarrativeSideInput,
    ) -> Result<serde_json::Value> {
        validate_narrative_side(&input).map_err(validation_error)?;
        ensure_narrative_map_row(db).await?;

        let column = match input.side.as_str() {
            "dominant" => "dominant",
            "bull" => "bull",
            "bear" => "bear",
            "consensus" => "consensus",
            "counter_narrative" => "counter_narrative",
            other => {
                return Err(Error::string(&format!("unknown narrative side: {other}")));
            }
        };

        execute_sql(
            db,
            &format!(
                "UPDATE narrative_map SET {column} = '{}' WHERE id = 1",
                sql_quote(&input.body),
            ),
        )
        .await?;

        Self::sync_narrative_map_section(db).await?;
        let snapshot = Self::snapshot(db).await?;
        Ok(json!({
            "side": input.side,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_narrative_items(
        db: &impl ConnectionTrait,
        input: CaptureNarrativeItemsInput,
    ) -> Result<serde_json::Value> {
        validate_narrative_items(&input).map_err(validation_error)?;
        ensure_narrative_map_row(db).await?;

        let start_order = scalar_i64(
            db,
            &format!(
                "SELECT COALESCE(MAX(item_order), 0) AS count FROM narrative_map_items WHERE item_type = '{}'",
                sql_quote(&input.item_type),
            ),
        )
        .await?;

        let mut items_added = 0_usize;
        let mut items_skipped = 0_usize;
        let mut next_order = start_order;
        for item in &input.items {
            if narrative_item_exists(db, &input.item_type, item).await? {
                items_skipped += 1;
                continue;
            }
            next_order += 1;
            execute_sql(
                db,
                &format!(
                    "INSERT INTO narrative_map_items (item_type, item_order, body)
                     VALUES ('{}', {}, '{}')",
                    sql_quote(&input.item_type),
                    next_order,
                    sql_quote(item),
                ),
            )
            .await?;
            items_added += 1;
        }

        Self::sync_narrative_map_section(db).await?;
        let snapshot = Self::snapshot(db).await?;
        Ok(json!({
            "item_type": input.item_type,
            "items_added": items_added,
            "items_skipped_duplicate": items_skipped,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_orientation(
        db: &impl ConnectionTrait,
        input: CaptureOrientationInput,
    ) -> Result<serde_json::Value> {
        validate_orientation(&input).map_err(validation_error)?;
        let body = json!({
            "dominant_question": input.dominant_question,
            "current_setup": input.current_setup,
            "time_horizon": input.time_horizon,
            "base_rate_warning": input.base_rate_warning,
        });
        Self::upsert_section_body(db, "orientation", None, &body.to_string(), "draft").await?;
        let snapshot = Self::snapshot(db).await?;
        Ok(json!({ "workspace": snapshot }))
    }

    pub async fn capture_section(
        db: &impl ConnectionTrait,
        input: CaptureSectionInput,
    ) -> Result<serde_json::Value> {
        validate_section(&input).map_err(validation_error)?;
        Self::upsert_section_body(
            db,
            &input.section_key,
            input.title.as_deref(),
            &input.body,
            "draft",
        )
        .await?;
        let snapshot = Self::snapshot(db).await?;
        Ok(json!({
            "section_key": input.section_key,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_research_gap(
        db: &impl ConnectionTrait,
        input: CaptureResearchGapInput,
    ) -> Result<serde_json::Value> {
        validate_research_gap(&input).map_err(validation_error)?;
        let now = Utc::now().to_rfc3339();
        let gap_key = format!("narrative_{}", input.gap_key.trim());
        execute_sql(
            db,
            &format!(
                "INSERT INTO data_gaps (gap_key, description, status, created_at)
                 VALUES ('{}', '{}', 'open', '{}')
                 ON CONFLICT(gap_key) DO UPDATE SET
                    description = excluded.description,
                    status = 'open'",
                sql_quote(&gap_key),
                sql_quote(&input.description),
                sql_quote(&now),
            ),
        )
        .await?;
        let snapshot = Self::snapshot(db).await?;
        Ok(json!({ "gap_key": gap_key, "workspace": snapshot }))
    }

    pub async fn finalize(db: &impl ConnectionTrait) -> Result<serde_json::Value> {
        let snapshot = Self::snapshot(db).await?;
        let map = load_narrative_map_fields(db).await?;
        validate_workspace_ready(
            snapshot.source_count,
            snapshot.claim_count,
            map.dominant.as_deref(),
            map.bull.as_deref(),
            map.bear.as_deref(),
            map.consensus.as_deref(),
            snapshot.crux_count,
            snapshot.orientation_captured,
            snapshot.sections_captured.iter().any(|k| k == "business_model"),
            snapshot.sections_captured.iter().any(|k| k == "why_now"),
        )
        .map_err(validation_error)?;

        let bull_claims = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM claims WHERE side = 'bull'",
        )
        .await?;
        let bear_claims = scalar_i64(
            db,
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
                db,
                &format!(
                    "UPDATE sections SET status = 'draft', updated_at = '{}'
                     WHERE section_key = '{}'",
                    sql_quote(&now),
                    sql_quote(section_key),
                ),
            )
            .await?;
        }

        Ok(json!({
            "status": "complete",
            "workspace": snapshot,
        }))
    }

    pub async fn snapshot(db: &impl ConnectionTrait) -> Result<NarrativeWorkspaceSnapshot> {
        let source_count = scalar_i64(db, "SELECT COUNT(*) AS count FROM sources").await?;
        let claim_count = scalar_i64(db, "SELECT COUNT(*) AS count FROM claims").await?;
        let agreement_count = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM narrative_map_items WHERE item_type = 'agreement'",
        )
        .await?;
        let crux_count = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM narrative_map_items WHERE item_type = 'crux'",
        )
        .await?;
        let research_gap_count = scalar_i64(
            db,
            "SELECT COUNT(*) AS count FROM data_gaps WHERE gap_key LIKE 'narrative_%'",
        )
        .await?;

        let map = load_narrative_map_fields(db).await?;
        let mut narrative_sides_captured = Vec::new();
        for (side, value) in [
            ("dominant", map.dominant.as_deref()),
            ("bull", map.bull.as_deref()),
            ("bear", map.bear.as_deref()),
            ("consensus", map.consensus.as_deref()),
            ("counter_narrative", map.counter_narrative.as_deref()),
        ] {
            if value.map(str::trim).is_some_and(|text| !text.is_empty()) {
                narrative_sides_captured.push(side.to_string());
            }
        }

        let orientation_captured = section_has_body(db, "orientation").await?;
        let mut sections_captured = Vec::new();
        for key in ["business_model", "why_now"] {
            if section_has_body(db, key).await? {
                sections_captured.push(key.to_string());
            }
        }

        Ok(NarrativeWorkspaceSnapshot {
            source_count,
            claim_count,
            narrative_sides_captured,
            agreement_count,
            crux_count,
            orientation_captured,
            sections_captured,
            research_gap_count,
        })
    }

    async fn sync_narrative_map_section(db: &impl ConnectionTrait) -> Result<()> {
        let map = load_narrative_map_fields(db).await?;
        let agreements = query_strings(
            db,
            "SELECT body FROM narrative_map_items WHERE item_type = 'agreement' ORDER BY item_order",
        )
        .await?;
        let cruxes = query_strings(
            db,
            "SELECT body FROM narrative_map_items WHERE item_type = 'crux' ORDER BY item_order",
        )
        .await?;

        let body = json!({
            "dominant": map.dominant.unwrap_or_default(),
            "bull": map.bull.unwrap_or_default(),
            "bear": map.bear.unwrap_or_default(),
            "consensus": map.consensus.unwrap_or_default(),
            "counter_narrative": map.counter_narrative.unwrap_or_default(),
            "agreements": agreements,
            "cruxes": cruxes,
        });
        Self::upsert_section_body(db, "narrative_map", None, &body.to_string(), "draft").await
    }

    async fn upsert_section_body(
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
}

struct NarrativeMapFields {
    dominant: Option<String>,
    bull: Option<String>,
    bear: Option<String>,
    consensus: Option<String>,
    counter_narrative: Option<String>,
}

async fn ensure_narrative_map_row(db: &impl ConnectionTrait) -> Result<()> {
    let count = scalar_i64(db, "SELECT COUNT(*) AS count FROM narrative_map").await?;
    if count == 0 {
        execute_sql(db, "INSERT INTO narrative_map (id) VALUES (1)").await?;
    }
    Ok(())
}

async fn load_narrative_map_fields(db: &impl ConnectionTrait) -> Result<NarrativeMapFields> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT dominant, bull, bear, consensus, counter_narrative FROM narrative_map WHERE id = 1"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load narrative_map failed: {err}")))?;

    let Some(row) = row else {
        return Ok(NarrativeMapFields {
            dominant: None,
            bull: None,
            bear: None,
            consensus: None,
            counter_narrative: None,
        });
    };

    Ok(NarrativeMapFields {
        dominant: row.try_get::<String>("", "dominant").ok(),
        bull: row.try_get::<String>("", "bull").ok(),
        bear: row.try_get::<String>("", "bear").ok(),
        consensus: row.try_get::<String>("", "consensus").ok(),
        counter_narrative: row.try_get::<String>("", "counter_narrative").ok(),
    })
}

async fn find_existing_source_id(
    db: &impl ConnectionTrait,
    source: &CaptureSourceInput,
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

async fn claim_already_exists(
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

async fn narrative_item_exists(
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

async fn resolve_source_id(db: &impl ConnectionTrait, claim: &CaptureClaimInput) -> Result<Option<i64>> {
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

fn validate_claim_relaxed(claim: &CaptureClaimInput) -> Result<()> {
    if claim.claim.trim().is_empty() {
        return Err(Error::string("claim cannot be empty"));
    }
    Ok(())
}

async fn section_has_body(db: &impl ConnectionTrait, section_key: &str) -> Result<bool> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT body FROM sections WHERE section_key = '{}'",
                sql_quote(section_key),
            ),
        ))
        .await
        .map_err(|err| Error::string(&format!("section lookup failed: {err}")))?;
    Ok(row
        .and_then(|row| row.try_get::<String>("", "body").ok())
        .is_some_and(|body| !body.trim().is_empty()))
}

async fn query_strings(db: &impl ConnectionTrait, sql: &str) -> Result<Vec<String>> {
    let rows = db
        .query_all(Statement::from_string(DatabaseBackend::Sqlite, sql.to_string()))
        .await
        .map_err(|err| Error::string(&format!("query failed: {err}")))?;
    rows.into_iter()
        .map(|row| {
            row.try_get::<String>("", "body")
                .map_err(|err| Error::string(&format!("parse body: {err}")))
        })
        .collect()
}

fn validation_error(err: ValidationError) -> Error {
    Error::string(&err.to_string())
}
