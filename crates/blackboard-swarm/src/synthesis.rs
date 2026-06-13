use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    blackboard::BlackboardState,
    obligation::Obligation,
    task::Task,
};

#[derive(Clone, Debug)]
pub enum SynthesisFormat {
    Markdown,
    Json,
    Docx,
    FileScoped(HashMap<String, String>),
    Domain(String),
}

#[derive(Clone, Debug)]
pub struct SynthesisRequest {
    pub task: Task,
    pub board: BlackboardState,
    pub must_include: Vec<Obligation>,
    pub format: SynthesisFormat,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum SynthesisOutput {
    Text(String),
    Json(serde_json::Value),
    Files(HashMap<String, Vec<u8>>),
}

#[async_trait]
pub trait Synthesizer: Send + Sync {
    async fn synthesize(&self, request: SynthesisRequest) -> anyhow::Result<SynthesisOutput>;
}

/// Minimal synthesizer that emits markdown from active entries and open obligations.
pub struct MarkdownBoardSynthesizer;

#[async_trait]
impl Synthesizer for MarkdownBoardSynthesizer {
    async fn synthesize(&self, request: SynthesisRequest) -> anyhow::Result<SynthesisOutput> {
        use crate::entry::EntryStatus;
        use crate::obligation::ObligationStatus;

        let mut lines = vec![format!("# {}", request.task.instruction), String::new()];

        lines.push("## Entries".to_string());
        for entry in &request.board.entries {
            if entry.status != EntryStatus::Active {
                continue;
            }
            lines.push(format!(
                "- **{}** ({}): {}",
                entry.kind.as_str(),
                entry.id,
                entry.content
            ));
        }

        lines.push(String::new());
        lines.push("## Obligations".to_string());
        for obligation in &request.must_include {
            if obligation.status != ObligationStatus::Open {
                continue;
            }
            lines.push(format!("- {}", obligation.summary));
        }

        Ok(SynthesisOutput::Text(lines.join("\n")))
    }
}
