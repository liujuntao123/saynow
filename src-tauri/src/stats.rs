use chrono::{DateTime, Local};
use serde::Serialize;

use crate::models::{RecognitionRecord, RecognitionStatus};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    pub total_duration_seconds: u32,
    pub total_records: usize,
    pub total_characters: usize,
    pub today_duration_seconds: u32,
    pub today_records: usize,
    pub today_characters: usize,
    /// 成功率（0-100 取整），无记录时为 None。
    pub success_rate_percent: Option<u32>,
}

pub fn aggregate_usage_stats(records: &[RecognitionRecord]) -> UsageStats {
    aggregate_usage_stats_at(records, Local::now().date_naive())
}

fn aggregate_usage_stats_at(
    records: &[RecognitionRecord],
    local_today: chrono::NaiveDate,
) -> UsageStats {
    let mut stats = UsageStats {
        total_duration_seconds: 0,
        total_records: 0,
        total_characters: 0,
        today_duration_seconds: 0,
        today_records: 0,
        today_characters: 0,
        success_rate_percent: None,
    };
    let mut attempts = 0usize;
    let mut successes = 0usize;

    for record in records {
        attempts += 1;
        if record.status != RecognitionStatus::Success {
            continue;
        }
        successes += 1;
        let characters = record.text.chars().count();
        stats.total_duration_seconds += record.duration_seconds;
        stats.total_records += 1;
        stats.total_characters += characters;
        if is_on_local_date(&record.created_at, local_today) {
            stats.today_duration_seconds += record.duration_seconds;
            stats.today_records += 1;
            stats.today_characters += characters;
        }
    }

    if attempts > 0 {
        stats.success_rate_percent =
            Some(((successes as f64 / attempts as f64) * 100.0).round() as u32);
    }
    stats
}

fn is_on_local_date(created_at: &str, local_today: chrono::NaiveDate) -> bool {
    DateTime::parse_from_rfc3339(created_at)
        .map(|timestamp| {
            timestamp
                .with_timezone(&Local)
                .date_naive()
                == local_today
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(duration_seconds: u32, text: &str, status: RecognitionStatus) -> RecognitionRecord {
        record_at(duration_seconds, text, status, "2026-06-02T10:00:00Z")
    }

    fn record_at(
        duration_seconds: u32,
        text: &str,
        status: RecognitionStatus,
        created_at: &str,
    ) -> RecognitionRecord {
        RecognitionRecord {
            id: 1,
            created_at: created_at.to_string(),
            duration_seconds,
            text: text.to_string(),
            provider: "MiMo".to_string(),
            model: "mimo-v2.5".to_string(),
            status,
            error_message: None,
            learning_processed_at: None,
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
        assert_eq!(stats.success_rate_percent, Some(67));
    }

    #[test]
    fn splits_today_stats_by_local_date() {
        let today = Local::now();
        let today_iso = today.to_rfc3339();
        let stats = aggregate_usage_stats_at(
            &[
                record_at(10, "今天", RecognitionStatus::Success, &today_iso),
                record_at(20, "很久以前", RecognitionStatus::Success, "2020-01-01T00:00:00Z"),
            ],
            today.date_naive(),
        );

        assert_eq!(stats.total_records, 2);
        assert_eq!(stats.today_records, 1);
        assert_eq!(stats.today_duration_seconds, 10);
        assert_eq!(stats.today_characters, 2);
    }

    #[test]
    fn success_rate_is_none_without_records() {
        let stats = aggregate_usage_stats(&[]);
        assert_eq!(stats.success_rate_percent, None);
    }
}
