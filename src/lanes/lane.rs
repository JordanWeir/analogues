use crate::lanes::{context::LaneContext, gate::Gate, result::LaneResult};
use async_trait::async_trait;
use loco_rs::prelude::*;
use std::sync::Arc;

/// A pipeline stage in the linear research path (see `01-pipeline-plan.md`).
///
/// Lanes orchestrate; `src/services/` owns deterministic domain logic; agents
/// own model loops. Each lane module should declare its reads/writes contract
/// and attach quality gates for downstream trust.
#[async_trait]
pub trait Lane: Send + Sync {
    fn name(&self) -> &'static str;

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        Vec::new()
    }

    async fn run(&self, ctx: &mut LaneContext) -> Result<LaneResult>;
}
