use crate::document::Document;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub instruction: String,
    pub documents: Vec<Document>,
    pub metadata: serde_json::Value,
    pub output: OutputSpec,
}

impl Task {
    pub fn new(instruction: impl Into<String>) -> Self {
        Self {
            instruction: instruction.into(),
            documents: Vec::new(),
            metadata: serde_json::Value::Object(Default::default()),
            output: OutputSpec::default(),
        }
    }

    pub fn with_document(mut self, document: Document) -> Self {
        self.documents.push(document);
        self
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputSpec {
    pub output_dir: Option<String>,
    pub deliverables: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStateMap {
    pub rows: Vec<TaskStateRow>,
}

impl Default for TaskStateMap {
    fn default() -> Self {
        Self { rows: Vec::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStateRow {
    pub object_type: String,
    pub required_fields: Vec<String>,
    pub relationships: Vec<String>,
    pub closure_checks: Vec<String>,
    pub worker_questions: Vec<String>,
    pub domain: serde_json::Value,
}
