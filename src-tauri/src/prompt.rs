use crate::models::{RecognitionRecord, RecognitionStatus, StylePrompt, VocabularyItem};

pub fn build_prompt_context(
    vocabulary: &[VocabularyItem],
    styles: &[StylePrompt],
    records: &[RecognitionRecord],
) -> String {
    let words = vocabulary
        .iter()
        .filter(|item| item.enabled)
        .take(30)
        .map(|item| {
            if item.alias.is_empty() {
                format!("- {}", item.term)
            } else {
                format!("- {} ({})", item.term, item.alias)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let style = styles
        .iter()
        .find(|style| style.enabled)
        .map(|style| style.prompt.as_str())
        .unwrap_or("输出自然、准确的简体中文文本。");

    let history = records
        .iter()
        .filter(|record| {
            record.status == RecognitionStatus::Success && !record.text.trim().is_empty()
        })
        .take(5)
        .map(|record| format!("- {}", record.text.trim()))
        .collect::<Vec<_>>()
        .join("\n");

    let mut sections = vec![
        "你是一个桌面端语音识别助手。只输出最终识别文本，不输出解释。".to_string(),
        format!("输出风格：{}", style),
    ];

    if !words.is_empty() {
        sections.push(format!("用户词库：\n{}", words));
    }
    if !history.is_empty() {
        sections.push(format!("相关历史：\n{}", history));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_enabled_context_only() {
        let prompt = build_prompt_context(
            &[
                VocabularyItem {
                    id: 1,
                    term: "Kunlun".to_string(),
                    alias: "昆仑".to_string(),
                    category: "project".to_string(),
                    note: String::new(),
                    enabled: true,
                },
                VocabularyItem {
                    id: 2,
                    term: "disabled".to_string(),
                    alias: String::new(),
                    category: String::new(),
                    note: String::new(),
                    enabled: false,
                },
            ],
            &[StylePrompt {
                id: 1,
                name: "书面语".to_string(),
                prompt: "整理为简洁书面语。".to_string(),
                enabled: true,
            }],
            &[RecognitionRecord {
                id: 1,
                created_at: "2026-06-02T10:00:00Z".to_string(),
                duration_seconds: 12,
                text: "昨天讨论 Kunlun 模型导出。".to_string(),
                provider: "MiMo".to_string(),
                model: "mimo-v2.5".to_string(),
                status: RecognitionStatus::Success,
                error_message: None,
            }],
        );

        assert!(prompt.contains("Kunlun"));
        assert!(prompt.contains("昆仑"));
        assert!(prompt.contains("整理为简洁书面语"));
        assert!(prompt.contains("昨天讨论 Kunlun"));
        assert!(!prompt.contains("disabled"));
    }
}
