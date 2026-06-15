use std::collections::BTreeSet;

use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{CorrectionRecord, LearningRule, RecognitionRecord};

const TECH_HINTS: [&str; 12] = [
    "status", "code", "id", "type", "value", "props", "payload", "token", "api", "http", "json",
    "url",
];

const CHINESE_DIGITS: [(&str, &str); 10] = [
    ("零", "0"),
    ("一", "1"),
    ("二", "2"),
    ("两", "2"),
    ("三", "3"),
    ("四", "4"),
    ("五", "5"),
    ("六", "6"),
    ("七", "7"),
    ("八", "8"),
];

pub fn extract_learning_rules(corrections: &[CorrectionRecord]) -> Vec<LearningRule> {
    let mut rules = Vec::new();
    for correction in corrections {
        if !is_small_edit(&correction.raw_text, &correction.corrected_text) {
            continue;
        }
        rules.extend(extract_numeric_context_rules(correction));
        rules.extend(extract_technical_term_rules(correction));
    }
    merge_learning_rules(rules)
}

pub fn build_llm_learning_payload(
    model: &str,
    corrections: &[CorrectionRecord],
    recognition_history: &[RecognitionRecord],
    existing_rules: &[LearningRule],
) -> Value {
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": learning_system_prompt()
            },
            {
                "role": "user",
                "content": json!({
                    "task": "extract_personal_speech_learning_rules",
                    "locale": "zh-CN",
                    "corrections": corrections.iter().map(correction_to_json).collect::<Vec<_>>(),
                    "recognitionHistory": recognition_history.iter().map(recognition_to_json).collect::<Vec<_>>(),
                    "existingRules": existing_rules.iter().take(30).map(rule_to_json).collect::<Vec<_>>()
                }).to_string()
            }
        ],
        "temperature": 0.1,
        "max_completion_tokens": 1600,
        "stream": false,
        "response_format": { "type": "json_object" }
    })
}

pub fn parse_llm_learning_rules(response_text: &str) -> Result<Vec<LearningRule>, String> {
    let json_text = extract_json_object(response_text)
        .ok_or_else(|| "学习模型没有返回 JSON 对象。".to_string())?;
    let output: LearningModelOutput = serde_json::from_str(json_text)
        .map_err(|error| format!("无法解析学习模型 JSON：{error}"))?;
    Ok(output
        .rules
        .into_iter()
        .filter_map(model_rule_to_rule)
        .collect())
}

fn learning_system_prompt() -> &'static str {
    r#"你是桌面端语音输入助手的个性化学习引擎。你的任务是从两类数据中提取稳定、低噪声、可审阅的语音识别偏好规则：
1. corrections：用户明确纠错对，是强信号。
2. recognitionHistory：用户成功识别历史，是弱信号，只能用于发现重复出现的常用词汇、短语、技术术语和语言习惯。

要求：
- 纠错对可学习“识别错了什么、用户改成什么”。
- 历史识别不能从单条文本生成规则，至少需要 3 条历史证据，或与纠错证据共同支持。
- 历史识别只学习对后续语音识别真正有帮助的内容，例如常用专有名词、英文缩写、技术字段、口头表达偏好、标点/格式习惯。
- 不学习用户事实、身份信息、完整句子、一次性话题、敏感内容、任务内容或私人信息。
- 不保存完整敏感句子，只输出抽象规则或短词短语。
- 高风险规则必须标记 high，不要把它们设为 active。
- 只有纠错证据达到 2 条，或历史证据达到 3 条，才可用 candidate；否则 status 用 observed。
- 输出严格 JSON，不要解释。

JSON schema:
{
  "rules": [
    {
      "type": "numeric_context | preferred_term | frequent_phrase | language_habit | symbol_rule | negative_rule | formatting_rule",
      "description": "短规则说明",
      "matchHints": ["status", "code"],
      "from": ["一"],
      "to": ["1"],
      "confidence": 0.0,
      "status": "observed | candidate",
      "risk": "low | medium | high",
      "evidenceCorrectionIds": [1, 2],
      "evidenceRecognitionIds": [10, 11, 12]
    }
  ],
  "ignored": [
    { "source": "correction", "id": 3, "reason": "不是识别纠错" },
    { "source": "recognition", "id": 8, "reason": "一次性话题，不适合学习" }
  ]
}"#
}

fn correction_to_json(correction: &CorrectionRecord) -> Value {
    json!({
        "id": correction.id,
        "rawText": correction.raw_text,
        "correctedText": correction.corrected_text,
        "source": correction.source,
    })
}

