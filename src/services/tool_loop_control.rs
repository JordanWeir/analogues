use crate::services::{openrouter_chat::CompletionTool, usage_snapshot::UsageSnapshot};
use openrouter_rs::api::chat::Message;
use openrouter_rs::types::Role;
use std::sync::Arc;

/// One executed tool call within an agent loop step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolCall {
    pub tool_name: String,
    pub arguments: String,
    pub succeeded: bool,
}

/// Record of one model turn that invoked at least one client tool.
#[derive(Debug, Clone, Default)]
pub struct AgentStep {
    /// Zero-based step index (matches AI SDK `stepNumber`).
    pub step_number: usize,
    pub tool_calls: Vec<AgentToolCall>,
    pub tool_results: Vec<String>,
    pub usage: UsageSnapshot,
    pub assistant_text: Option<String>,
}

/// Context passed to [`PrepareStepHook::prepare_step`] before each model completion.
pub struct PrepareStepContext<'a> {
    pub step_number: usize,
    pub max_steps: usize,
    pub steps: &'a [AgentStep],
    pub messages: &'a [Message],
    pub model: &'a str,
}

impl<'a> PrepareStepContext<'a> {
    pub fn steps_remaining(&self) -> usize {
        self.max_steps.saturating_sub(self.step_number)
    }

    pub fn steps_completed(&self) -> usize {
        self.step_number
    }
}

/// Optional overrides returned from [`PrepareStepHook::prepare_step`].
#[derive(Debug, Clone, Default)]
pub struct PrepareStepResult {
    pub messages: Option<Vec<Message>>,
    pub model: Option<String>,
    pub tools: Option<Vec<CompletionTool>>,
}

