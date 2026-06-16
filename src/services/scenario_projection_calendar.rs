//! Shared quarterly projection calendar for scenario detail workers.
//! Built deterministically from AlphaVantage quarterly period ends so every scenario
//! shares the same historical anchor and terminal endpoint.

use crate::{
    agents::scenario_builder::types::{
        ScenarioProjectionCalendarSpec, FORWARD_QUARTERS_MAX, FORWARD_QUARTERS_MIN,
        HISTORICAL_QUARTERS_TARGET, MIN_TOTAL_QUARTERLY_PERIODS,
    },
    services::workspace_sql::execute_sql,
};
use chrono::{Datelike, NaiveDate};
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionPeriod {
    pub period_order: i64,
    pub period_end: String,
    pub is_historical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioProjectionCalendar {
    pub historical_quarters: usize,
    pub forward_quarters: usize,
    pub historical_anchor_end: String,
    pub terminal_period_end: String,
    pub periods: Vec<ProjectionPeriod>,
}

impl ScenarioProjectionCalendar {
    pub fn total_periods(&self) -> usize {
        self.periods.len()
    }

    pub fn period_end_for_order(&self, period_order: i64) -> Option<&str> {
        self.periods
            .iter()
            .find(|period| period.period_order == period_order)
            .map(|period| period.period_end.as_str())
    }
}

pub fn validate_projection_calendar_spec(spec: &ScenarioProjectionCalendarSpec) -> Result<()> {
    if spec.forward_quarters < FORWARD_QUARTERS_MIN || spec.forward_quarters > FORWARD_QUARTERS_MAX {
        return Err(Error::string(&format!(
            "projection_calendar.forward_quarters must be {FORWARD_QUARTERS_MIN}–{FORWARD_QUARTERS_MAX}, got {}",
            spec.forward_quarters
        )));
    }
    let historical = spec.historical_quarters();
    if historical == 0 || historical > HISTORICAL_QUARTERS_TARGET + 2 {
        return Err(Error::string(&format!(
            "projection_calendar.historical_quarters must be 1–{}, got {historical}",
            HISTORICAL_QUARTERS_TARGET + 2
        )));
    }
    let total = historical + spec.forward_quarters;
    if total < MIN_TOTAL_QUARTERLY_PERIODS {
        return Err(Error::string(&format!(
            "projection calendar needs at least {MIN_TOTAL_QUARTERLY_PERIODS} total quarters \
             (historical {historical} + forward {}), got {total}",
            spec.forward_quarters
        )));
    }
    Ok(())
}

pub async fn build_from_av(
    db: &impl ConnectionTrait,
    spec: &ScenarioProjectionCalendarSpec,
) -> Result<ScenarioProjectionCalendar> {
    validate_projection_calendar_spec(spec)?;
    let historical_quarters = spec.historical_quarters();
    let av_period_ends = load_av_quarterly_period_ends(db).await?;
    if av_period_ends.len() < historical_quarters {
        return Err(Error::string(&format!(
            "need at least {historical_quarters} AV quarterly period_end values for totalRevenue, got {}",
            av_period_ends.len()
        )));
    }

    let historical: Vec<String> = av_period_ends
        .iter()
        .rev()
        .take(historical_quarters)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let mut period_ends = historical.clone();
    let step = quarter_step_days(&period_ends)?;
    let mut cursor = parse_period_end(period_ends.last().expect("historical non-empty"))?;
    for _ in 0..spec.forward_quarters {
        cursor = advance_quarter_end(cursor, step);
        period_ends.push(format_period_end(cursor));
    }

    let periods = period_ends
        .iter()
        .enumerate()
        .map(|(index, period_end)| ProjectionPeriod {
            period_order: index as i64 + 1,
            period_end: period_end.clone(),
            is_historical: index < historical_quarters,
        })
        .collect::<Vec<_>>();

    Ok(ScenarioProjectionCalendar {
        historical_quarters,
        forward_quarters: spec.forward_quarters,
        historical_anchor_end: historical
            .last()
            .cloned()
            .expect("historical non-empty"),
        terminal_period_end: period_ends
            .last()
            .cloned()
            .expect("forward quarters non-empty"),
        periods,
    })
}

pub async fn load_av_quarterly_period_ends(
    db: &impl ConnectionTrait,
) -> Result<Vec<String>> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT DISTINCT period_end
             FROM av_raw_facts
             WHERE report_type = 'quarterly'
               AND period_type = 'quarter'
               AND field_name = 'totalRevenue'
               AND period_end IS NOT NULL
               AND TRIM(period_end) != ''
             ORDER BY period_end ASC"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load AV quarterly period ends: {err}")))?;

    rows.into_iter()
        .map(|row| {
            row.try_get_by_index::<String>(0)
                .map_err(|err| Error::string(&format!("read period_end: {err}")))
        })
        .collect()
}

