pub mod financial_model_explorer;
pub mod fundamental_catalog_manager;
pub mod narrative_researcher;
pub mod scenario_builder;
pub mod tool_loop_agent;
pub mod tools;

pub use tool_loop_agent::{ToolLoopAgent, ToolLoopRequest, ToolLoopResponse};
pub use crate::services::tool_loop_control::{
    apply_step_budget_prepare, has_tool_call, step_count_is, AgentStep, AgentToolCall,
    ChainedPrepareStep, PrepareStepContext, PrepareStepHook, PrepareStepResult,
    StepBudgetPrepareStep, StopCondition, StopConditionContext,
};
pub use tools::{SharedTool, ToolRegistry, WebSearchConfig};
