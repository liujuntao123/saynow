use serde::Serialize;

use crate::models::{RecognitionRecord, RecognitionStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    pub total_duration_seconds: u32,
    pub total_records: usize,
    pub total_characters: usize,
}

pub fn aggregate_usage_stats(records: &[RecognitionRecord]) -> UsageStats {
    records
        .iter()
        .filter(|record| record.status == RecognitionStatus::Success)
        .fold(
            UsageStats {
                total_duration_seconds: 0,
                total_records: 0,
                total_characters: 0,
            },
            |mut stats, record| {
                stats.total_duration_seconds += record.duration_seconds;
                stats.total_records += 1;
                stats.total_characters += record.text.chars().count();
                stats
            },
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(duration_seconds: u32, text: &str, status: RecognitionStatus) -> RecognitionRecord {
        RecognitionRecord {
            id: 1,
            created_at: "2026-06-02T10:00:00Z".to_string(),
            duration_seconds,
            text: text.to_string(),
            provider: "MiMo".to_string(),
            model: "mimo-v2.5".to_string(),
            status,
            error_message: None,
        }
    }

    #[test]
    fn aggregates_successful_records_only() {
        let stats = aggregate_usage_stats(&[
            record(11, "你好", RecognitionStatus::Success),
            record(4, "failed", RecognitionStatus::Failed),
            record(20, "Qwen Omni", RecognitionStatus::Success),
        ]);

        assert_eq!(stats.total_duration_seconds, 31);
        assert_eq!(stats.total_records, 2);
        assert_eq!(stats.total_characters, 11);
    }
}
