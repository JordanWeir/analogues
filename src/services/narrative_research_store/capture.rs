use super::{support, NarrativeResearchStore};
use crate::{
    agents::narrative_researcher::{
        types::{
            CaptureClaimInput, CaptureNarrativeItemsInput, CaptureNarrativeSideInput,
            CaptureOrientationInput, CaptureResearchGapInput, CaptureSectionInput, CaptureSourceInput,
        },
        validate::{
            validate_claim, validate_claim_relaxed, validate_narrative_items,
            validate_narrative_side, validate_orientation, validate_research_gap, validate_section,
            validate_source,
        },
    },
    services::workspace_sql::{execute_sql, last_insert_rowid, scalar_i64, sql_literal, sql_quote},
};
use chrono::Utc;
use loco_rs::prelude::*;
use serde_json::{json, Value};

impl<'a> NarrativeResearchStore<'a> {
    pub async fn capture_sources(&self, sources: Vec<CaptureSourceInput>) -> Result<Value> {
        if sources.is_empty() {
            return Err(Error::string("capture_sources requires at least one source"));
        }
        for source in &sources {
            validate_source(source).map_err(support::validation_error)?;
        }

        let now = Utc::now().to_rfc3339();
        let mut captured = Vec::new();
        for source in sources {
            if let Some(existing_id) = support::find_existing_source_id(self.db, &source).await? {
                captured.push(json!({
                    "id": existing_id,
                    "title": source.title,
                    "status": "already_exists",
                }));
                continue;
            }

            execute_sql(
                self.db,
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
            let id = last_insert_rowid(self.db).await?;
            captured.push(json!({
                "id": id,
                "title": source.title,
                "status": "inserted",
            }));
        }

        let snapshot = self.snapshot().await?;
        Ok(json!({
            "captured": captured,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_claims(&self, claims: Vec<CaptureClaimInput>) -> Result<Value> {
        if claims.is_empty() {
            return Err(Error::string("capture_claims requires at least one claim"));
        }

        let mut inserted = 0_i64;
        let mut skipped = 0_i64;
        for claim in claims {
            if claim.confidence == "inference" {
                validate_claim_relaxed(&claim).map_err(support::validation_error)?;
            } else {
                validate_claim(&claim).map_err(support::validation_error)?;
            }
            let source_id = support::resolve_source_id(self.db, &claim).await?;
            if support::claim_already_exists(self.db, &claim.claim, source_id).await? {
                skipped += 1;
                continue;
            }
            execute_sql(
                self.db,
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

        let snapshot = self.snapshot().await?;
        Ok(json!({
            "claims_added": inserted,
            "claims_skipped_duplicate": skipped,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_narrative_side(
        &self,
        input: CaptureNarrativeSideInput,
    ) -> Result<Value> {
        validate_narrative_side(&input).map_err(support::validation_error)?;
        support::ensure_narrative_map_row(self.db).await?;

        let column = support::narrative_side_column(&input.side)?;

        execute_sql(
            self.db,
            &format!(
                "UPDATE narrative_map SET {column} = '{}' WHERE id = 1",
                sql_quote(&input.body),
            ),
        )
        .await?;

        support::sync_narrative_map_section(self.db).await?;
        let snapshot = self.snapshot().await?;
        Ok(json!({
            "side": input.side,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_narrative_items(
        &self,
        input: CaptureNarrativeItemsInput,
    ) -> Result<Value> {
        validate_narrative_items(&input).map_err(support::validation_error)?;
        support::ensure_narrative_map_row(self.db).await?;

        let start_order = scalar_i64(
            self.db,
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
            if support::narrative_item_exists(self.db, &input.item_type, item).await? {
                items_skipped += 1;
                continue;
            }
            next_order += 1;
            execute_sql(
                self.db,
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

        support::sync_narrative_map_section(self.db).await?;
        let snapshot = self.snapshot().await?;
        Ok(json!({
            "item_type": input.item_type,
            "items_added": items_added,
            "items_skipped_duplicate": items_skipped,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_orientation(&self, input: CaptureOrientationInput) -> Result<Value> {
        validate_orientation(&input).map_err(support::validation_error)?;
        let body = json!({
            "dominant_question": input.dominant_question,
            "current_setup": input.current_setup,
            "time_horizon": input.time_horizon,
            "base_rate_warning": input.base_rate_warning,
        });
        support::upsert_section_body(self.db, "orientation", None, &body.to_string(), "draft")
            .await?;
        let snapshot = self.snapshot().await?;
        Ok(json!({ "workspace": snapshot }))
    }

    pub async fn capture_section(&self, input: CaptureSectionInput) -> Result<Value> {
        validate_section(&input).map_err(support::validation_error)?;
        support::upsert_section_body(
            self.db,
            &input.section_key,
            input.title.as_deref(),
            &input.body,
            "draft",
        )
        .await?;
        let snapshot = self.snapshot().await?;
        Ok(json!({
            "section_key": input.section_key,
            "workspace": snapshot,
        }))
    }

    pub async fn capture_research_gap(&self, input: CaptureResearchGapInput) -> Result<Value> {
        validate_research_gap(&input).map_err(support::validation_error)?;
        let now = Utc::now().to_rfc3339();
        let gap_key = format!("narrative_{}", input.gap_key.trim());
        execute_sql(
            self.db,
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
        let snapshot = self.snapshot().await?;
        Ok(json!({ "gap_key": gap_key, "workspace": snapshot }))
    }
}
