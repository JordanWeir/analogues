use crate::{
    adapter::{review_entry, DomainAdapter},
    blackboard::Blackboard,
    document::DocumentStatus,
    entry::EntryStatus,
    error::{BlackboardError, Result},
    ids::RunId,
    model::ModelClient,
    orchestrator::{Orchestrator, OrchestratorDecision},
    persistence::{BlackboardEvent, BlackboardSnapshot, PersistenceStore},
    quality::GateResult,
    synthesis::{SynthesisOutput, SynthesisRequest, Synthesizer},
    task::Task,
    worker::{Worker, WorkerContext},
    worker_run::WorkerRun,
};

#[derive(Clone, Debug)]
pub struct SwarmConfig {
    pub max_iterations: u32,
    pub min_iterations: u32,
    pub max_workers: usize,
    pub token_budget: Option<u64>,
    pub save_snapshots: bool,
    pub snapshot_label_prefix: String,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_iterations: 8,
            min_iterations: 1,
            max_workers: 4,
            token_budget: None,
            save_snapshots: true,
            snapshot_label_prefix: "iter".to_string(),
        }
    }
}

pub struct SwarmResult {
    pub run_id: RunId,
    pub output: SynthesisOutput,
    pub final_state: crate::blackboard::BlackboardState,
    pub worker_runs: Vec<WorkerRun>,
}

pub struct SwarmRuntime<A, O, W, M, P, S> {
    pub adapter: A,
    pub orchestrator: O,
    pub worker: W,
    pub model: M,
    pub persistence: P,
    pub synthesizer: S,
    pub config: SwarmConfig,
}

impl<A, O, W, M, P, S> SwarmRuntime<A, O, W, M, P, S>
where
    A: DomainAdapter,
    O: Orchestrator,
    W: Worker,
    M: ModelClient,
    P: PersistenceStore,
    S: Synthesizer,
{
    pub async fn run(&self, task: Task) -> Result<SwarmResult> {
        self.run_with_seed(task, None).await
    }

    pub async fn run_with_seed(
        &self,
        task: Task,
        seed: Option<crate::blackboard::BlackboardState>,
    ) -> Result<SwarmResult> {
        let run_id = seed
            .as_ref()
            .map(|state| state.run_id.clone())
            .unwrap_or_else(RunId::new);

        let mut board = if let Some(seed_state) = seed {
            Blackboard::from_state(seed_state)
        } else {
            Blackboard::new(run_id.clone(), task.instruction.clone())
        };

        if board.state().task_instruction.is_empty() {
            board.state_mut().task_instruction = task.instruction.clone();
        }
        board.state_mut().token_budget = self.config.token_budget;
        if board.state().documents.is_empty() {
            board.state_mut().documents = task
                .documents
                .iter()
                .map(DocumentStatus::from)
                .collect();
        }

        let mut worker_runs = Vec::new();

        for iteration in 0..self.config.max_iterations {
            board.increment_iteration();

            let decision = self
                .orchestrator
                .plan_next(board.state())
                .await
                .map_err(BlackboardError::Other)?;

            match decision {
                OrchestratorDecision::Converge { .. } if iteration + 1 < self.config.min_iterations => {
                    // Keep looping until min_iterations satisfied.
                }
                OrchestratorDecision::Converge { .. } => break,
                OrchestratorDecision::DispatchWorkers(tasks) => {
                    for worker_task in tasks.into_iter().take(self.config.max_workers) {
                        let ctx = WorkerContext {
                            task_instruction: task.instruction.clone(),
                            entries: board.state().entries.clone(),
                            signals: board.state().signals.clone(),
                            document_sections: Vec::new(),
                        };

                        let mut run = WorkerRun::started(
                            run_id.clone(),
                            worker_task.id.clone(),
                            worker_task.description.clone(),
                        );

                        let output = self
                            .worker
                            .run(worker_task.clone(), ctx)
                            .await
                            .map_err(BlackboardError::Other)?;

                        board.add_tokens_used(output.usage.total);

                        if let Some(budget) = self.config.token_budget {
                            if board.state().tokens_used > budget {
                                return Err(BlackboardError::invalid_state(format!(
                                    "token budget exceeded: {} > {budget}",
                                    board.state().tokens_used
                                )));
                            }
                        }

                        let gates = self.adapter.quality_gates();
                        for entry in &output.entries {
                            let outcomes = review_entry(&gates, entry, board.state());
                            if outcomes
                                .iter()
                                .any(|outcome| matches!(outcome.result, GateResult::Reject(_)))
                            {
                                continue;
                            }

                            let mut accepted = entry.clone();
                            if outcomes.iter().any(|outcome| {
                                matches!(outcome.result, GateResult::Quarantine(_))
                            }) {
                                accepted.status = EntryStatus::Quarantined;
                            }

                            let entry_id = accepted.id.clone();
                            board.add_entry(accepted)?;
                            self.persistence
                                .append_event(BlackboardEvent::EntryAdded {
                                    run_id: run_id.clone(),
                                    entry_id: entry_id.clone(),
                                })
                                .await?;

                            for signal_id in &worker_task.addresses_signals {
                                if board.signal(signal_id).is_some() {
                                    board.address_signal(signal_id, &entry_id)?;
                                    self.persistence
                                        .append_event(BlackboardEvent::SignalAddressed {
                                            run_id: run_id.clone(),
                                            signal_id: signal_id.clone(),
                                            entry_id: entry_id.clone(),
                                        })
                                        .await?;
                                }
                            }
                        }

                        run = run.complete(
                            output.entries.len() as u32,
                            output.usage,
                            output.model,
                            0,
                        );
                        worker_runs.push(run);
                    }
                }
            }

            if self.config.save_snapshots {
                let label = format!("{}-{}", self.config.snapshot_label_prefix, iteration);
                let snapshot =
                    BlackboardSnapshot::new(run_id.clone(), label, board.state().clone());
                self.persistence.save_snapshot(snapshot).await?;
            }
        }

        let open_obligations: Vec<_> = board
            .open_obligations()
            .into_iter()
            .cloned()
            .collect();

        let synthesis_request = SynthesisRequest {
            task: task.clone(),
            board: board.state().clone(),
            must_include: open_obligations,
            format: self.adapter.synthesis_format(&task),
        };

        let output = self
            .synthesizer
            .synthesize(synthesis_request)
            .await
            .map_err(BlackboardError::Other)?;

        Ok(SwarmResult {
            run_id,
            output,
            final_state: board.into_state(),
            worker_runs,
        })
    }
}