fn recognition_to_json(record: &RecognitionRecord) -> Value {
    json!({
        "id": record.id,
        "text": truncate_for_learning(&record.text, 260),
        "durationSeconds": record.duration_seconds,
        "createdAt": record.created_at,
    })
}

fn rule_to_json(rule: &LearningRule) -> Value {
    json!({
        "id": rule.id,
        "type": rule.rule_type,
        "description": rule.description,
        "matchHints": split_csv(&rule.match_hints),
        "from": split_csv(&rule.from_text),
        "to": split_csv(&rule.to_text),
        "confidence": rule.confidence,
        "status": rule.status,
        "risk": rule.risk,
        "evidenceCorrectionIds": split_csv(&rule.evidence_correction_ids),
        "evidenceRecognitionIds": split_csv(&rule.evidence_recognition_ids),
    })
}

fn model_rule_to_rule(rule: LearningModelRule) -> Option<LearningRule> {
    let rule_type = normalize_rule_type(&rule.rule_type);
    let description = rule.description.trim();
    if rule_type.is_empty()
        || description.is_empty()
        || (rule.evidence_correction_ids.is_empty() && rule.evidence_recognition_ids.is_empty())
    {
        return None;
    }
    let now = Utc::now().to_rfc3339();
    let correction_evidence_text = ids_to_csv(rule.evidence_correction_ids);
    let recognition_evidence_text = ids_to_csv(rule.evidence_recognition_ids);
    let correction_evidence_count = evidence_count(&correction_evidence_text);
    let recognition_evidence_count = evidence_count(&recognition_evidence_text);
    let status = normalize_status(
        rule.status.as_deref(),
        correction_evidence_count,
        recognition_evidence_count,
    );
    Some(LearningRule {
        id: 0,
        created_at: now.clone(),
        updated_at: now,
        rule_type,
        description: description.to_string(),
        match_hints: normalize_list(rule.match_hints),
        from_text: normalize_list(rule.from),
        to_text: normalize_list(rule.to),
        confidence: rule.confidence.unwrap_or(0.65).clamp(0.0, 1.0),
        status,
        evidence_correction_ids: correction_evidence_text,
        evidence_recognition_ids: recognition_evidence_text,
        risk: normalize_risk(rule.risk.as_deref()),
    })
}

