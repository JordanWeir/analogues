use super::{
    board::{
        ClaimRow, GapRow, NarrativeBoard, NarrativeItemRow, NarrativeMapFields, SectionRow,
        SourceRow,
    },
    NarrativeResearchStore,
};
use crate::{
    agents::narrative_researcher::types::NarrativeWorkspaceSnapshot,
    services::workspace_sql::{scalar_i64, sql_quote},
};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

const SECTION_KEYS: &[&str] = &["orientation", "business_model", "why_now", "narrative_map"];

impl<'a> NarrativeResearchStore<'a> {
    /// Load durable narrative state already on the board for agent context (no deletes).
    pub async fn load_existing_context(&self) -> Result<Value> {
        Ok(serialize_board(&load_board(self.db).await?))
    }

    /// Lightweight workspace metrics for capture tool responses and post-run checks.
    pub async fn snapshot(&self) -> Result<NarrativeWorkspaceSnapshot> {
        snapshot_metrics(self.db).await
    }
}

pub(crate) async fn load_board(db: &impl ConnectionTrait) -> Result<NarrativeBoard> {
    Ok(NarrativeBoard {
        sources: load_sources(db).await?,
        claims: load_claims(db).await?,
        map: load_narrative_map_fields(db).await?,
        agreements: load_narrative_items(db, "agreement").await?,
        cruxes: load_narrative_items(db, "crux").await?,
        sections: load_sections(db).await?,
        gaps: load_gaps(db).await?,
    })
}

pub(crate) async fn snapshot_metrics(
    db: &impl ConnectionTrait,
) -> Result<NarrativeWorkspaceSnapshot> {
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
    let narrative_sides_captured = narrative_sides_from_map(&map);
    let orientation_captured = section_has_body_db(db, "orientation").await?;
    let mut sections_captured = Vec::new();
    for key in ["business_model", "why_now"] {
        if section_has_body_db(db, key).await? {
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

pub(crate) fn serialize_board(board: &NarrativeBoard) -> Value {
    let source_rows: Vec<Value> = board
        .sources
        .iter()
        .map(|source| {
            json!({
                "id": source.id,
                "title": source.title,
                "url": source.url,
                "source_type": source.source_type,
                "published_at": source.published_at,
                "why_it_matters": source.why_it_matters,
                "notes": source.notes,
            })
        })
        .collect();

    let claim_rows: Vec<Value> = board
        .claims
        .iter()
        .map(|claim| {
            json!({
                "id": claim.id,
                "claim": claim.claim,
                "source_id": claim.source_id,
                "claim_type": claim.claim_type,
                "side": claim.side,
                "confidence": claim.confidence,
                "metric": claim.metric,
                "notes": claim.notes,
            })
        })
        .collect();

    let agreement_rows: Vec<Value> = board
        .agreements
        .iter()
        .map(serialize_narrative_item)
        .collect();
    let crux_rows: Vec<Value> = board.cruxes.iter().map(serialize_narrative_item).collect();

    let sections: Map<String, Value> = board
        .sections
        .iter()
        .map(|(key, section)| {
            (
                key.clone(),
                json!({
                    "status": section.status,
                    "title": section.title,
                    "body": section.body,
                }),
            )
        })
        .collect();

    let gap_rows: Vec<Value> = board
        .gaps
        .iter()
        .map(|gap| {
            json!({
                "gap_key": gap.gap_key,
                "description": gap.description,
                "status": gap.status,
            })
        })
        .collect();

    json!({
        "sources": source_rows,
        "claims": claim_rows,
        "narrative_map": {
            "dominant": board.map.dominant,
            "bull": board.map.bull,
            "bear": board.map.bear,
            "consensus": board.map.consensus,
            "counter_narrative": board.map.counter_narrative,
        },
        "narrative_map_items": {
            "agreements": agreement_rows,
            "cruxes": crux_rows,
        },
        "sections": sections,
        "research_gaps": gap_rows,
    })
}

pub(crate) fn summarize_board(board: &NarrativeBoard) -> NarrativeWorkspaceSnapshot {
    let narrative_sides_captured = narrative_sides_from_map(&board.map);
    let orientation_captured = section_has_body(board, "orientation");
    let mut sections_captured = Vec::new();
    for key in ["business_model", "why_now"] {
        if section_has_body(board, key) {
            sections_captured.push(key.to_string());
        }
    }

    NarrativeWorkspaceSnapshot {
        source_count: board.sources.len() as i64,
        claim_count: board.claims.len() as i64,
        narrative_sides_captured,
        agreement_count: board.agreements.len() as i64,
        crux_count: board.cruxes.len() as i64,
        orientation_captured,
        sections_captured,
        research_gap_count: board.gaps.len() as i64,
    }
}

pub(crate) async fn load_narrative_map_fields(
    db: &impl ConnectionTrait,
) -> Result<NarrativeMapFields> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT dominant, bull, bear, consensus, counter_narrative FROM narrative_map WHERE id = 1"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load narrative_map failed: {err}")))?;

    let Some(row) = row else {
        return Ok(NarrativeMapFields::default());
    };

    Ok(NarrativeMapFields {
        dominant: row.try_get::<String>("", "dominant").ok(),
        bull: row.try_get::<String>("", "bull").ok(),
        bear: row.try_get::<String>("", "bear").ok(),
        consensus: row.try_get::<String>("", "consensus").ok(),
        counter_narrative: row.try_get::<String>("", "counter_narrative").ok(),
    })
}

