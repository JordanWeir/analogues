mod gate;
mod writes;

use super::{
    context::LaneContext, gate::Gate, lane::Lane, result::LaneResult, result::LaneStatus,
    result::LaneWritesSummary,
};
use crate::{
    services::workspace_ingest::{
        finalize_sec_catalog_if_present, record_financial_fetch_status, run_workspace_ingest,
    },
    workspace::InitWorkspaceRequest,
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

pub struct InitWorkspaceLane {
    fetch_financials: bool,
    defer_catalog: bool,
}

impl InitWorkspaceLane {
    pub fn new(request: &InitWorkspaceRequest) -> Self {
        Self {
            fetch_financials: request.fetch_financials,
            defer_catalog: request.fetch_financials && request.mapping_strategy.is_none(),
        }
    }
}

#[async_trait]
impl Lane for InitWorkspaceLane {
    fn name(&self) -> &'static str {
        "init_workspace"
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        gate::init_workspace_gates()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult> {
        let outcome = run_workspace_ingest(
            ctx.workspace.connection(),
            ctx.ticker(),
            self.fetch_financials,
        )
        .await?;

        let mut writes = LaneWritesSummary::default()
            .wrote("stock_info")
            .wrote("run_metadata");

        if outcome.skipped {
            writes = writes
                .wrote("data_gaps")
                .note("financial fetch skipped by request");
            return Ok(LaneResult {
                lane_name: self.name().to_string(),
                status: LaneStatus::Skipped,
                writes,
                gate_results: Vec::new(),
                error_message: None,
            });
        }

        if outcome.alpha_vantage_persisted {
            writes = writes.wrote("av_raw_facts");
        }
        if outcome.sec_ingested {
            writes = writes.wrote("sec_raw_facts");
            finalize_sec_catalog_if_present(&ctx.workspace).await?;
            writes = writes.wrote("concept_catalog_entries");
        }
        if outcome.market_persisted {
            writes = writes
                .wrote("fundamentals")
                .wrote("fundamental_observations");
        }
        if outcome.fetch_status == "failed" {
            writes = writes.wrote("data_gaps");
        }

        if self.defer_catalog && outcome.alpha_vantage_persisted {
            record_financial_fetch_status(
                ctx.workspace.connection(),
                "ingested",
                Some("Alpha Vantage canonical mapping and starter fundamentals deferred"),
            )
            .await?;
        }

        for note in &outcome.source_notes {
            writes = writes.note(note.clone());
        }

        Ok(LaneResult {
            lane_name: self.name().to_string(),
            status: LaneStatus::Success,
            writes,
            gate_results: Vec::new(),
            error_message: outcome.fetch_error,
        })
    }
}
