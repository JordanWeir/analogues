use crate::services::workspace_sql::execute_sql;
use loco_rs::prelude::*;
use sea_orm::DatabaseConnection;

pub async fn seed_minimum_report_data(db: &DatabaseConnection) -> Result<()> {
    execute_sql(
        db,
        "UPDATE stock_info
         SET company_name = 'Microsoft Corp', currency = 'USD'
         WHERE id = 1",
    )
    .await?;

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
        execute_sql(db, sql).await?;
    }

    for sql in [
        "INSERT INTO fundamental_observations (
            canonical_key, metric_key, metric_label, statement_type, period_type, period_start, period_end,
            metric_value, unit, source_type, source_note, quality, is_derived, updated_at
         ) VALUES (
            'revenue', 'revenue_quarter', 'Revenue', 'income_statement', 'quarter', '2026-04-01', '2026-06-30',
            250000000.0, 'USD', 'SEC Company Facts', 'Seeded quarterly revenue.', 'reported', 0,
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
        execute_sql(db, sql).await?;
    }

    execute_sql(
        db,
        "INSERT INTO sources (title, url, source_type, published_at, why_it_matters)
         VALUES ('FY 2026 filing', 'https://example.com/filing', 'Filing', '2026-06-01', 'Baseline financials')",
    )
    .await?;
    execute_sql(
        db,
        "INSERT INTO claims (claim, source_id, claim_type, side, confidence, metric)
         VALUES ('Revenue can compound in the scenario window.', 1, 'revenue growth', 'bull', 'Medium', 'revenue')",
    )
    .await?;

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
        .await?;
    }

    execute_sql(
        db,
        "INSERT INTO narrative_map (id, dominant, bull, bear, consensus, counter_narrative)
         VALUES (1, 'AI growth', 'Growth accelerates', 'Multiple compresses', 'Durable compounder', 'Margins matter')",
    )
    .await?;
    execute_sql(
        db,
        "INSERT INTO content_blocks (section_key, block_order, block_type, title, body)
         VALUES ('financial_math', 1, 'paragraph', 'Economic bridge', 'Revenue and margin drive the scenario math.')",
    )
    .await?;
    execute_sql(
        db,
        "INSERT INTO watch_items (item_order, title, description, signal_type)
         VALUES (1, 'Growth durability', 'Revenue growth relative to expectations', 'Scenario One')",
    )
    .await?;
    execute_sql(
        db,
        "INSERT INTO historical_analogues (analogue_order, analogue, setup, lesson, why_it_can_mislead)
         VALUES (1, 'Large-cap software transition', 'Durable growth narrative', 'Watch margin durability', 'Different market context')",
    )
    .await?;

    execute_sql(
        db,
        "INSERT INTO scenario_assumptions (
            scenario_order, scenario_key, name, stance, probability, description, assumption_summary
         ) VALUES (
            1, 'scenario_one', 'Scenario One', 'bullish', 1.0, 'Growth improves modestly.', 'Revenue growth and margin are stable.'
         )",
    )
    .await?;
    execute_sql(
        db,
        "INSERT INTO scenario_crux_assumptions (scenario_id, crux_order, crux, assumption, impact)
         VALUES (1, 1, 'Growth durability', 'Growth remains above baseline.', 'Revenue expands')",
    )
    .await?;
    execute_sql(
        db,
        "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
         VALUES (1, 1, 'confirming', 'Revenue guide rises')",
    )
    .await?;
    execute_sql(
        db,
        "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
         VALUES (1, 1, 'breaking', 'Margins compress')",
    )
    .await?;
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
    .await?;
    execute_sql(
        db,
        "UPDATE monte_carlo_config SET iterations = 100, seed = 42, bins = 5 WHERE id = 1",
    )
    .await?;

    Ok(())
}
