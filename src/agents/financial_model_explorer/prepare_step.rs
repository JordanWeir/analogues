use crate::services::financial_analysis_store::AnalysisDraftSummary;
use super::explorer_context::{
    format_draft_hygiene_prepare_message, load_draft_summaries_for_prepare,
};
use crate::services::tool_loop_control::{
    apply_mechanics_step_budget_prepare, PrepareStepContext, PrepareStepHook, PrepareStepResult,
};
use openrouter_rs::api::chat::Message;
use openrouter_rs::types::Role;
use std::{path::Path, path::PathBuf, sync::Arc};

fn load_drafts_for_prepare_step(
    sqlite_path: &Path,
    focus_crux_key: Option<&str>,
    scout_worker: bool,
) -> Vec<AnalysisDraftSummary> {
    let path = sqlite_path.to_path_buf();
    let focus = focus_crux_key.map(str::to_string);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("draft prepare runtime");
        rt.block_on(async move {
            load_draft_summaries_for_prepare(&path, focus.as_deref(), scout_worker).await
        })
    })
    .join()
    .unwrap_or_default()
}

/// Mechanics step budget: penultimate turn finalizes drafts; final turn submits.
#[derive(Debug, Clone)]
pub struct MechanicsStepBudgetPrepareStep;

impl PrepareStepHook for MechanicsStepBudgetPrepareStep {
    fn prepare_step(&self, ctx: PrepareStepContext<'_>) -> PrepareStepResult {
        apply_mechanics_step_budget_prepare(&ctx)
    }
}

/// Reminds the model about open drafts scoped to this worker's crux (or scout gaps).
#[derive(Clone)]
pub struct MechanicsDraftPrepareStep {
    sqlite_path: PathBuf,
    focus_crux_key: Option<String>,
    scout_worker: bool,
}

impl MechanicsDraftPrepareStep {
    pub fn new(
        sqlite_path: PathBuf,
        focus_crux_key: Option<String>,
        scout_worker: bool,
    ) -> Self {
        Self {
            sqlite_path,
            focus_crux_key,
            scout_worker,
        }
    }
}

impl PrepareStepHook for MechanicsDraftPrepareStep {
    fn prepare_step(&self, ctx: PrepareStepContext<'_>) -> PrepareStepResult {
        if ctx.step_number == 0 || ctx.step_number >= ctx.max_steps {
            return PrepareStepResult::default();
        }

        let sqlite_path = self.sqlite_path.clone();
        let focus_crux_key = self.focus_crux_key.clone();
        let scout_worker = self.scout_worker;
        let drafts = load_drafts_for_prepare_step(
            &sqlite_path,
            focus_crux_key.as_deref(),
            scout_worker,
        );
        if drafts.is_empty() {
            return PrepareStepResult::default();
        }

        let mut messages = ctx.messages.to_vec();
        messages.push(Message::new(
            Role::User,
            format_draft_hygiene_prepare_message(&drafts).as_str(),
        ));
        PrepareStepResult {
            messages: Some(messages),
            ..Default::default()
        }
    }
}

pub fn mechanics_prepare_step_chain(
    sqlite_path: PathBuf,
    focus_crux_key: Option<String>,
    scout_worker: bool,
) -> Arc<dyn PrepareStepHook> {
    Arc::new(crate::services::tool_loop_control::ChainedPrepareStep::new(vec![
        Arc::new(MechanicsStepBudgetPrepareStep),
        Arc::new(MechanicsDraftPrepareStep::new(
            sqlite_path,
            focus_crux_key,
            scout_worker,
        )),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::tool_loop_control::agent_mechanics_step_budget_message;
    use openrouter_rs::api::chat::Message;
    use openrouter_rs::types::Role;

    #[test]
    fn mechanics_budget_penultimate_nudges_finalize_not_submit() {
        let message = agent_mechanics_step_budget_message(34, 36, 2);
        assert!(message.contains("finalize_analysis"));
        assert!(!message.contains("submit_mechanics_experiments now"));
    }

    #[test]
    fn mechanics_budget_final_turn_nudges_submit() {
        let message = agent_mechanics_step_budget_message(35, 36, 1);
        assert!(message.contains("submit_mechanics_experiments"));
    }

    #[tokio::test]
    async fn draft_prepare_step_does_not_panic_inside_runtime() {
        let hook = MechanicsDraftPrepareStep::new(
            PathBuf::from("/tmp/nonexistent-draft-prepare-test.sqlite"),
            Some("test_crux".to_string()),
            false,
        );
        let messages = vec![Message::new(Role::User, "hello")];
        let ctx = crate::services::tool_loop_control::PrepareStepContext {
            step_number: 1,
            max_steps: 36,
            steps: &[],
            messages: &messages,
            model: "test/model",
        };
        let _ = hook.prepare_step(ctx);
    }
}
