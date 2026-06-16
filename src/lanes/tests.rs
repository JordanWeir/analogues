use super::*;
use crate::{
    services::workspace_store::{execute_schema, WorkspaceStore},
    workspace::{seed_database, InitWorkspaceRequest, WorkspacePaths},
};
use async_trait::async_trait;
use loco_rs::prelude::*;
use sea_orm::Database;
use std::{path::PathBuf, sync::Arc};

struct StubLane {
    name: &'static str,
    gates: Vec<Arc<dyn Gate>>,
    status: LaneStatus,
}

impl StubLane {
    fn new(name: &'static str, status: LaneStatus) -> Self {
        Self {
            name,
            gates: Vec::new(),
            status,
        }
    }

    fn with_gate(mut self, gate: Arc<dyn Gate>) -> Self {
        self.gates.push(gate);
        self
    }
}

#[async_trait]
impl Lane for StubLane {
    fn name(&self) -> &'static str {
        self.name
    }

    fn gates(&self) -> Vec<Arc<dyn Gate>> {
        self.gates.clone()
    }

    async fn run(&self, _ctx: &mut LaneContext) -> Result<LaneResult> {
        Ok(match self.status {
            LaneStatus::Success => LaneResult::success(self.name, LaneWritesSummary::default()),
            LaneStatus::Skipped => LaneResult::skipped(self.name, "skipped for test"),
            LaneStatus::Failed => LaneResult::failed(self.name, "lane failed for test"),
        })
    }
}

struct PassGate;

#[async_trait]
impl Gate for PassGate {
    fn name(&self) -> &'static str {
        "always_pass"
    }

    async fn check(&self, _ctx: &LaneContext, _result: &LaneResult) -> GateResult {
        GateResult::pass(self.name())
    }
}

struct WarnGate;

#[async_trait]
impl Gate for WarnGate {
    fn name(&self) -> &'static str {
        "warn_only"
    }

    async fn check(&self, _ctx: &LaneContext, _result: &LaneResult) -> GateResult {
        GateResult::warn(self.name(), "non-blocking warning")
    }
}

struct RejectGate;

#[async_trait]
impl Gate for RejectGate {
    fn name(&self) -> &'static str {
        "reject_lane"
    }

    async fn check(&self, _ctx: &LaneContext, _result: &LaneResult) -> GateResult {
        GateResult::reject(self.name(), "blocking rejection")
    }
}

async fn test_lane_context() -> (LaneContext, PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "analogues-lane-runner-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let paths = WorkspacePaths {
        run_slug: "TEST-2026-06-08-1".to_string(),
        workspace_dir: path.parent().unwrap().to_path_buf(),
        sqlite_path: path.clone(),
        generated_dir: path.parent().unwrap().join("generated"),
    };

    let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
        .await
        .expect("sqlite");
    execute_schema(&db).await.expect("schema");
    seed_database(
        &db,
        &InitWorkspaceRequest {
            ticker: "TEST".to_string(),
            date: "2026-06-08".to_string(),
            base_dir: PathBuf::from("reports/stock-narrative-research"),
            fetch_financials: false,
            mapping_strategy: None,
            build_narrative_map: false,
            build_financial_analysis: false,
        },
        &paths,
    )
    .await
    .expect("seed");
    db.close().await.expect("close");

    let workspace = WorkspaceStore
        .open_workspace(&path)
        .await
        .expect("open workspace");
    let ctx = LaneContext::new(workspace, LaneConfig::new("TEST"));
    (ctx, path)
}

#[tokio::test]
async fn linear_runner_executes_all_lanes_when_gates_pass() {
    let (mut ctx, _path) = test_lane_context().await;
    let runner = LinearRunner::new(vec![
        Arc::new(StubLane::new("lane_a", LaneStatus::Success)),
        Arc::new(StubLane::new("lane_b", LaneStatus::Success).with_gate(Arc::new(PassGate))),
    ]);

    let report = runner.run(&mut ctx).await.expect("run");

    assert!(report.completed_all_lanes());
    assert_eq!(report.lane_results.len(), 2);
    assert!(report.lane_results.iter().all(|r| r.status.is_success()));
}

#[tokio::test]
async fn linear_runner_continues_on_gate_warn() {
    let (mut ctx, _path) = test_lane_context().await;
    let runner = LinearRunner::new(vec![
        Arc::new(StubLane::new("lane_warn", LaneStatus::Success).with_gate(Arc::new(WarnGate))),
        Arc::new(StubLane::new("lane_after_warn", LaneStatus::Success)),
    ]);

    let report = runner.run(&mut ctx).await.expect("run");

    assert!(report.completed_all_lanes());
    assert_eq!(report.lane_results.len(), 2);
    assert_eq!(
        report.lane_results[0].gate_results[0].status,
        GateStatus::Warn
    );
}

#[tokio::test]
async fn linear_runner_stops_on_gate_reject() {
    let (mut ctx, path) = test_lane_context().await;
    let runner = LinearRunner::new(vec![
        Arc::new(StubLane::new("lane_reject", LaneStatus::Success).with_gate(Arc::new(RejectGate))),
        Arc::new(StubLane::new("lane_never_runs", LaneStatus::Success)),
    ]);

    let report = runner.run(&mut ctx).await.expect("run");

    assert!(report.stopped_early);
    assert_eq!(report.lane_results.len(), 1);
    assert_eq!(report.lane_results[0].lane_name, "lane_reject");
    assert!(report.lane_results[0].has_blocking_gate_failure());

    let count =
        crate::services::quality_gate_store::QualityGateStore::count_for_lane(&path, "lane_reject")
            .await
            .expect("count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn linear_runner_stops_on_failed_lane_status() {
    let (mut ctx, _path) = test_lane_context().await;
    let runner = LinearRunner::new(vec![
        Arc::new(StubLane::new("lane_fail", LaneStatus::Failed)),
        Arc::new(StubLane::new("lane_never_runs", LaneStatus::Success)),
    ]);

    let report = runner.run(&mut ctx).await.expect("run");

    assert!(report.stopped_early);
    assert_eq!(report.lane_results.len(), 1);
    assert_eq!(report.lane_results[0].status, LaneStatus::Failed);
}
