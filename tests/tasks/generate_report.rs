use analogues::tasks::{
    generate_report::{generate_report, GenerateReportRequest},
    init_workspace::{initialize_workspace, InitWorkspaceRequest},
};
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};
use std::{fs, path::PathBuf};

use serial_test::serial;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn test_can_run_generate_report() {
    let base_dir = temp_report_root();
    let init_request = InitWorkspaceRequest {
        ticker: "MSFT".to_string(),
        date: "2026-06-04".to_string(),
        base_dir: base_dir.clone(),
        fetch_financials: false,
    };
    let paths = initialize_workspace(&init_request).await.unwrap();
    let db = open_run_db_rw(&paths.sqlite_path).await;

    seed_minimum_report_data(&db).await;
    db.close().await.unwrap();

    let report_path = generate_report(&GenerateReportRequest {
        ticker: "MSFT".to_string(),
        date: "2026-06-04".to_string(),
        index: Some(1),
        base_dir: base_dir.clone(),
    })
    .await
    .unwrap();

    assert_eq!(report_path, paths.generated_dir.join("report.html"));
    let html = fs::read_to_string(&report_path).unwrap();
    assert!(html.contains("Scenario One"));
    assert!(html.contains("report-data"));
    assert!(html.contains("historical_growth"));
    assert!(html.contains("data_quality"));

    let db = open_run_db_rw(&paths.sqlite_path).await;
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) FROM artifacts WHERE artifact_type = 'report_html'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar_i64(&db, "SELECT COUNT(*) FROM monte_carlo_summary").await,
        1
    );
    assert_eq!(
        scalar_i64(&db, "SELECT COUNT(*) FROM monte_carlo_histogram_bins").await,
        5
    );
    assert_eq!(
        scalar_i64(
            &db,
            "SELECT COUNT(*) FROM monte_carlo_scenario_probabilities"
        )
        .await,
        1
    );

    db.close().await.unwrap();
    fs::remove_dir_all(base_dir).unwrap();
}