pub(crate) async fn load_narrative_item_bodies(
    db: &impl ConnectionTrait,
    item_type: &str,
) -> Result<Vec<String>> {
    Ok(load_narrative_items(db, item_type)
        .await?
        .into_iter()
        .map(|item| item.body)
        .collect())
}

fn narrative_sides_from_map(map: &NarrativeMapFields) -> Vec<String> {
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
    narrative_sides_captured
}

fn section_has_body(board: &NarrativeBoard, section_key: &str) -> bool {
    board
        .sections
        .get(section_key)
        .and_then(|section| section.body.as_deref())
        .is_some_and(|body| !body.trim().is_empty())
}

async fn section_has_body_db(db: &impl ConnectionTrait, section_key: &str) -> Result<bool> {
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

fn serialize_narrative_item(item: &NarrativeItemRow) -> Value {
    json!({
        "id": item.id,
        "item_order": item.item_order,
        "body": item.body,
    })
}

async fn load_sources(db: &impl ConnectionTrait) -> Result<Vec<SourceRow>> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id, title, url, source_type, published_at, why_it_matters, notes
             FROM sources ORDER BY id"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load sources failed: {err}")))?;

    rows.into_iter()
        .map(|row| {
            Ok(SourceRow {
                id: row.try_get::<i64>("", "id")?,
                title: row.try_get::<String>("", "title").ok(),
                url: row.try_get::<String>("", "url").ok(),
                source_type: row.try_get::<String>("", "source_type").ok(),
                published_at: row.try_get::<String>("", "published_at").ok(),
                why_it_matters: row.try_get::<String>("", "why_it_matters").ok(),
                notes: row.try_get::<String>("", "notes").ok(),
            })
        })
        .collect()
}

async fn load_claims(db: &impl ConnectionTrait) -> Result<Vec<ClaimRow>> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id, claim, source_id, claim_type, side, confidence, metric, notes
             FROM claims ORDER BY id"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load claims failed: {err}")))?;

    rows.into_iter()
        .map(|row| {
            Ok(ClaimRow {
                id: row.try_get::<i64>("", "id")?,
                claim: row.try_get::<String>("", "claim")?,
                source_id: row.try_get::<i64>("", "source_id").ok(),
                claim_type: row.try_get::<String>("", "claim_type").ok(),
                side: row.try_get::<String>("", "side").ok(),
                confidence: row.try_get::<String>("", "confidence").ok(),
                metric: row.try_get::<String>("", "metric").ok(),
                notes: row.try_get::<String>("", "notes").ok(),
            })
        })
        .collect()
}

async fn load_narrative_items(
    db: &impl ConnectionTrait,
    item_type: &str,
) -> Result<Vec<NarrativeItemRow>> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT id, item_type, item_order, body FROM narrative_map_items
                 WHERE item_type = '{}' ORDER BY item_order",
                sql_quote(item_type),
            ),
        ))
        .await
        .map_err(|err| Error::string(&format!("load narrative items failed: {err}")))?;

    rows.into_iter()
        .map(|row| {
            Ok(NarrativeItemRow {
                id: row.try_get::<i64>("", "id")?,
                item_order: row.try_get::<i64>("", "item_order")?,
                body: row.try_get::<String>("", "body")?,
            })
        })
        .collect()
}

async fn load_sections(db: &impl ConnectionTrait) -> Result<HashMap<String, SectionRow>> {
    let mut sections = HashMap::new();
    for section_key in SECTION_KEYS {
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
                (*section_key).to_string(),
                SectionRow {
                    status: row.try_get::<String>("", "status").ok(),
                    title: row.try_get::<String>("", "title").ok(),
                    body: row.try_get::<String>("", "body").ok(),
                },
            );
        }
    }
    Ok(sections)
}

async fn load_gaps(db: &impl ConnectionTrait) -> Result<Vec<GapRow>> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT gap_key, description, status FROM data_gaps
             WHERE gap_key LIKE 'narrative_%' ORDER BY gap_key"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load narrative gaps failed: {err}")))?;

    rows.into_iter()
        .map(|row| {
            Ok(GapRow {
                gap_key: row.try_get::<String>("", "gap_key")?,
                description: row.try_get::<String>("", "description")?,
                status: row.try_get::<String>("", "status").ok(),
            })
        })
        .collect()
}
