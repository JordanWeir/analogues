use crate::ids::DocumentId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub name: String,
    pub text: Option<String>,
    pub metadata: serde_json::Value,
}

impl Document {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: DocumentId::new(),
            name: name.into(),
            text: Some(text.into()),
            metadata: serde_json::Value::Object(Default::default()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentStatus {
    pub id: DocumentId,
    pub name: String,
    pub profile: Option<DocumentProfile>,
    pub sections_read: Vec<String>,
    pub sections_unread: Vec<String>,
}

impl From<&Document> for DocumentStatus {
    fn from(document: &Document) -> Self {
        Self {
            id: document.id.clone(),
            name: document.name.clone(),
            profile: None,
            sections_read: Vec::new(),
            sections_unread: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DocumentProfile {
    pub numbered_items: Option<u32>,
    pub tables: Option<u32>,
    pub sections: Option<u32>,
    pub document_type: String,
    pub key_entities: Vec<String>,
    pub estimated_complexity: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}