pub trait PrepareStepHook: Send + Sync {
    fn prepare_step(&self, ctx: PrepareStepContext<'_>) -> PrepareStepResult;
}

/// Default prepare step: append the step-budget user message after the first completed step.
#[derive(Debug, Clone)]
pub struct StepBudgetPrepareStep {
    pub submit_tool_name: Option<String>,
}

impl StepBudgetPrepareStep {
    pub fn new(submit_tool_name: Option<String>) -> Self {
        Self { submit_tool_name }
    }
}

impl PrepareStepHook for StepBudgetPrepareStep {
    fn prepare_step(&self, ctx: PrepareStepContext<'_>) -> PrepareStepResult {
        apply_step_budget_prepare(&ctx, self.submit_tool_name.as_deref())
    }
}

/// Runs multiple prepare hooks in order, threading message/model/tool updates through each.
#[derive(Clone)]
pub struct ChainedPrepareStep {
    hooks: Vec<Arc<dyn PrepareStepHook>>,
}

impl ChainedPrepareStep {
    pub fn new(hooks: Vec<Arc<dyn PrepareStepHook>>) -> Self {
        Self { hooks }
    }
}

impl PrepareStepHook for ChainedPrepareStep {
    fn prepare_step(&self, ctx: PrepareStepContext<'_>) -> PrepareStepResult {
        let mut messages = ctx.messages.to_vec();
        let mut model = ctx.model.to_string();
        let mut tools: Option<Vec<CompletionTool>> = None;

        for hook in &self.hooks {
            let step_ctx = PrepareStepContext {
                step_number: ctx.step_number,
                max_steps: ctx.max_steps,
                steps: ctx.steps,
                messages: &messages,
                model: &model,
            };
            let result = hook.prepare_step(step_ctx);
            if let Some(msgs) = result.messages {
                messages = msgs;
            }
            if let Some(next_model) = result.model {
                model = next_model;
            }
            if let Some(next_tools) = result.tools {
                tools = Some(next_tools);
            }
        }

        PrepareStepResult {
            messages: Some(messages),
            model: Some(model),
            tools,
        }
    }
}

/// Appends the standard `[Agent budget]` user message when appropriate.
pub fn apply_step_budget_prepare(
    ctx: &PrepareStepContext<'_>,
    submit_tool_name: Option<&str>,
) -> PrepareStepResult {
    let steps_used = ctx.step_number;
    if steps_used == 0 || steps_used >= ctx.max_steps {
        return PrepareStepResult::default();
    }

    let steps_remaining = ctx.max_steps - steps_used;
    let content = agent_step_budget_message(
        steps_used,
        ctx.max_steps,
        steps_remaining,
        submit_tool_name,
    );
    let mut messages = ctx.messages.to_vec();
    messages.push(Message::new(Role::User, content.as_str()));
    PrepareStepResult {
        messages: Some(messages),
        ..Default::default()
    }
}

pub fn agent_step_budget_message(
    steps_used: usize,
    max_rounds: usize,
    steps_remaining: usize,
    submit_tool_name: Option<&str>,
) -> String {
    let submit_tool = submit_tool_name.unwrap_or("submit_concept_review");
    match steps_remaining {
        0 => format!(
            "[Agent budget] Step {steps_used}/{max_rounds} complete. No steps remaining."
        ),
        1 => {
            if submit_tool_name.is_some() {
                format!(
                    "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: 1 (final turn). \
                     Call {submit_tool} now with corrected decisions if your previous submission failed validation."
                )
            } else {
                format!(
                    "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: 1 (final turn)."
                )
            }
        }
        2 => {
            if submit_tool_name.is_some() {
                format!(
                    "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: 2. \
                     This is your penultimate turn — call {submit_tool} now with your final decisions. \
                     Reserve the last turn to fix validation errors and resubmit if needed."
                )
            } else {
                format!(
                    "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: 2 (penultimate turn)."
                )
            }
        }
        _ => format!(
            "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: {steps_remaining}."
        ),
    }
}

/// Mechanics experiment lane: finalize drafts on penultimate turn, submit on final turn.
pub fn agent_mechanics_step_budget_message(
    steps_used: usize,
    max_rounds: usize,
    steps_remaining: usize,
) -> String {
    match steps_remaining {
        0 => format!(
            "[Agent budget] Step {steps_used}/{max_rounds} complete. No steps remaining."
        ),
        1 => format!(
            "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: 1 (final turn). \
             Call submit_mechanics_experiments now (include crux_key and per_worker true for fan-out workers). \
             If submit failed validation, fix and resubmit."
        ),
        2 => format!(
            "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: 2 (penultimate turn). \
             Finalize every pending draft with finalize_analysis (promote, background, or rejected) before submitting. \
             Reserve the last turn for submit_mechanics_experiments."
        ),
        _ => format!(
            "[Agent budget] Step {steps_used}/{max_rounds} complete. Steps remaining: {steps_remaining}. \
             After run_analysis_draft, always follow with finalize_analysis before starting another draft."
        ),
    }
}

/// Appends mechanics-specific step-budget messages (draft finalize before submit).
pub fn apply_mechanics_step_budget_prepare(ctx: &PrepareStepContext<'_>) -> PrepareStepResult {
    let steps_used = ctx.step_number;
    if steps_used == 0 || steps_used >= ctx.max_steps {
        return PrepareStepResult::default();
    }

    let steps_remaining = ctx.max_steps - steps_used;
    let content = agent_mechanics_step_budget_message(steps_used, ctx.max_steps, steps_remaining);
    let mut messages = ctx.messages.to_vec();
    messages.push(Message::new(Role::User, content.as_str()));
    PrepareStepResult {
        messages: Some(messages),
        ..Default::default()
    }
}

pub fn merge_prepare_step_result(
    messages: &mut Vec<Message>,
    model: &mut String,
    tools: &mut Option<Vec<CompletionTool>>,
    result: PrepareStepResult,
) {
    if let Some(msgs) = result.messages {
        *messages = msgs;
    }
    if let Some(next_model) = result.model {
        *model = next_model;
    }
    if let Some(next_tools) = result.tools {
        *tools = Some(next_tools);
    }
}

/// Context for evaluating [`StopCondition`] after a step with tool results.
pub struct StopConditionContext<'a> {
    pub steps: &'a [AgentStep],
    pub total_usage: &'a UsageSnapshot,
    pub max_steps: usize,
}

