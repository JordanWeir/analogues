use crate::lanes::{
    context::LaneContext,
    gate::GateResult,
    lane::Lane,
    result::{LaneResult, LinearRunReport},
};
use crate::services::{
    quality_gate_store::QualityGateStore, workspace_checkpoint_store::WorkspaceCheckpointStore,
};
use loco_rs::prelude::*;
use std::sync::Arc;

/// Runs pipeline lanes in order, evaluating quality gates after each lane.
///
/// Stops when a lane fails, returns a failed status, or a gate rejects/quarantines.
/// Warnings are recorded but do not stop the pipeline.
pub struct LinearRunner {
    lanes: Vec<Arc<dyn Lane>>,
}

impl LinearRunner {
    pub fn new(lanes: Vec<Arc<dyn Lane>>) -> Self {
        Self { lanes }
    }

    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    pub async fn run(&self, ctx: &mut LaneContext) -> Result<LinearRunReport> {
        let mut report = LinearRunReport {
            lane_results: Vec::new(),
            stopped_early: false,
            stop_reason: None,
        };

        for lane in &self.lanes {
            let mut result = match lane.run(ctx).await {
                Ok(result) => result,
                Err(err) => {
                    report.stopped_early = true;
                    report.stop_reason = Some(format!("{} failed: {err}", lane.name()));
                    break;
                }
            };

            let gate_results = self.evaluate_gates(lane.as_ref(), ctx, &result).await?;
            QualityGateStore::persist_batch(
                &ctx.workspace.paths.sqlite_path,
                &result.lane_name,
                &gate_results,
            )
            .await?;
            result = result.with_gate_results(gate_results);

            if !result.status.is_success() {
                report.lane_results.push(result);
                report.stopped_early = true;
                report.stop_reason = Some(format!(
                    "{} returned status {:?}",
                    lane.name(),
                    report.lane_results.last().map(|r| &r.status)
                ));
                break;
            }

            if result.has_blocking_gate_failure() {
                report.lane_results.push(result);
                report.stopped_early = true;
                report.stop_reason = Some(format!("{} failed a quality gate", lane.name()));
                break;
            }

            report.lane_results.push(result);

            if let Some(checkpoints_dir) =
                ctx.config.checkpoints_dir(&ctx.workspace.paths.workspace_dir)
            {
                WorkspaceCheckpointStore::save_lane_checkpoint(
                    ctx.workspace.connection(),
                    &checkpoints_dir,
                    lane.name(),
                )
                .await?;
            }
        }

        Ok(report)
    }

    async fn evaluate_gates(
        &self,
        lane: &dyn Lane,
        ctx: &LaneContext,
        result: &LaneResult,
    ) -> Result<Vec<GateResult>> {
        let mut gate_results = Vec::new();
        for gate in lane.gates() {
            gate_results.push(gate.check(ctx, result).await);
        }
        Ok(gate_results)
    }
}