fn quarter_step_days(period_ends: &[String]) -> Result<i64> {
    if period_ends.len() < 2 {
        return Ok(91);
    }
    let prev = parse_period_end(&period_ends[period_ends.len() - 2])?;
    let last = parse_period_end(period_ends.last().expect("len >= 2"))?;
    let step = (last - prev).num_days();
    if !(80..=100).contains(&step) {
        return Err(Error::string(&format!(
            "unexpected quarter spacing between {} and {}: {step} days",
            period_ends[period_ends.len() - 2],
            period_ends.last().expect("len >= 2"),
        )));
    }
    Ok(step)
}

fn advance_quarter_end(date: NaiveDate, step_days: i64) -> NaiveDate {
    let candidate = date + chrono::Duration::days(step_days);
    if candidate.day() != date.day() {
        let month = candidate.month();
        let year = candidate.year();
        let day = date.day().min(last_day_of_month(year, month));
        return NaiveDate::from_ymd_opt(year, month, day).unwrap_or(candidate);
    }
    candidate
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_of_next = NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("valid month");
    first_of_next.pred_opt().expect("previous day").day()
}

fn parse_period_end(value: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|err| Error::string(&format!("invalid period_end '{value}': {err}")))
}

fn format_period_end(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub async fn persist_calendar(
    db: &impl ConnectionTrait,
    calendar: &ScenarioProjectionCalendar,
) -> Result<()> {
    execute_sql(db, "DELETE FROM scenario_projection_periods").await?;
    execute_sql(db, "DELETE FROM scenario_projection_config").await?;
    execute_sql(
        db,
        &format!(
            "INSERT INTO scenario_projection_config (
                id, historical_quarters, forward_quarters, historical_anchor_end, terminal_period_end
             ) VALUES (
                1, {}, {}, {}, {}
             )",
            calendar.historical_quarters as i64,
            calendar.forward_quarters as i64,
            sql_quote(&calendar.historical_anchor_end),
            sql_quote(&calendar.terminal_period_end),
        ),
    )
    .await?;

    for period in &calendar.periods {
        execute_sql(
            db,
            &format!(
                "INSERT INTO scenario_projection_periods (period_order, period_end, is_historical)
                 VALUES ({}, {}, {})",
                period.period_order,
                sql_quote(&period.period_end),
                if period.is_historical { 1 } else { 0 },
            ),
        )
        .await?;
    }

    Ok(())
}