pub trait StopCondition: Send + Sync {
    fn should_stop(&self, ctx: &StopConditionContext<'_>) -> bool;
}

pub fn any_stop_condition(
    conditions: &[Arc<dyn StopCondition>],
    ctx: &StopConditionContext<'_>,
) -> bool {
    conditions.iter().any(|condition| condition.should_stop(ctx))
}

/// Stop after the agent has completed `count` tool-bearing steps.
#[derive(Debug, Clone, Copy)]
pub struct StepCountStop {
    pub count: usize,
}

impl StopCondition for StepCountStop {
    fn should_stop(&self, ctx: &StopConditionContext<'_>) -> bool {
        ctx.steps.len() >= self.count
    }
}

pub fn step_count_is(count: usize) -> Arc<dyn StopCondition> {
    Arc::new(StepCountStop { count })
}

/// Stop once a named tool has been called in any prior step.
#[derive(Debug, Clone)]
pub struct HasToolCallStop {
    pub tool_name: String,
}

impl StopCondition for HasToolCallStop {
    fn should_stop(&self, ctx: &StopConditionContext<'_>) -> bool {
        ctx.steps.iter().any(|step| {
            step.tool_calls
                .iter()
                .any(|call| call.tool_name == self.tool_name)
        })
    }
}

pub fn has_tool_call(tool_name: impl Into<String>) -> Arc<dyn StopCondition> {
    Arc::new(HasToolCallStop {
        tool_name: tool_name.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrouter_rs::api::chat::{Content, Message};
    use openrouter_rs::types::Role;

    fn message_text(message: &Message) -> &str {
        match &message.content {
            Content::Text(text) => text.as_str(),
            Content::Parts(_) => "",
            _ => "",
        }
    }

    fn sample_context<'a>(
        step_number: usize,
        max_steps: usize,
        steps: &'a [AgentStep],
        messages: &'a [Message],
    ) -> PrepareStepContext<'a> {
        PrepareStepContext {
            step_number,
            max_steps,
            steps,
            messages,
            model: "test/model",
        }
    }

    #[test]
    fn step_budget_prepare_skips_first_step() {
        let messages = vec![Message::new(Role::User, "hello")];
        let ctx = sample_context(0, 20, &[], &messages);
        let result = apply_step_budget_prepare(&ctx, Some("submit"));
        assert!(result.messages.is_none());
    }

    #[test]
    fn step_budget_prepare_appends_budget_message() {
        let messages = vec![Message::new(Role::User, "hello")];
        let ctx = sample_context(1, 20, &[], &messages);
        let result = apply_step_budget_prepare(&ctx, Some("submit_concept_review"));
        let messages = result.messages.expect("messages");
        assert_eq!(messages.len(), 2);
        let last = message_text(&messages[1]);
        assert!(last.contains("Step 1/20"));
        assert!(last.contains("Steps remaining: 19"));
    }

    #[test]
    fn agent_step_budget_message_nudges_submit_on_penultimate_step() {
        let message = agent_step_budget_message(18, 20, 2, Some("submit_concept_review"));
        assert!(message.contains("Steps remaining: 2"));
        assert!(message.contains("penultimate"));
        assert!(message.contains("submit_concept_review"));
    }

    #[test]
    fn step_count_is_stops_after_n_steps() {
        let condition = step_count_is(2);
        let empty_ctx = StopConditionContext {
            steps: &[],
            total_usage: &UsageSnapshot::default(),
            max_steps: 10,
        };
        assert!(!condition.should_stop(&empty_ctx));

        let steps = vec![
            AgentStep {
                step_number: 0,
                ..Default::default()
            },
            AgentStep {
                step_number: 1,
                ..Default::default()
            },
        ];
        let ctx = StopConditionContext {
            steps: &steps,
            total_usage: &UsageSnapshot::default(),
            max_steps: 10,
        };
        assert!(condition.should_stop(&ctx));
    }

    #[test]
    fn has_tool_call_matches_prior_steps() {
        let condition = has_tool_call("workspace_sql");
        let steps = vec![AgentStep {
            step_number: 0,
            tool_calls: vec![AgentToolCall {
                tool_name: "workspace_sql".to_string(),
                arguments: "{}".to_string(),
                succeeded: true,
            }],
            ..Default::default()
        }];
        let ctx = StopConditionContext {
            steps: &steps,
            total_usage: &UsageSnapshot::default(),
            max_steps: 10,
        };
        assert!(condition.should_stop(&ctx));
        assert!(!has_tool_call("web_search").should_stop(&ctx));
    }

    #[test]
    fn chained_prepare_step_runs_hooks_in_order() {
        struct AppendHook(&'static str);

        impl PrepareStepHook for AppendHook {
            fn prepare_step(&self, ctx: PrepareStepContext<'_>) -> PrepareStepResult {
                let mut messages = ctx.messages.to_vec();
                messages.push(Message::new(Role::User, self.0));
                PrepareStepResult {
                    messages: Some(messages),
                    ..Default::default()
                }
            }
        }

        let messages = vec![Message::new(Role::User, "start")];
        let ctx = sample_context(0, 5, &[], &messages);
        let chain = ChainedPrepareStep::new(vec![
            Arc::new(AppendHook("first")),
            Arc::new(AppendHook("second")),
        ]);
        let result = chain.prepare_step(ctx);
        let messages = result.messages.expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(message_text(&messages[1]), "first");
        assert_eq!(message_text(&messages[2]), "second");
    }
}
