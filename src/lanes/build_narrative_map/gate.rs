use crate::{
    agents::narrative_researcher::types::MIN_NARRATIVE_BODY_LEN,
    lanes::{
        context::LaneContext,
        gate::{Gate, GateResult},
        result::LaneResult,
    },
    services::workspace_sql::{scalar_i64, sql_quote},
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::sync::Arc;

pub fn build_narrative_map_gates() -> Vec<Arc<dyn Gate>> {
    vec![
        Arc::new(SourcePackPopulatedGate),
        Arc::new(ClaimsSourceCustodyGate),
        Arc::new(NarrativeDebatePresentGate),
        Arc::new(NarrativeCruxesPresentGate),
        Arc::new(EarlySectionsDraftedGate),
    ]
}

struct SourcePackPopulatedGate;

#[async_trait]
impl Gate for SourcePackPopulatedGate {
    fn name(&self) -> &'static str {
        "source_pack_populated"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let source_count =
            scalar_i64(ctx.workspace.connection(), "SELECT COUNT(*) AS count FROM sources")
                .await;
        if let Err(err) = &source_count {
            return GateResult::reject(self.name(), format!("source count failed: {err}"));
        }
        if let Ok(count) = source_count {
            if count < 3 {
                return GateResult::reject(
                    self.name(),
                    format!("need at least 3 sources, found {count}"),
                );
            }
        }

        let primary_sources = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM sources
             WHERE url IS NOT NULL AND TRIM(url) != ''
                OR source_type IN ('Filing', 'Transcript', 'Official company source')",
        )
        .await;
        if let Ok(count) = primary_sources {
            let total = scalar_i64(
                ctx.workspace.connection(),
                "SELECT COUNT(*) AS count FROM sources",
            )
            .await
            .unwrap_or(0);
            if total > 0 && count * 2 < total {
                return GateResult::warn(
                    self.name(),
                    "fewer than half of sources have urls or primary source types",
                );
            }
        }

        let substantive = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM sources WHERE LENGTH(TRIM(why_it_matters)) >= 40",
        )
        .await;
        match substantive {
            Ok(0) => GateResult::reject(
                self.name(),
                "no source has a substantive why_it_matters field",
            ),
            Ok(_) => GateResult::pass(self.name()),
            Err(err) => GateResult::reject(self.name(), format!("source quality check failed: {err}")),
        }
    }
}

struct ClaimsSourceCustodyGate;

#[async_trait]
impl Gate for ClaimsSourceCustodyGate {
    fn name(&self) -> &'static str {
        "claims_source_custody"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let claim_count =
            scalar_i64(ctx.workspace.connection(), "SELECT COUNT(*) AS count FROM claims")
                .await;
        match claim_count {
            Ok(count) if count < 5 => {
                return GateResult::reject(self.name(), format!("need at least 5 claims, found {count}"))
            }
            Err(err) => return GateResult::reject(self.name(), format!("claim count failed: {err}")),
            _ => {}
        }

        let orphan_claims = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM claims
             WHERE confidence != 'inference' AND source_id IS NULL",
        )
        .await;
        if let Ok(count) = orphan_claims {
            if count > 0 {
                return GateResult::reject(
                    self.name(),
                    format!("{count} non-inference claims are missing source_id"),
                );
            }
        }

        let bull_claims = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM claims WHERE side = 'bull'",
        )
        .await
        .unwrap_or(0);
        let bear_claims = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM claims WHERE side = 'bear'",
        )
        .await
        .unwrap_or(0);
        if bull_claims == 0 || bear_claims == 0 {
            return GateResult::reject(
                self.name(),
                "need at least one bull claim and one bear claim",
            );
        }

        GateResult::pass(self.name())
    }
}

struct NarrativeDebatePresentGate;