pub async fn load_calendar(db: &impl ConnectionTrait) -> Result<Option<ScenarioProjectionCalendar>> {
    let config = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT historical_quarters, forward_quarters, historical_anchor_end, terminal_period_end
             FROM scenario_projection_config
             WHERE id = 1"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load projection config: {err}")))?;

    let Some(config) = config else {
        return Ok(None);
    };

    let historical_quarters: i64 = config
        .try_get_by_index(0)
        .map_err(|err| Error::string(&format!("historical_quarters: {err}")))?;
    let forward_quarters: i64 = config
        .try_get_by_index(1)
        .map_err(|err| Error::string(&format!("forward_quarters: {err}")))?;
    let historical_anchor_end: String = config
        .try_get_by_index(2)
        .map_err(|err| Error::string(&format!("historical_anchor_end: {err}")))?;
    let terminal_period_end: String = config
        .try_get_by_index(3)
        .map_err(|err| Error::string(&format!("terminal_period_end: {err}")))?;

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT period_order, period_end, is_historical
             FROM scenario_projection_periods
             ORDER BY period_order ASC"
                .to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("load projection periods: {err}")))?;

    let periods = rows
        .into_iter()
        .map(|row| {
            let period_order: i64 = row
                .try_get_by_index(0)
                .map_err(|err| Error::string(&format!("period_order: {err}")))?;
            let period_end: String = row
                .try_get_by_index(1)
                .map_err(|err| Error::string(&format!("period_end: {err}")))?;
            let is_historical: i64 = row
                .try_get_by_index(2)
                .map_err(|err| Error::string(&format!("is_historical: {err}")))?;
            Ok(ProjectionPeriod {
                period_order,
                period_end,
                is_historical: is_historical != 0,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(ScenarioProjectionCalendar {
        historical_quarters: historical_quarters as usize,
        forward_quarters: forward_quarters as usize,
        historical_anchor_end,
        terminal_period_end,
        periods,
    }))
}

pub async fn has_calendar(db: &impl ConnectionTrait) -> Result<bool> {
    let count = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM scenario_projection_config WHERE id = 1".to_string(),
        ))
        .await
        .map_err(|err| Error::string(&format!("count projection config: {err}")))?;
    Ok(count
        .and_then(|row| row.try_get_by_index::<i64>(0).ok())
        .unwrap_or(0)
        > 0)
}

pub fn validate_detail_periods(
    calendar: &ScenarioProjectionCalendar,
    periods: &[crate::agents::scenario_builder::types::ScenarioPeriodInput],
) -> Result<()> {
    if periods.len() != calendar.periods.len() {
        return Err(Error::string(&format!(
            "scenario detail must include exactly {} quarterly periods aligned to the blueprint calendar, got {}",
            calendar.periods.len(),
            periods.len()
        )));
    }

    for expected in &calendar.periods {
        let actual = periods
            .iter()
            .find(|period| period.period_order == expected.period_order)
            .ok_or_else(|| {
                Error::string(&format!(
                    "missing period_order {} from blueprint projection calendar",
                    expected.period_order
                ))
            })?;
        if actual.period_end != expected.period_end {
            return Err(Error::string(&format!(
                "period_order {} must use period_end {} per blueprint calendar, got {}",
                expected.period_order, expected.period_end, actual.period_end
            )));
        }
    }

    Ok(())
}

pub fn format_calendar_summary(calendar: &ScenarioProjectionCalendar) -> String {
    let historical: Vec<_> = calendar
        .periods
        .iter()
        .filter(|period| period.is_historical)
        .map(|period| format!("{}={}", period.period_order, period.period_end))
        .collect();
    let forward_preview: Vec<_> = calendar
        .periods
        .iter()
        .filter(|period| !period.is_historical)
        .take(3)
        .map(|period| format!("{}={}", period.period_order, period.period_end))
        .collect();
    format!(
        "Historical quarters: {} (anchor end {})\n\
         Forward quarters: {} (terminal {})\n\
         Historical period_order → period_end: {}\n\
         Forward preview: {} … {}\n\
         Detail workers MUST use these exact period_end values for each period_order.",
        calendar.historical_quarters,
        calendar.historical_anchor_end,
        calendar.forward_quarters,
        calendar.terminal_period_end,
        historical.join(", "),
        forward_preview.join(", "),
        calendar.terminal_period_end,
    )
}

