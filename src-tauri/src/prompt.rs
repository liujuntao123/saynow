use crate::models::{RecognitionRecord, RecognitionStatus, StylePrompt, VocabularyItem};

const FORMAT_EXAMPLE: &str =
    "格式示例：将“上周三，也就是六月三号，我上午九点零五分参加了第二次产品评审，讨论了三个方案、十二条反馈和百分之十五的预算调整。下午，我把Meeting Notes发给了Alice，晚上八点半又确认了一遍OKR”输出为“上周三，也就是6月3号，我上午9:05参加了第2次产品评审，讨论了3个方案、12条反馈和15%的预算调整。下午，我把Meeting Notes发给了Alice，晚上8:30又确认了一遍OKR”。";

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
        FORMAT_EXAMPLE.to_string(),
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
        assert!(prompt.contains("6月3号"));
        assert!(prompt.contains("9:05"));
        assert!(prompt.contains("第2次"));
        assert!(prompt.contains("3个方案"));
        assert!(prompt.contains("12条反馈"));
        assert!(prompt.contains("15%"));
        assert!(prompt.contains("Meeting Notes"));
        assert!(prompt.contains("Alice"));
        assert!(prompt.contains("8:30"));
        assert!(prompt.contains("OKR"));
        assert!(prompt.contains("昨天讨论 Kunlun"));
        assert!(!prompt.contains("disabled"));
    }
}
