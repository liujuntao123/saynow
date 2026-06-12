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
pub struct CorrectionRecord {
    pub id: i64,
    pub created_at: String,
    pub recognition_record_id: i64,
    pub raw_text: String,
    pub corrected_text: String,
    pub source: String,
    pub applied: bool,
    pub error_message: Option<String>,
    pub learning_processed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCorrectionInput {
    pub recognition_record_id: i64,
    pub raw_text: String,
    pub corrected_text: String,
    pub source: String,
    pub apply_replacement: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningRule {
    pub id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub rule_type: String,
    pub description: String,
    pub match_hints: String,
    pub from_text: String,
    pub to_text: String,
    pub confidence: f64,
    pub status: String,
    pub evidence_correction_ids: String,
    pub risk: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalizationPreferences {
    pub remove_trailing_period: bool,
}

impl Default for PersonalizationPreferences {
    fn default() -> Self {
        Self {
            remove_trailing_period: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningEngineConfig {
    pub enabled: bool,
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub api_key_ref: String,
    pub run_mode: String,
    pub min_new_corrections: u32,
    pub idle_seconds: u32,
}

impl Default for LearningEngineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: String::new(),
            base_url: String::new(),
            model: String::new(),
            api_key_ref: String::new(),
            run_mode: "llmAssist".to_string(),
            min_new_corrections: 5,
            idle_seconds: 30,
        }
    }
}

impl LearningEngineConfig {
    pub fn has_complete_provider(&self) -> bool {
        !self.provider.trim().is_empty()
            && !self.base_url.trim().is_empty()
            && !self.model.trim().is_empty()
            && !self.api_key_ref.trim().is_empty()
    }
}
