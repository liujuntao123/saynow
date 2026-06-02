use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecognitionStatus {
    Success,
    Failed,
    Processing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionRecord {
    pub id: i64,
    pub created_at: String,
    pub duration_seconds: u32,
    pub text: String,
    pub provider: String,
    pub model: String,
    pub status: RecognitionStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabularyItem {
    pub id: i64,
    pub term: String,
    pub alias: String,
    pub category: String,
    pub note: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StylePrompt {
    pub id: i64,
    pub name: String,
    pub prompt: String,
    pub enabled: bool,
}
