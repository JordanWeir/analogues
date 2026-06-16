use crate::{
    blackboard::BlackboardState,
    entry::Entry,
    orchestrator::WorkerTask,
    quality::{GateOutcome, QualityGate},
    synthesis::SynthesisFormat,
    task::{Task, TaskStateMap},
    worker::WorkerContext,
};

pub trait DomainAdapter: Send + Sync {
    fn name(&self) -> &'static str;

    fn build_task_state_map(&self, task: &Task, board: &BlackboardState) -> TaskStateMap;

    fn seed_plan_prompt(&self, task: &Task, board: &BlackboardState) -> String;

    fn worker_prompt(&self, task: &WorkerTask, ctx: &WorkerContext) -> String;

    fn quality_gates(&self) -> Vec<Box<dyn QualityGate>>;

    fn synthesis_format(&self, task: &Task) -> SynthesisFormat;
}

/// Minimal no-op adapter useful for testing the runtime loop.
pub struct NullAdapter;

impl DomainAdapter for NullAdapter {
    fn name(&self) -> &'static str {
        "null"
    }

    fn build_task_state_map(&self, _task: &Task, _board: &BlackboardState) -> TaskStateMap {
        TaskStateMap::default()
    }

    fn seed_plan_prompt(&self, task: &Task, _board: &BlackboardState) -> String {
        task.instruction.clone()
    }

    fn worker_prompt(&self, task: &WorkerTask, _ctx: &WorkerContext) -> String {
        task.description.clone()
    }

    fn quality_gates(&self) -> Vec<Box<dyn QualityGate>> {
        Vec::new()
    }

    fn synthesis_format(&self, _task: &Task) -> SynthesisFormat {
        SynthesisFormat::Markdown
    }
}

/// Runs adapter quality gates against a candidate entry.
pub fn review_entry(
    gates: &[Box<dyn QualityGate>],
    entry: &Entry,
    board: &BlackboardState,
) -> Vec<GateOutcome> {
    gates
        .iter()
        .map(|gate| GateOutcome {
            gate_name: gate.name().to_string(),
            result: gate.check(entry, board),
        })
        .collect()
}