async fn seed_minimum_report_data(db: &sea_orm::DatabaseConnection) {
    execute_sql(
        db,
        "UPDATE stock_info
         SET company_name = 'Microsoft Corp', currency = 'USD'
         WHERE id = 1",
    )
    .await;

    for sql in [
        "INSERT INTO fundamentals (metric_key, metric_label, metric_value, unit, period, updated_at)
         VALUES ('current_price', 'Current price', 100.0, 'USD', NULL, '2026-06-04T00:00:00Z')",
        "INSERT INTO fundamentals (metric_key, metric_label, metric_value, unit, period, updated_at)
         VALUES ('revenue_ttm', 'Revenue TTM', 1000000000.0, 'USD', '2026-06-30', '2026-06-04T00:00:00Z')",
        "INSERT INTO fundamentals (metric_key, metric_label, metric_value, unit, period, updated_at)
         VALUES ('shares_outstanding', 'Shares outstanding', 100000000.0, 'shares', '2026-06-30', '2026-06-04T00:00:00Z')",
        "INSERT INTO fundamentals (metric_key, metric_label, metric_value, unit, period, updated_at)
         VALUES ('net_margin', 'Net margin', 0.2, 'ratio', '2026-06-30', '2026-06-04T00:00:00Z')",
        "INSERT INTO fundamentals (metric_key, metric_label, metric_value, unit, period, updated_at)
         VALUES ('eps_ttm', 'EPS TTM', 2.0, 'USD', '2026-06-30', '2026-06-04T00:00:00Z')",
    ] {
        execute_sql(db, sql).await;
    }

    for sql in [
        "INSERT INTO fundamental_observations (
            canonical_key, metric_key, metric_label, statement_type, period_type, period_start, period_end,
            metric_value, unit, source_type, source_note, quality, is_derived, updated_at
         ) VALUES (
            'revenue', 'revenue', 'Revenue', 'income_statement', 'annual', '2025-07-01', '2026-06-30',
            1000000000.0, 'USD', 'SEC Company Facts', 'Seeded annual revenue.', 'reported', 0,
            '2026-06-04T00:00:00Z'
         )",
        "INSERT INTO fundamental_observations (
            canonical_key, metric_key, metric_label, statement_type, period_type, period_start, period_end,
            metric_value, unit, source_type, source_note, quality, is_derived, updated_at
         ) VALUES (
            'revenue', 'revenue_ttm', 'Revenue TTM', 'income_statement', 'ttm', '2025-07-01', '2026-06-30',
            1000000000.0, 'USD', 'SEC Company Facts', 'Seeded aligned revenue TTM.', 'aligned', 1,
            '2026-06-04T00:00:00Z'
         )",
    ] {
        execute_sql(db, sql).await;
    }

    execute_sql(
        db,
        "INSERT INTO sources (title, url, source_type, published_at, why_it_matters)
         VALUES ('FY 2026 filing', 'https://example.com/filing', 'Filing', '2026-06-01', 'Baseline financials')",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO claims (claim, source_id, claim_type, side, confidence, metric)
         VALUES ('Revenue can compound in the scenario window.', 1, 'revenue growth', 'bull', 'Medium', 'revenue')",
    )
    .await;

    for (key, body) in [
        (
            "orientation",
            r#"{"one_minute_version":"A simple seeded report.","dominant_question":"Can growth persist?","time_horizon":"3 years","current_setup":"Baseline setup","base_rate_warning":"Illustrative only"}"#,
        ),
        (
            "business_model",
            r#"{"what_the_company_sells":"Software","how_it_makes_money":"Subscriptions"}"#,
        ),
        ("why_now", r#"{"next_catalysts":"Earnings updates"}"#),
        ("industry_context", r#"{"market_structure":"Concentrated"}"#),
        (
            "final_talk_track",
            r#"{"one_minute_version":"Scenario-conditioned narrative.","bull_case":"Growth improves","bear_case":"Growth fades"}"#,
        ),
    ] {
        execute_sql(
            db,
            &format!(
                "UPDATE sections SET body = '{}' WHERE section_key = '{}'",
                body.replace('\'', "''"),
                key
            ),
        )
        .await;
    }

    execute_sql(
        db,
        "INSERT INTO narrative_map (id, dominant, bull, bear, consensus, counter_narrative)
         VALUES (1, 'AI growth', 'Growth accelerates', 'Multiple compresses', 'Durable compounder', 'Margins matter')",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO content_blocks (section_key, block_order, block_type, title, body)
         VALUES ('financial_math', 1, 'paragraph', 'Economic bridge', 'Revenue and margin drive the scenario math.')",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO watch_items (item_order, title, description, signal_type)
         VALUES (1, 'Growth durability', 'Revenue growth relative to expectations', 'Scenario One')",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO historical_analogues (analogue_order, analogue, setup, lesson, why_it_can_mislead)
         VALUES (1, 'Large-cap software transition', 'Durable growth narrative', 'Watch margin durability', 'Different market context')",
    )
    .await;

    execute_sql(
        db,
        "INSERT INTO scenario_assumptions (
            scenario_order, name, stance, probability, description, assumption_summary
         ) VALUES (
            1, 'Scenario One', 'bullish', 1.0, 'Growth improves modestly.', 'Revenue growth and margin are stable.'
         )",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO scenario_crux_assumptions (scenario_id, crux_order, crux, assumption, impact)
         VALUES (1, 1, 'Growth durability', 'Growth remains above baseline.', 'Revenue expands')",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
         VALUES (1, 1, 'confirming', 'Revenue guide rises')",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
         VALUES (1, 1, 'breaking', 'Margins compress')",
    )
    .await;
    execute_sql(
        db,
        "INSERT INTO scenario_periods (
            scenario_id, period_order, label, revenue_growth, diluted_shares,
            net_margin, ps_low, ps_median, ps_high, pe_low, pe_median, pe_high,
            blend_ps_weight, blend_pe_weight
         ) VALUES (
            1, 1, '+12 months', 0.10, 100000000.0,
            0.21, 8.0, 9.0, 10.0, 30.0, 35.0, 40.0,
            0.5, 0.5
         )",
    )
    .await;
    execute_sql(
        db,
        "UPDATE monte_carlo_config SET iterations = 100, seed = 42, bins = 5 WHERE id = 1",
    )
    .await;
}

async fn open_run_db_rw(path: &PathBuf) -> sea_orm::DatabaseConnection {
    Database::connect(format!(
        "sqlite://{}?mode=rw",
        path.to_string_lossy().replace('\\', "/")
    ))
    .await
    .unwrap()
}

async fn execute_sql(db: &sea_orm::DatabaseConnection, sql: &str) {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        sql.to_string(),
    ))
    .await
    .unwrap();
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

fn temp_report_root() -> PathBuf {
    std::env::temp_dir().join(format!("analogues-generate-report-test-{}", Uuid::new_v4()))
}
