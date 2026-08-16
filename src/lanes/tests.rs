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
    let workspace_dir = std::env::temp_dir().join(format!(
        "analogues-lane-runner-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&workspace_dir).expect("workspace dir");
    let path = workspace_dir.join("run.sqlite");
    let paths = WorkspacePaths {
        run_slug: "TEST-2026-06-08-1".to_string(),
        workspace_dir: workspace_dir.clone(),
        sqlite_path: path.clone(),
        generated_dir: workspace_dir.join("generated"),
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
            checkpoints: false,
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

async fn test_lane_context_with_checkpoints(checkpoints: bool) -> (LaneContext, PathBuf) {
    let (mut ctx, path) = test_lane_context().await;
    ctx.config.checkpoints = checkpoints;
    let checkpoints_dir = ctx.workspace.paths.workspace_dir.join("checkpoints");
    if checkpoints {
        std::fs::create_dir_all(&checkpoints_dir).expect("checkpoints dir");
    }
    (ctx, checkpoints_dir)
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

#[tokio::test]
async fn linear_runner_saves_checkpoints_after_each_completed_lane() {
    let (mut ctx, checkpoints_dir) = test_lane_context_with_checkpoints(true).await;
    let runner = LinearRunner::new(vec![
        Arc::new(StubLane::new("lane_a", LaneStatus::Success)),
        Arc::new(StubLane::new("lane_b", LaneStatus::Success)),
    ]);

    let report = runner.run(&mut ctx).await.expect("run");

    assert!(report.completed_all_lanes());
    assert!(checkpoints_dir.join("lane_a.sqlite").is_file());
    assert!(checkpoints_dir.join("lane_b.sqlite").is_file());
}

#[tokio::test]
async fn linear_runner_does_not_save_checkpoints_when_disabled() {
    let (mut ctx, checkpoints_dir) = test_lane_context_with_checkpoints(false).await;
    let runner = LinearRunner::new(vec![Arc::new(StubLane::new(
        "lane_a",
        LaneStatus::Success,
    ))]);

    runner.run(&mut ctx).await.expect("run");

    assert!(!checkpoints_dir.exists());
}