fn sql_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agents::scenario_builder::types::ScenarioPeriodInput,
        services::workspace_store::execute_schema,
    };
    use sea_orm::Database;

    fn sample_av_periods() -> Vec<&'static str> {
        vec![
            "2024-02-29",
            "2024-05-31",
            "2024-08-31",
            "2024-11-30",
            "2025-02-28",
            "2025-05-31",
        ]
    }

    async fn seed_av_periods(db: &sea_orm::DatabaseConnection) {
        for period_end in sample_av_periods() {
            execute_sql(
                db,
                &format!(
                    "INSERT INTO av_raw_facts (
                        endpoint, field_name, report_type, period_type, period_end, metric_value, unit, raw_json, fetched_at
                     ) VALUES (
                        'income', 'totalRevenue', 'quarterly', 'quarter', '{period_end}', 1.0, 'USD', '{{}}', '2026-01-01'
                     )"
                ),
            )
            .await
            .expect("seed av");
        }
    }

    #[tokio::test]
    async fn builds_aligned_calendar_from_av() {
        let path = std::env::temp_dir().join(format!(
            "analogues-projection-calendar-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::connect(crate::services::workspace_store::sqlite_uri(&path))
            .await
            .expect("sqlite");
        execute_schema(&db).await.expect("schema");
        seed_av_periods(&db).await;

        let spec = ScenarioProjectionCalendarSpec {
            forward_quarters: 12,
            historical_quarters: Some(4),
        };
        let calendar = build_from_av(&db, &spec).await.expect("calendar");
        assert_eq!(calendar.total_periods(), 16);
        assert_eq!(calendar.historical_anchor_end, "2025-05-31");
        assert_eq!(
            calendar.periods.first().map(|p| p.period_end.as_str()),
            Some("2024-08-31")
        );
        assert_eq!(calendar.terminal_period_end, calendar.periods.last().unwrap().period_end);
    }

    #[tokio::test]
    async fn rejects_misaligned_detail_periods() {
        let calendar = ScenarioProjectionCalendar {
            historical_quarters: 1,
            forward_quarters: 1,
            historical_anchor_end: "2025-05-31".to_string(),
            terminal_period_end: "2025-08-31".to_string(),
            periods: vec![
                ProjectionPeriod {
                    period_order: 1,
                    period_end: "2025-05-31".to_string(),
                    is_historical: true,
                },
                ProjectionPeriod {
                    period_order: 2,
                    period_end: "2025-08-31".to_string(),
                    is_historical: false,
                },
            ],
        };
        let periods = vec![
            ScenarioPeriodInput {
                period_order: 1,
                label: "Q1".to_string(),
                period_end: "2025-05-31".to_string(),
                period_type: "quarter".to_string(),
                revenue: Some(1.0),
                revenue_growth: None,
                diluted_shares: None,
                gross_margin: None,
                operating_margin: None,
                net_margin: None,
                net_income: None,
                eps: None,
                ps_low: None,
                ps_median: None,
                ps_high: None,
                pe_low: None,
                pe_median: None,
                pe_high: None,
                blend_ps_weight: 0.5,
                blend_pe_weight: 0.5,
                source_note: None,
            },
            ScenarioPeriodInput {
                period_order: 2,
                label: "Q2".to_string(),
                period_end: "2025-11-30".to_string(),
                period_type: "quarter".to_string(),
                revenue_growth: Some(0.1),
                revenue: None,
                diluted_shares: None,
                gross_margin: None,
                operating_margin: None,
                net_margin: None,
                net_income: None,
                eps: None,
                ps_low: None,
                ps_median: Some(5.0),
                ps_high: None,
                pe_low: None,
                pe_median: None,
                pe_high: None,
                blend_ps_weight: 0.5,
                blend_pe_weight: 0.5,
                source_note: None,
            },
        ];
        let err = validate_detail_periods(&calendar, &periods).expect_err("misaligned");
        assert!(err.to_string().contains("period_end"));
    }
}