#[async_trait]
impl Gate for NarrativeDebatePresentGate {
    fn name(&self) -> &'static str {
        "narrative_debate_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let row = ctx
            .workspace
            .connection()
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT dominant, bull, bear, consensus FROM narrative_map WHERE id = 1".to_string(),
            ))
            .await;

        let row = match row {
            Ok(Some(row)) => row,
            Ok(None) => {
                return GateResult::reject(self.name(), "narrative_map row is missing")
            }
            Err(err) => {
                return GateResult::reject(self.name(), format!("narrative_map query failed: {err}"))
            }
        };

        for field in ["dominant", "bull", "bear", "consensus"] {
            let value = row
                .try_get::<String>("", field)
                .unwrap_or_default();
            if value.trim().len() < MIN_NARRATIVE_BODY_LEN {
                return GateResult::reject(
                    self.name(),
                    format!("{field} narrative is missing or too short"),
                );
            }
        }

        let bull = row.try_get::<String>("", "bull").unwrap_or_default();
        let bear = row.try_get::<String>("", "bear").unwrap_or_default();
        if bull.trim().eq_ignore_ascii_case(bear.trim()) {
            return GateResult::reject(self.name(), "bull and bear narratives must differ");
        }

        GateResult::pass(self.name())
    }
}

struct NarrativeCruxesPresentGate;

#[async_trait]
impl Gate for NarrativeCruxesPresentGate {
    fn name(&self) -> &'static str {
        "narrative_cruxes_present"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        let crux_count = scalar_i64(
            ctx.workspace.connection(),
            "SELECT COUNT(*) AS count FROM narrative_map_items WHERE item_type = 'crux'",
        )
        .await;
        match crux_count {
            Ok(count) if count < 2 => GateResult::reject(
                self.name(),
                format!("need at least 2 crux items, found {count}"),
            ),
            Ok(_) => GateResult::pass(self.name()),
            Err(err) => GateResult::reject(self.name(), format!("crux count failed: {err}")),
        }
    }
}

struct EarlySectionsDraftedGate;

#[async_trait]
impl Gate for EarlySectionsDraftedGate {
    fn name(&self) -> &'static str {
        "early_sections_drafted"
    }

    async fn check(&self, ctx: &LaneContext, result: &LaneResult) -> GateResult {
        if result.status == crate::lanes::result::LaneStatus::Skipped {
            return GateResult::pass(self.name());
        }

        for section_key in ["orientation", "business_model", "why_now"] {
            let row = ctx
                .workspace
                .connection()
                .query_one(Statement::from_string(
                    DatabaseBackend::Sqlite,
                    format!(
                        "SELECT status, body FROM sections WHERE section_key = '{}'",
                        sql_quote(section_key),
                    ),
                ))
                .await;

            let row = match row {
                Ok(Some(row)) => row,
                Ok(None) => {
                    return GateResult::reject(self.name(), format!("{section_key} section missing"))
                }
                Err(err) => {
                    return GateResult::reject(
                        self.name(),
                        format!("{section_key} section query failed: {err}"),
                    )
                }
            };

            let status = row.try_get::<String>("", "status").unwrap_or_default();
            let body = row.try_get::<String>("", "body").unwrap_or_default();
            if body.trim().is_empty() {
                return GateResult::reject(self.name(), format!("{section_key} section body is empty"));
            }
            if !matches!(status.as_str(), "draft" | "complete") {
                return GateResult::reject(
                    self.name(),
                    format!("{section_key} section status is {status}, expected draft or complete"),
                );
            }
        }

        GateResult::pass(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lanes::{
            build_narrative_map::BuildNarrativeMapLane,
            result::LaneWritesSummary,
        },
        lanes::build_narrative_map::fixtures::{catalog_lane_context, populate_fixture_narrative},
    };

    #[tokio::test]
    async fn fixture_narrative_passes_all_gates() {
        let (ctx, path) = catalog_lane_context().await;
        populate_fixture_narrative(&path).await;

        let result = LaneResult {
            lane_name: BuildNarrativeMapLane::default_lane_name().to_string(),
            status: crate::lanes::result::LaneStatus::Success,
            writes: LaneWritesSummary::default(),
            gate_results: Vec::new(),
            error_message: None,
        };

        for gate in build_narrative_map_gates() {
            let gate_result = gate.check(&ctx, &result).await;
            assert!(
                !gate_result.is_blocking(),
                "gate {} failed: {:?}",
                gate.name(),
                gate_result
            );
        }

        ctx.workspace.close().await.ok();
    }
}
