use crate::services::workspace_sql::scalar_i64;
use loco_rs::prelude::*;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ScenarioWorkspaceContext {
    pub promoted_crux_count: i64,
    pub promoted_experiment_count: i64,
    pub av_quarter_count: i64,
    pub claims_count: i64,
    pub crux_summary: String,
    pub experiment_summary: String,
    pub av_coverage_summary: String,
    pub blueprint_summary: String,
    pub sources_summary: String,
}

pub async fn load_scenario_context(sqlite_path: &Path) -> Result<ScenarioWorkspaceContext> {
    let db = sea_orm::Database::connect(crate::services::workspace_store::sqlite_uri(
        sqlite_path,
    ))
    .await?;
    let ctx = load_scenario_context_from_db(&db).await?;
    db.close().await.ok();
    Ok(ctx)
}

pub async fn load_scenario_context_from_db(
    db: &sea_orm::DatabaseConnection,
) -> Result<ScenarioWorkspaceContext> {
    let promoted_crux_count = scalar_i64(
        db,
        "SELECT COUNT(*) AS count FROM crux_candidates WHERE disposition = 'promoted' AND status = 'active'",
    )
    .await?;
    let promoted_experiment_count = scalar_i64(
        db,
        "SELECT COUNT(*) AS count FROM analysis_experiments WHERE disposition = 'promoted'",
    )
    .await?;
    let av_quarter_count = scalar_i64(
        db,
        "SELECT COUNT(*) AS count FROM av_raw_facts WHERE report_type = 'quarterly' AND period_type = 'quarter'",
    )
    .await?;
    let claims_count = scalar_i64(db, "SELECT COUNT(*) AS count FROM claims").await?;

    Ok(ScenarioWorkspaceContext {
        promoted_crux_count,
        promoted_experiment_count,
        av_quarter_count,
        claims_count,
        crux_summary: summarize_cruxes(db).await?,
        experiment_summary: summarize_experiments(db).await?,
        av_coverage_summary: summarize_av_coverage(db).await?,
        blueprint_summary: summarize_blueprint(db).await?,
        sources_summary: summarize_sources(db).await?,
    })
}

pub fn format_scenario_context_section(ctx: &ScenarioWorkspaceContext) -> String {
    format!(
        "Promoted cruxes: {promoted_crux_count}\n\
         Promoted experiments: {promoted_experiment_count}\n\
         AV quarterly facts: {av_quarter_count}\n\
         Claims: {claims_count}\n\n\
         Crux board:\n{crux_summary}\n\n\
         Key experiments:\n{experiment_summary}\n\n\
         AV quarterly coverage:\n{av_coverage_summary}\n\n\
         Sources board (reuse id in crux_assumptions.source_id):\n{sources_summary}\n\n\
         Scenario blueprint (if present):\n{blueprint_summary}",
        promoted_crux_count = ctx.promoted_crux_count,
        promoted_experiment_count = ctx.promoted_experiment_count,
        av_quarter_count = ctx.av_quarter_count,
        claims_count = ctx.claims_count,
        crux_summary = ctx.crux_summary,
        experiment_summary = ctx.experiment_summary,
        av_coverage_summary = ctx.av_coverage_summary,
        sources_summary = ctx.sources_summary,
        blueprint_summary = ctx.blueprint_summary,
    )
}

async fn summarize_cruxes(db: &sea_orm::DatabaseConnection) -> Result<String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT crux_key, title, bridge_archetype FROM crux_candidates
             WHERE disposition = 'promoted' AND status = 'active'
             ORDER BY id LIMIT 15"
                .to_string(),
        ))
        .await
        .map_err(|e| Error::string(&format!("crux summary: {e}")))?;
    if rows.is_empty() {
        return Ok("(none)".to_string());
    }
    rows.into_iter()
        .map(|row| {
            let key: String = row.try_get_by_index(0).map_err(|e| Error::string(&e.to_string()))?;
            let title: String = row.try_get_by_index(1).map_err(|e| Error::string(&e.to_string()))?;
            let archetype: Option<String> = row
                .try_get_by_index(2)
                .map_err(|e| Error::string(&e.to_string()))?;
            Ok(format!(
                "- {key}: {title}{}",
                archetype
                    .filter(|a| !a.is_empty())
                    .map(|a| format!(" [{a}]"))
                    .unwrap_or_default()
            ))
        })
        .collect::<Result<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}