/// Convenience builder for assembling a runtime with shared Arc components.
pub struct SwarmRuntimeBuilder<A, O, W, M, P, S> {
    adapter: A,
    orchestrator: O,
    worker: W,
    model: M,
    persistence: P,
    synthesizer: S,
    config: SwarmConfig,
}

impl<A, O, W, M, P, S> SwarmRuntimeBuilder<A, O, W, M, P, S> {
    pub fn new(
        adapter: A,
        orchestrator: O,
        worker: W,
        model: M,
        persistence: P,
        synthesizer: S,
    ) -> Self {
        Self {
            adapter,
            orchestrator,
            worker,
            model,
            persistence,
            synthesizer,
            config: SwarmConfig::default(),
        }
    }

    pub fn config(mut self, config: SwarmConfig) -> Self {
        self.config = config;
        self
    }

    pub fn build(self) -> SwarmRuntime<A, O, W, M, P, S> {
        SwarmRuntime {
            adapter: self.adapter,
            orchestrator: self.orchestrator,
            worker: self.worker,
            model: self.model,
            persistence: self.persistence,
            synthesizer: self.synthesizer,
            config: self.config,
        }
    }
}

/// Null model client for deterministic-only runtimes.
pub struct NullModelClient;

#[async_trait::async_trait]
impl ModelClient for NullModelClient {
    async fn complete(
        &self,
        _request: crate::model::ModelRequest,
    ) -> anyhow::Result<crate::model::ModelResponse> {
        Err(anyhow::anyhow!("null model client does not call models"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::NullAdapter,
        orchestrator::SignalDrainedOrchestrator,
        persistence::MemoryStore,
        signal::{Signal, SignalKind},
        synthesis::MarkdownBoardSynthesizer,
        worker::EchoWorker,
    };

    struct NoOpModel;
    #[async_trait::async_trait]
    impl ModelClient for NoOpModel {
        async fn complete(
            &self,
            _request: crate::model::ModelRequest,
        ) -> anyhow::Result<crate::model::ModelResponse> {
            Err(anyhow::anyhow!("unused"))
        }
    }

    #[tokio::test]
    async fn runtime_addresses_open_signals() {
        let persistence = MemoryStore::new();
        let runtime = SwarmRuntimeBuilder::new(
            NullAdapter,
            SignalDrainedOrchestrator {
                min_priority: crate::signal::Priority::Low,
            },
            EchoWorker {
                worker_id: "echo".to_string(),
            },
            NoOpModel,
            persistence,
            MarkdownBoardSynthesizer,
        )
        .config(SwarmConfig {
            max_iterations: 2,
            min_iterations: 1,
            max_workers: 2,
            token_budget: None,
            save_snapshots: false,
            snapshot_label_prefix: "iter".to_string(),
        })
        .build();

        let task = Task::new("Summarize findings");
        let mut board = Blackboard::new(RunId::new(), task.instruction.clone());
        board
            .add_signal(
                Signal::builder(SignalKind::Question, "What is the revenue trend?").build(),
            )
            .unwrap();
        let seed = board.into_state();

        let result = runtime.run_with_seed(task, Some(seed)).await.unwrap();
        assert!(!result.final_state.entries.is_empty());
        assert!(matches!(result.output, SynthesisOutput::Text(_)));
    }
}
