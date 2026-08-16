use crate::agents::financial_model_explorer::golden_path::explorer_schema_hint;

pub const PREAMBLE: &str = "You are the Financial Mechanics Validator. Review promoted mechanics experiments like a pull-request reviewer: stamp each scope as approved or changes_requested. Use workspace_sql to re-run experiment SQL, compare arithmetic outputs to sec_raw_facts and claims, and check for SEC staleness notes when facts lag narrative guidance. Be strict on arithmetic mistakes, orphan drafts, and promoted experiments that lack both arithmetic and interpretation outputs. When finished, call submit_mechanics_review — do not end with a plain assistant message. Fix validation errors and resubmit.";

pub fn golden_path() -> &'static str {
    r#"Golden path — mechanics review (PR-style):

1. Load promoted analysis_experiments for your assigned scope (one crux_key or scout gaps).
2. For each experiment, inspect sql_body, inputs_json, outputs_json, assumptions_json, and linked analysis_runs.
3. Re-run SQL with workspace_sql when needed to verify arithmetic and period_basis.
4. Compare key outputs against sec_raw_facts and relevant claims; flag stale SEC inputs missing disclosure.
5. Check for unfinalized analysis_runs drafts in scope — these block approval.
6. Stamp verdict:
   - approved: experiments are arithmetically sound, sourced, and scope is clean
   - changes_requested: blocking issues remain; include remediation guidance per finding
7. List every promoted experiment_key in experiments_reviewed."#
}

pub fn submit_example() -> &'static str {
    r#"{
  "summary": "Two promoted experiments verified against SEC facts; staleness noted appropriately.",
  "per_worker": true,
  "crux_key": "capex_roic_stranding",
  "scout": false,
  "verdict": "approved",
  "findings": [],
  "experiments_reviewed": ["capex_ocf_ratio", "capex_roic_forward_sensitivity"]
}"#
}

pub fn changes_requested_example() -> &'static str {
    r#"{
  "summary": "Forward projection uses stale OCF baseline without adequate disclosure.",
  "per_worker": true,
  "crux_key": "self_fund_ai_buildout",
  "scout": false,
  "verdict": "changes_requested",
  "findings": [{
    "category": "stale_inputs",
    "severity": "blocking",
    "description": "OCF baseline $23B conflicts with latest SEC operating cash flow.",
    "experiment_key": "fy2027_funding_gap_projection",
    "remediation": "Re-run SQL with latest SEC OCF or document staleness in assumptions and interpretation."
  }],
  "experiments_reviewed": ["fy2027_funding_gap_projection", "ocf_capex_coverage"]
}"#
}

pub fn build_user_prompt(
    company_label: Option<&str>,
    focus_prefix: Option<&str>,
    review_round: i64,
) -> String {
    let company = company_label
        .map(|label| format!("Company: {label}\n\n"))
        .unwrap_or_default();
    let focus = focus_prefix
        .map(|prefix| format!("{prefix}\n\n"))
        .unwrap_or_default();

    format!(
        r#"{company}{focus}{schema}

Review round: {review_round}

{golden_path}

Approved example:
{approved_example}

Changes requested example:
{changes_example}

Call submit_mechanics_review with per_worker true and your assigned crux_key or scout true."#,
        schema = explorer_schema_hint(),
        golden_path = golden_path(),
        approved_example = submit_example(),
        changes_example = changes_requested_example(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_submit_tool_and_verdicts() {
        let prompt = build_user_prompt(Some("ORCL"), None, 1);
        assert!(prompt.contains("submit_mechanics_review"));
        assert!(prompt.contains("changes_requested"));
        assert!(prompt.contains("approved"));
    }
}
