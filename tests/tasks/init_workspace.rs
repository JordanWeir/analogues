/*
Init Workspace Task Goals:
- Parse and validate the requested ticker/run slug.
- Create working directory for the research run.
    - reports/stock-narrative-research/${TICKER}-YYYY-MM-DD-{INDEX}
    - Ticker is the symbol; INDEX is the run number if it's already been run that day, to avoid overwrites
- Create expected subdirectories, including generated/.
- Initialize run.sqlite in the workspace directory.
- Apply the expected SQLite schema and record schema_version/run metadata.
- Seed stock_info, fundamentals, and empty/placeholder section records the agent should populate.
- Fetch starter financial information when available and populate the corresponding tables.
- If financial fetch fails, keep the workspace usable and record the data gap.
- Print the created workspace path and SQLite path.

*/

use analogues::app::App;
use analogues::tasks::init_workspace::{initialize_workspace, InitWorkspaceRequest};
use loco_rs::{task, testing::prelude::*};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use std::{fs, path::PathBuf};

use loco_rs::boot::run_task;
use serial_test::serial;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn test_can_run_init_workspace() {
    let boot = boot_test::<App>().await.unwrap();
    let base_dir = temp_report_root();

    assert!(run_task::<App>(
        &boot.app_context,
        Some(&"initWorkspace".to_string()),
        &task::Vars::from_cli_args(vec![
            ("ticker".to_string(), "MSFT".to_string()),
            ("date".to_string(), "2026-06-04".to_string()),
            ("base_dir".to_string(), base_dir.display().to_string()),
            ("fetch_financials".to_string(), "false".to_string()),
        ]),
    )
    .await
    .is_ok());

    assert!(base_dir.join("MSFT-2026-06-04-1").exists());

    fs::remove_dir_all(base_dir).unwrap();
}

#[tokio::test]
#[serial]
async fn test_initializes_workspace_directories_and_database() {
    let base_dir = temp_report_root();
    let request = InitWorkspaceRequest {
        ticker: "MSFT".to_string(),
        date: "2026-06-04".to_string(),
        base_dir: base_dir.clone(),
        fetch_financials: false,
    };

    let paths = initialize_workspace(&request).await.unwrap();

    assert_eq!(paths.run_slug, "MSFT-2026-06-04-1");
    assert!(paths.workspace_dir.is_dir());
    assert!(paths.generated_dir.is_dir());
    assert!(paths.sqlite_path.is_file());

    let db = open_run_db(&paths.sqlite_path).await;

    assert_eq!(
        scalar_string(&db, "SELECT ticker FROM run_metadata WHERE id = 1").await,
        "MSFT"
    );
    assert_eq!(
        scalar_string(&db, "SELECT ticker FROM stock_info WHERE id = 1").await,
        "MSFT"
    );
    assert_eq!(
        scalar_i64(&db, "SELECT COUNT(*) AS count FROM sections").await,
        11
    );
    assert_eq!(
        scalar_string(
            &db,
            "SELECT status FROM data_gaps WHERE gap_key = 'starter_financials'"
        )
        .await,
        "open"
    );
    assert_eq!(
        scalar_string(
            &db,
            "SELECT financial_fetch_status FROM run_metadata WHERE id = 1"
        )
        .await,
        "skipped"
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'scenario_outputs'"
        )
        .await,
        0
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN (
                'sec_raw_facts',
                'canonical_metric_definitions',
                'canonical_metric_mappings',
                'supporting_metric_selections',
                'fundamental_observations',
                'data_quality_flags',
                'scenario_crux_assumptions',
                'scenario_sensitivities',
                'scenario_signals',
                'monte_carlo_config',
                'monte_carlo_summary',
                'monte_carlo_histogram_bins',
                'monte_carlo_scenario_probabilities'
            )"
        )
        .await,
        13
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'view' AND name IN (
                'raw_fact_metric_catalog',
                'canonical_fundamental_observations'
            )"
        )
        .await,
        2
    );
    assert_eq!(
        scalar_i64(&db, "SELECT COUNT(*) FROM canonical_metric_definitions").await,
        9
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) FROM monte_carlo_config
             WHERE id = 1 AND iterations = 10000 AND seed = 42 AND bins = 30"
        )
        .await,
        1
    );

    db.close().await.unwrap();
    fs::remove_dir_all(base_dir).unwrap();
}

#[tokio::test]
#[serial]
async fn test_allocates_next_index_without_overwriting() {
    let base_dir = temp_report_root();
    let request = InitWorkspaceRequest {
        ticker: "MSFT".to_string(),
        date: "2026-06-04".to_string(),
        base_dir: base_dir.clone(),
        fetch_financials: false,
    };

    let first = initialize_workspace(&request).await.unwrap();
    let second = initialize_workspace(&request).await.unwrap();

    assert_eq!(first.run_slug, "MSFT-2026-06-04-1");
    assert_eq!(second.run_slug, "MSFT-2026-06-04-2");
    assert!(first.sqlite_path.is_file());
    assert!(second.sqlite_path.is_file());

    fs::remove_dir_all(base_dir).unwrap();
}

fn temp_report_root() -> PathBuf {
    std::env::temp_dir().join(format!("analogues-init-workspace-test-{}", Uuid::new_v4()))
}

async fn open_run_db(path: &PathBuf) -> sea_orm::DatabaseConnection {
    Database::connect(format!(
        "sqlite://{}?mode=ro",
        path.to_string_lossy().replace('\\', "/")
    ))
    .await
    .unwrap()
}

async fn scalar_string(db: &sea_orm::DatabaseConnection, sql: &str) -> String {
    db.query_one(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index::<String>(0)
    .unwrap()
}

async fn scalar_i64(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
    db.query_one(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get_by_index::<i64>(0)
    .unwrap()
}