fn ids_to_csv(ids: Vec<i64>) -> String {
    ids.into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn evidence_count(text: &str) -> usize {
    text.split(',')
        .filter(|item| !item.trim().is_empty())
        .count()
}

fn normalize_list(items: Vec<String>) -> String {
    items
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(",")
}

fn split_csv(text: &str) -> Vec<String> {
    text.split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn normalize_rule_type(rule_type: &str) -> String {
    match rule_type.trim() {
        "numeric_context" | "preferred_term" | "symbol_rule" | "negative_rule"
        | "formatting_rule" | "frequent_phrase" | "language_habit" => rule_type.trim().to_string(),
        _ => String::new(),
    }
}

fn normalize_status(
    status: Option<&str>,
    correction_evidence_count: usize,
    recognition_evidence_count: usize,
) -> String {
    let enough_for_candidate = correction_evidence_count >= 2
        || correction_evidence_count + recognition_evidence_count >= 3
        || recognition_evidence_count >= 3;
    match status.unwrap_or_default().trim() {
        "candidate" if enough_for_candidate => "candidate".to_string(),
        _ => status_for_evidence(correction_evidence_count, recognition_evidence_count).to_string(),
    }
}

fn normalize_risk(risk: Option<&str>) -> String {
    match risk.unwrap_or_default().trim() {
        "low" => "low".to_string(),
        "high" => "high".to_string(),
        _ => "medium".to_string(),
    }
}

fn extract_json_object(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|body| body.strip_suffix("```"))
        .map(str::trim);
    if let Some(json) = without_fence {
        if json.starts_with('{') && json.ends_with('}') {
            return Some(json);
        }
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (start < end).then_some(&trimmed[start..=end])
}

#[derive(Debug, Deserialize)]
struct LearningModelOutput {
    #[serde(default)]
    rules: Vec<LearningModelRule>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LearningModelRule {
    #[serde(rename = "type")]
    rule_type: String,
    description: String,
    #[serde(default)]
    match_hints: Vec<String>,
    #[serde(default)]
    from: Vec<String>,
    #[serde(default)]
    to: Vec<String>,
    confidence: Option<f64>,
    status: Option<String>,
    risk: Option<String>,
    #[serde(default)]
    evidence_correction_ids: Vec<i64>,
    #[serde(default)]
    evidence_recognition_ids: Vec<i64>,
}

fn truncate_for_learning(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let mut output = trimmed.chars().take(max_chars).collect::<String>();
    if trimmed.chars().count() > max_chars {
        output.push('…');
    }
    output
}

fn extract_numeric_context_rules(correction: &CorrectionRecord) -> Vec<LearningRule> {
    let raw_lower = correction.raw_text.to_ascii_lowercase();
    let corrected_lower = correction.corrected_text.to_ascii_lowercase();
    let hints = TECH_HINTS
        .iter()
        .copied()
        .filter(|hint| raw_lower.contains(hint) || corrected_lower.contains(hint))
        .collect::<Vec<_>>();
    if hints.is_empty() {
        return Vec::new();
    }

    CHINESE_DIGITS
        .iter()
        .filter(|(from, to)| {
            correction.raw_text.contains(from) && correction.corrected_text.contains(to)
        })
        .map(|(from, to)| {
            new_rule(
                "numeric_context",
                &format!(
                    "在 {} 等技术上下文附近，{} 更可能表示数字 {}。",
                    hints.join("/"),
                    from,
                    to
                ),
                &hints.join(","),
                from,
                to,
                0.62,
                correction.id,
            )
        })
        .collect()
}

fn extract_technical_term_rules(correction: &CorrectionRecord) -> Vec<LearningRule> {
    TECH_HINTS
        .iter()
        .copied()
        .filter(|term| {
            correction
                .corrected_text
                .to_ascii_lowercase()
                .contains(term)
        })
        .filter(|term| !correction.raw_text.to_ascii_lowercase().contains(term))
        .map(|term| {
            new_rule(
                "preferred_term",
                &format!("用户倾向在技术文本中保留英文词 {}。", term),
                "technical",
                "",
                term,
                0.58,
                correction.id,
            )
        })
        .collect()
}

fn merge_learning_rules(rules: Vec<LearningRule>) -> Vec<LearningRule> {
    let mut merged = Vec::<LearningRule>::new();
    for rule in rules {
        if let Some(existing) = merged.iter_mut().find(|existing| {
            existing.rule_type == rule.rule_type
                && existing.match_hints == rule.match_hints
                && existing.from_text == rule.from_text
                && existing.to_text == rule.to_text
        }) {
            let correction_evidence = merge_evidence_ids(
                &existing.evidence_correction_ids,
                &rule.evidence_correction_ids,
            );
            let recognition_evidence = merge_evidence_ids(
                &existing.evidence_recognition_ids,
                &rule.evidence_recognition_ids,
            );
            let correction_count = evidence_count(&correction_evidence);
            let recognition_count = evidence_count(&recognition_evidence);
            existing.evidence_correction_ids = correction_evidence;
            existing.evidence_recognition_ids = recognition_evidence;
            existing.confidence = confidence_for_count(
                correction_count + recognition_count,
                existing.confidence.max(rule.confidence),
            );
            existing.status = status_for_evidence(correction_count, recognition_count).to_string();
        } else {
            merged.push(rule);
        }
    }
    merged
}

fn merge_evidence_ids(left: &str, right: &str) -> String {
    let mut ids = BTreeSet::<i64>::new();
    for value in left.split(',').chain(right.split(',')) {
        if let Ok(id) = value.trim().parse::<i64>() {
            ids.insert(id);
        }
    }
    ids.into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn new_rule(
    rule_type: &str,
    description: &str,
    match_hints: &str,
    from_text: &str,
    to_text: &str,
    confidence: f64,
    correction_id: i64,
) -> LearningRule {
    let now = Utc::now().to_rfc3339();
    LearningRule {
        id: 0,
        created_at: now.clone(),
        updated_at: now,
        rule_type: rule_type.to_string(),
        description: description.to_string(),
        match_hints: match_hints.to_string(),
        from_text: from_text.to_string(),
        to_text: to_text.to_string(),
        confidence,
        status: "observed".to_string(),
        evidence_correction_ids: correction_id.to_string(),
        evidence_recognition_ids: String::new(),
        risk: "medium".to_string(),
    }
}

fn confidence_for_count(count: usize, base: f64) -> f64 {
    (base + (count.saturating_sub(1) as f64 * 0.12)).min(0.9)
}

fn status_for_evidence(correction_count: usize, recognition_count: usize) -> &'static str {
    if correction_count >= 2 || correction_count + recognition_count >= 3 || recognition_count >= 3
    {
        "candidate"
    } else {
        "observed"
    }
}

fn is_small_edit(raw: &str, corrected: &str) -> bool {
    let raw_chars = raw.chars().count();
    let corrected_chars = corrected.chars().count();
    if raw_chars == 0 || corrected_chars == 0 {
        return false;
    }
    let max_len = raw_chars.max(corrected_chars);
    let diff = raw_chars.abs_diff(corrected_chars);
    diff <= 8 || diff * 3 <= max_len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn correction(id: i64, raw_text: &str, corrected_text: &str) -> CorrectionRecord {
        CorrectionRecord {
            id,
            created_at: "2026-06-12T00:00:00Z".to_string(),
            recognition_record_id: id,
            raw_text: raw_text.to_string(),
            corrected_text: corrected_text.to_string(),
            source: "test".to_string(),
            applied: true,
            error_message: None,
            learning_processed_at: None,
        }
    }

    fn recognition(id: i64, text: &str) -> RecognitionRecord {
        RecognitionRecord {
            id,
            created_at: "2026-06-12T00:00:00Z".to_string(),
            duration_seconds: 8,
            text: text.to_string(),
            provider: "MiMo".to_string(),
            model: "mimo-v2.5".to_string(),
            status: crate::models::RecognitionStatus::Success,
            error_message: None,
            learning_processed_at: None,
        }
    }

    #[test]
    fn extracts_numeric_context_rule_from_technical_correction() {
        let rules =
            extract_learning_rules(&[correction(1, "status不应该会是一吧", "status不应该会是1吧")]);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_type, "numeric_context");
        assert_eq!(rules[0].match_hints, "status");
        assert_eq!(rules[0].from_text, "一");
        assert_eq!(rules[0].to_text, "1");
        assert_eq!(rules[0].status, "observed");
    }

    #[test]
    fn merges_repeated_evidence_into_candidate_rule() {
        let rules = extract_learning_rules(&[
            correction(1, "status是一", "status是1"),
            correction(2, "status又是一", "status又是1"),
        ]);

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].status, "candidate");
        assert_eq!(rules[0].evidence_correction_ids, "1,2");
        assert!(rules[0].confidence > 0.7);
    }

    #[test]
    fn ignores_large_rewrites() {
        let rules = extract_learning_rules(&[correction(
            1,
            "这个用例不准确",
            "请重新帮我写一个完整测试用例，覆盖所有边界条件",
        )]);

        assert!(rules.is_empty());
    }

    #[test]
    fn parses_llm_learning_rules_from_json_fence() {
        let rules = parse_llm_learning_rules(
            r#"```json
{
  "rules": [
    {
      "type": "numeric_context",
      "description": "在 status 附近中文数字更可能是阿拉伯数字。",
      "matchHints": ["status"],
      "from": ["一"],
      "to": ["1"],
      "confidence": 0.82,
      "status": "candidate",
      "risk": "medium",
      "evidenceCorrectionIds": [1, 2],
      "evidenceRecognitionIds": [10, 11, 12]
    }
  ],
  "ignored": []
}
```"#,
        )
        .unwrap();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_type, "numeric_context");
        assert_eq!(rules[0].status, "candidate");
        assert_eq!(rules[0].risk, "medium");
        assert_eq!(rules[0].match_hints, "status");
        assert_eq!(rules[0].evidence_correction_ids, "1,2");
        assert_eq!(rules[0].evidence_recognition_ids, "10,11,12");
    }

    #[test]
    fn builds_llm_learning_payload_with_corrections() {
        let payload = build_llm_learning_payload(
            "gpt-4.1-mini",
            &[correction(1, "status是一", "status是1")],
            &[recognition(10, "今天继续看 status 和 payload 的处理逻辑。")],
            &[],
        );

        assert_eq!(payload["model"], "gpt-4.1-mini");
        assert_eq!(payload["stream"], false);
        let content = payload["messages"][1]["content"].as_str().unwrap();
        assert!(content.contains("status是一"));
        assert!(content.contains("recognitionHistory"));
        assert!(content.contains("payload"));
    }

    #[test]
    fn parses_history_only_learning_rule_as_candidate_after_repeated_evidence() {
        let rules = parse_llm_learning_rules(
            r#"{
  "rules": [
    {
      "type": "preferred_term",
      "description": "用户经常提到 payload，应作为常用技术词保留英文形式。",
      "matchHints": ["payload"],
      "from": [],
      "to": ["payload"],
      "confidence": 0.74,
      "status": "candidate",
      "risk": "low",
      "evidenceCorrectionIds": [],
      "evidenceRecognitionIds": [10, 11, 12]
    }
  ]
}"#,
        )
        .unwrap();

        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].status, "candidate");
        assert_eq!(rules[0].evidence_correction_ids, "");
        assert_eq!(rules[0].evidence_recognition_ids, "10,11,12");
    }
}