async fn summarize_experiments(db: &sea_orm::DatabaseConnection) -> Result<String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT ae.experiment_key, ae.purpose, cc.crux_key
             FROM analysis_experiments ae
             LEFT JOIN crux_candidates cc ON cc.id = ae.crux_id
             WHERE ae.disposition = 'promoted'
               AND ae.purpose IN ('forward_projection', 'sensitivity', 'scenario_validation')
             ORDER BY ae.id LIMIT 12"
                .to_string(),
        ))
        .await
        .map_err(|e| Error::string(&format!("experiment summary: {e}")))?;
    if rows.is_empty() {
        return Ok("(no forward/sensitivity experiments)".to_string());
    }
    rows.into_iter()
        .map(|row| {
            let key: String = row.try_get_by_index(0).map_err(|e| Error::string(&e.to_string()))?;
            let purpose: String = row.try_get_by_index(1).map_err(|e| Error::string(&e.to_string()))?;
            let crux: Option<String> = row.try_get_by_index(2).map_err(|e| Error::string(&e.to_string()))?;
            Ok(format!(
                "- {key} ({purpose}){}",
                crux.map(|c| format!(" → {c}")).unwrap_or_default()
            ))
        })
        .collect::<Result<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}

async fn summarize_av_coverage(db: &sea_orm::DatabaseConnection) -> Result<String> {
    let row = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(DISTINCT period_end) AS quarters,
                    MIN(period_end) AS earliest,
                    MAX(period_end) AS latest
             FROM av_raw_facts
             WHERE report_type = 'quarterly' AND period_type = 'quarter'
               AND field_name = 'totalRevenue'"
                .to_string(),
        ))
        .await
        .map_err(|e| Error::string(&format!("av coverage: {e}")))?;
    match row {
        Some(row) => {
            let quarters: i64 = row.try_get_by_index(0).unwrap_or(0);
            let earliest: Option<String> = row.try_get_by_index(1).ok();
            let latest: Option<String> = row.try_get_by_index(2).ok();
            Ok(format!(
                "totalRevenue quarters: {quarters} ({earliest} → {latest})",
                earliest = earliest.unwrap_or_else(|| "?".to_string()),
                latest = latest.unwrap_or_else(|| "?".to_string()),
            ))
        }
        None => Ok("(no AV quarterly revenue)".to_string()),
    }
}

async fn summarize_blueprint(db: &sea_orm::DatabaseConnection) -> Result<String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT scenario_key, name, stance, probability
             FROM scenario_assumptions ORDER BY scenario_order"
                .to_string(),
        ))
        .await
        .map_err(|e| Error::string(&format!("blueprint summary: {e}")))?;
    if rows.is_empty() {
        return Ok("(blueprint not submitted yet)".to_string());
    }
    rows.into_iter()
        .map(|row| {
            let key: String = row.try_get_by_index(0).map_err(|e| Error::string(&e.to_string()))?;
            let name: String = row.try_get_by_index(1).map_err(|e| Error::string(&e.to_string()))?;
            let stance: String = row.try_get_by_index(2).map_err(|e| Error::string(&e.to_string()))?;
            let prob: Option<f64> = row.try_get_by_index(3).map_err(|e| Error::string(&e.to_string()))?;
            Ok(format!(
                "- {key} [{stance}] p={}: {name}",
                prob.map(|p| format!("{p:.2}")).unwrap_or_else(|| "?".to_string()),
            ))
        })
        .collect::<Result<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}

async fn summarize_sources(db: &sea_orm::DatabaseConnection) -> Result<String> {
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id, title, source_type FROM sources ORDER BY id LIMIT 20".to_string(),
        ))
        .await
        .map_err(|e| Error::string(&format!("sources summary: {e}")))?;
    if rows.is_empty() {
        return Ok("(none — omit source_id on crux_assumptions)".to_string());
    }
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get_by_index(0).map_err(|e| Error::string(&e.to_string()))?;
            let title: String = row.try_get_by_index(1).map_err(|e| Error::string(&e.to_string()))?;
            let source_type: Option<String> = row.try_get_by_index(2).ok();
            Ok(format!(
                "- id={id}: {title}{}",
                source_type
                    .filter(|value| !value.is_empty())
                    .map(|value| format!(" [{value}]"))
                    .unwrap_or_default()
            ))
        })
        .collect::<Result<Vec<_>>>()
        .map(|lines| lines.join("\n"))
}
