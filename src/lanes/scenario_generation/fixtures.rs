use crate::lanes::context::LaneContext;
use crate::services::workspace_sql::execute_sql;
use loco_rs::prelude::*;

pub async fn persist_fixture_scenarios(ctx: &mut LaneContext) -> Result<()> {
    let db = ctx.workspace.connection();
    for sql in [
        "DELETE FROM scenario_signals",
        "DELETE FROM scenario_sensitivities",
        "DELETE FROM scenario_crux_assumptions",
        "DELETE FROM scenario_periods",
        "DELETE FROM scenario_assumptions",
        "DELETE FROM scenario_projection_periods",
        "DELETE FROM scenario_projection_config",
    ] {
        execute_sql(db, sql).await?;
    }

    let mut calendar_periods: Vec<(i64, String)> = Vec::new();
    for period_order in 1..=16_i64 {
        let month = ((period_order - 1) % 12) + 1;
        let year = 2024 + (period_order - 1) / 12;
        calendar_periods.push((period_order, format!("{year}-{month:02}-28")));
    }
    let historical_anchor_end = calendar_periods[3].1.clone();
    let terminal_period_end = calendar_periods[15].1.clone();
    execute_sql(
        db,
        &format!(
            "INSERT INTO scenario_projection_config (
                id, historical_quarters, forward_quarters, historical_anchor_end, terminal_period_end
             ) VALUES (1, 4, 12, '{historical_anchor_end}', '{terminal_period_end}')"
        ),
    )
    .await?;
    for (period_order, period_end) in &calendar_periods {
        execute_sql(
            db,
            &format!(
                "INSERT INTO scenario_projection_periods (period_order, period_end, is_historical)
                 VALUES ({period_order}, '{period_end}', {})",
                if *period_order <= 4 { 1 } else { 0 }
            ),
        )
        .await?;
    }

    for (order, key, stance, prob) in [
        (1, "bull_path", "bullish", 0.35),
        (2, "base_path", "neutral", 0.35),
        (3, "bear_path", "bearish", 0.20),
        (4, "mixed_path", "mixed", 0.10),
    ] {
        execute_sql(
            db,
            &format!(
                "INSERT INTO scenario_assumptions (
                    scenario_order, scenario_key, name, stance, probability, description, assumption_summary
                 ) VALUES (
                    {order}, '{key}', '{key}', '{stance}', {prob},
                    'Fixture scenario {key}', 'Fixture summary'
                 )"
            ),
        )
        .await?;
    }

    for scenario_id in 1..=4 {
        for (period_order, period_end) in &calendar_periods {
            let is_terminal = *period_order == 16;
            let ps_vals = if is_terminal {
                "8.0, 9.0, 10.0, 30.0, 35.0, 40.0"
            } else {
                "NULL, NULL, NULL, NULL, NULL, NULL"
            };
            execute_sql(
                db,
                &format!(
                    "INSERT INTO scenario_periods (
                        scenario_id, period_order, label, period_end, period_type,
                        revenue_growth, diluted_shares, net_margin,
                        ps_low, ps_median, ps_high, pe_low, pe_median, pe_high,
                        blend_ps_weight, blend_pe_weight
                     ) VALUES (
                        {scenario_id}, {period_order}, 'Q{period_order}',
                        '{period_end}', 'quarter',
                        0.02, 100000000.0, 0.20,
                        {ps_vals}, 0.5, 0.5
                     )"
                ),
            )
            .await?;
        }
        execute_sql(
            db,
            &format!(
                "INSERT INTO scenario_crux_assumptions (
                    scenario_id, crux_order, crux_key, crux, assumption
                 ) VALUES ({scenario_id}, 1, 'fixture_crux', 'Fixture crux', 'Resolved in scenario')"
            ),
        )
        .await?;
        execute_sql(
            db,
            &format!(
                "INSERT INTO scenario_sensitivities (scenario_id, sensitivity_order, body)
                 VALUES ({scenario_id}, 1, 'Revenue growth ±200bps')"
            ),
        )
        .await?;
        execute_sql(
            db,
            &format!(
                "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
                 VALUES ({scenario_id}, 1, 'confirming', 'Fixture confirming signal')"
            ),
        )
        .await?;
        execute_sql(
            db,
            &format!(
                "INSERT INTO scenario_signals (scenario_id, signal_order, signal_type, body)
                 VALUES ({scenario_id}, 1, 'breaking', 'Fixture breaking signal')"
            ),
        )
        .await?;
    }

    Ok(())
}
