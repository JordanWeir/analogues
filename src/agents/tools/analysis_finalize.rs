use crate::{
    agents::financial_model_explorer::{
        FinancialModelExplorerAgent, types::{AnalysisExperimentInput, AnalysisFinalizeInput},
    },
    services::financial_analysis_store::{AnalysisRunRecord, FinancialAnalysisStore},
};
use chrono::Utc;
use loco_rs::prelude::*;
use openrouter_rs::types::Tool;
use serde_json::json;
use std::path::PathBuf;

pub const TOOL_NAME: &str = "finalize_analysis";

fn experiment_parameters_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "required": ["experiment_key", "question", "purpose", "period_basis", "disposition"],
        "properties": {
            "experiment_key": {
                "type": "string",
                "description": "Stable key for this experiment; upserted on conflict."
            },
            "question": {
                "type": "string",
                "description": "Focused falsifiable question the SQL answers."
            },
            "purpose": {
                "type": "string",
                "enum": [
                    "historical_investigation",
                    "sensitivity",
                    "forward_projection",
                    "scenario_validation"
                ],
                "description": "Experiment intent category — not free text."
            },
            "sql_body": {
                "type": "string",
                "description": "Executed SQL for the experiment. Omit to reuse the draft run's executed_sql."
            },
            "period_basis": {
                "type": "string",
                "description": "One of: quarter | ytd | annual | instant. Omit to reuse the draft run value."
            },
            "crux_key": { "type": "string" },
            "disposition": {
                "type": "string",
                "enum": ["candidate", "promoted", "rejected", "background"]
            },
            "rejection_reason": {
                "type": "string",
                "description": "Required when disposition is rejected."
            },
            "source_note": { "type": "string" },
            "rationale": { "type": "string" },
            "assumptions": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["key", "value"],
                    "properties": {
                        "key": { "type": "string" },
                        "value": { "type": "string" },
                        "note": { "type": "string" }
                    }
                }
            },
            "inputs": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["input_type"],
                    "properties": {
                        "input_type": { "type": "string" },
                        "taxonomy": { "type": "string" },
                        "concept_name": { "type": "string" },
                        "unit": { "type": "string" },
                        "canonical_key": { "type": "string" },
                        "note": { "type": "string" }
                    }
                }
            },
            "outputs": {
                "type": "array",
                "description": "Promoted experiments require at least one arithmetic/ratio row and one interpretation row.",
                "items": {
                    "type": "object",
                    "required": ["kind", "label"],
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": [
                                "ratio",
                                "arithmetic",
                                "series_point",
                                "bridge_step",
                                "interpretation"
                            ]
                        },
                        "label": { "type": "string" },
                        "value": { "type": "number" },
                        "unit": { "type": "string" },
                        "period_end": { "type": "string" },
                        "formula": { "type": "string" },
                        "text": {
                            "type": "string",
                            "description": "Required for kind interpretation."
                        }
                    }
                }
            },
            "bridge": {
                "type": "object",
                "required": ["archetype", "driver", "mechanism", "outcome", "conclusion"],
                "properties": {
                    "archetype": { "type": "string" },
                    "driver": { "type": "string" },
                    "mechanism": { "type": "string" },
                    "outcome": { "type": "string" },
                    "conclusion": { "type": "string" }
                }
            }
        }
    })
}

pub fn openrouter_tool() -> Tool {
    Tool::builder()
        .name(TOOL_NAME)
        .description(
            "Judge a draft analysis run and persist a finalized experiment record. \
             Use only after reviewing run_analysis_draft results. \
             sql_body and period_basis default from the draft run when omitted.",
        )
        .parameters(json!({
            "type": "object",
            "properties": {
                "run_key": {
                    "type": "string",
                    "description": "run_key from a successful run_analysis_draft call."
                },
                "experiment": experiment_parameters_schema()
            },
            "required": ["run_key", "experiment"]
        }))
        .build()
        .expect("finalize_analysis tool definition should be valid")
}

pub(crate) fn backfill_experiment_from_run(
    experiment: &mut AnalysisExperimentInput,
    run: &AnalysisRunRecord,
) {
    if experiment.sql_body.trim().is_empty() {
        experiment.sql_body = run.executed_sql.clone();
    }
    if experiment.period_basis.trim().is_empty() {
        experiment.period_basis = run.period_basis.clone();
    }
}

pub async fn execute(sqlite_path: &PathBuf, arguments: &str) -> Result<String> {
    let mut input: AnalysisFinalizeInput = serde_json::from_str(arguments).map_err(|err| {
        Error::string(&format!("finalize_analysis arguments were not valid JSON: {err}"))
    })?;
    if input.run_key.trim().is_empty() {
        return Err(Error::string("run_key cannot be empty"));
    }

    let db = sea_orm::Database::connect(crate::services::workspace_store::sqlite_uri(
        sqlite_path,
    ))
    .await?;
    let store = FinancialAnalysisStore::new(&db);
    let run = store
        .load_analysis_run(&input.run_key)
        .await?
        .ok_or_else(|| Error::string(&format!("unknown analysis run_key: {}", input.run_key)))?;

    backfill_experiment_from_run(&mut input.experiment, &run);
    FinancialModelExplorerAgent::validate_experiment_input(&input.experiment)?;

    if run.status != "draft" {
        return Err(Error::string(&format!(
            "analysis run {} is not in draft status",
            input.run_key
        )));
    }
    if run.execution_status == "error" && input.experiment.disposition == "promoted" {
        return Err(Error::string(
            "cannot promote an experiment when the draft run errored",
        ));
    }

    let created_at = Utc::now().to_rfc3339();
    store
        .finalize_analysis_run(
            &input.run_key,
            &input.experiment,
            "financial_model_explorer",
            &created_at,
            None,
        )
        .await?;
    db.close().await.ok();

    Ok(json!({
        "run_key": input.run_key,
        "experiment_key": input.experiment.experiment_key,
        "disposition": input.experiment.disposition,
        "status": "finalized"
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::financial_model_explorer::types::AnalysisExperimentInput;

    #[test]
    fn backfills_sql_and_period_from_draft_run() {
        let run = AnalysisRunRecord {
            run_key: "draft_run".to_string(),
            status: "draft".to_string(),
            execution_status: "success".to_string(),
            executed_sql: "SELECT 1".to_string(),
            period_basis: "annual".to_string(),
        };
        let mut experiment = AnalysisExperimentInput {
            experiment_key: "capex_ocf".to_string(),
            question: "Does capex exceed OCF?".to_string(),
            purpose: "historical_investigation".to_string(),
            crux_key: None,
            sql_body: String::new(),
            period_basis: String::new(),
            disposition: "promoted".to_string(),
            rejection_reason: None,
            source_note: None,
            rationale: None,
            assumptions: vec![],
            inputs: vec![],
            outputs: vec![],
            bridge: None,
        };

        backfill_experiment_from_run(&mut experiment, &run);

        assert_eq!(experiment.sql_body, "SELECT 1");
        assert_eq!(experiment.period_basis, "annual");
    }
}
