mod gate;
mod writes;

use super::{context::LaneContext, gate::Gate, lane::Lane, result::LaneResult, result::LaneStatus, result::LaneWritesSummary};
use crate::services::workspace_ingest::run_workspace_ingest;
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

pub struct InitWorkspaceLane {
    fetch_financials: bool,
}

impl InitWorkspaceLane {
    pub fn new(fetch_financials: bool) -> Self {
        Self { fetch_financials }
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
            writes = writes.wrote("data_gaps").note("financial fetch skipped by request");
            return Ok(LaneResult {
                lane_name: self.name().to_string(),
                status: LaneStatus::Skipped,
                writes,
                gate_results: Vec::new(),
                error_message: None,
            });
        }

        if outcome.sec_ingested {
            writes = writes.wrote("sec_raw_facts");
        }
        if outcome.fetch_status == "failed" {
            writes = writes.wrote("data_gaps");
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
