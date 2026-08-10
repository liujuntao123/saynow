use std::{io::BufRead, sync::OnceLock, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    asr::{is_doubao_provider, recognize_doubao_with_transcript, test_doubao_connection},
    audio::{NativeAudioRecorder, NativeRecordedAudio},
    db::{AppConfig, AppDb, ProviderConfig},
    learning::{build_llm_learning_payload, extract_learning_rules, parse_llm_learning_rules},
    models::{
        CorrectionRecord, LearningEngineConfig, LearningRule, PersonalizationPreferences,
        RecognitionRecord, RecognitionStatus, SaveCorrectionInput, StylePrompt, VocabularyItem,
    },
    platform::{current_platform_status, inject_text, replace_last_injected_text, PlatformStatus},
    prompt::build_prompt_context,
    provider::{
        build_dashscope_multimodal_payload, build_openai_compatible_payload,
        dashscope_connection_test_audio_base64, dashscope_multimodal_url,
        extract_dashscope_multimodal_text, extract_openai_compatible_text,
        first_openai_compatible_stream_text, is_dashscope_multimodal_provider,
        push_openai_compatible_stream_line,
    },
    stats::{aggregate_usage_stats, UsageStats},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    pub stats: UsageStats,
    pub records: Vec<RecognitionRecord>,
    pub platform: PlatformStatus,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionAudioInput {
    pub audio_base64: String,
    pub duration_seconds: u32,
    pub mime_type: String,
}

/// 识别失败时保留的最后一段音频，供"重试"使用，避免用户重新口述。
#[derive(Default)]
pub struct LastRecognitionAudio(std::sync::Mutex<Option<RecognitionAudioInput>>);

impl LastRecognitionAudio {
    pub fn store(&self, input: &RecognitionAudioInput) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(input.clone());
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
        }
    }

    pub fn take_for_retry(&self) -> Option<RecognitionAudioInput> {
        self.0.lock().ok().and_then(|guard| guard.clone())
    }
}

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RECOGNITION_TIMEOUT: Duration = Duration::from_secs(90);
const LEARNING_TIMEOUT: Duration = Duration::from_secs(120);

/// 全局共享 HTTP 客户端：复用连接池/TLS 会话，并统一设置连接超时，
/// 避免供应商端点挂起时识别流程无限期卡死。
fn http_client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .build()
                .map_err(|error| format!("无法初始化 HTTP 客户端：{error}"))
        })
        .as_ref()
        .map_err(Clone::clone)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderTranscriptPayload {
    pub text: String,
    pub done: bool,
}

pub fn dashboard_data(db: &AppDb) -> Result<DashboardData, String> {
    // 统计基于更长的历史窗口，展示列表只取最近 50 条。
    let stats_records = db.list_records(2000).map_err(|error| error.to_string())?;
    let records = stats_records.iter().take(50).cloned().collect();
    Ok(DashboardData {
        stats: aggregate_usage_stats(&stats_records),
        records,
        platform: current_platform_status(),
    })
}

pub fn write_runtime_log_data(message: String) {
    crate::runtime_log::write_line(&message);
}

pub fn get_runtime_log_path_data() -> Result<String, String> {
    crate::runtime_log::log_path_string()
}

pub fn get_config_data(db: &AppDb) -> Result<AppConfig, String> {
    db.get_config().map_err(|error| error.to_string())
}

pub fn save_config_data(db: &AppDb, config: AppConfig) -> Result<AppConfig, String> {
    db.save_config(&config).map_err(|error| error.to_string())?;
    Ok(config)
}

pub fn list_provider_configs_data(db: &AppDb) -> Result<Vec<ProviderConfig>, String> {
    db.list_provider_configs()
        .map_err(|error| error.to_string())
}

pub fn save_provider_config_data(
    db: &AppDb,
    provider: ProviderConfig,
) -> Result<Vec<ProviderConfig>, String> {
    if !provider.has_complete_provider() {
        return Err("供应商配置不完整。".to_string());
    }
    db.save_provider_config(&provider)
        .map_err(|error| error.to_string())?;
    list_provider_configs_data(db)
}

pub fn select_provider_config_data(db: &AppDb, id: i64) -> Result<AppConfig, String> {
    db.select_provider_config(id)
        .map_err(|error| error.to_string())
}

const PROVIDER_TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// 验证供应商配置是否可用。
/// OpenAI-compatible：最短 chat 请求；DashScope：短静音音频；豆包：WebSocket 握手。
pub fn test_provider_connection_data(provider: ProviderConfig) -> Result<String, String> {
    if !provider.has_complete_provider() {
        return Err("请先填写完整的供应商配置。".to_string());
    }

    if is_doubao_provider(&provider.kind, &provider.provider) {
        let config = AppConfig {
            provider: provider.provider.clone(),
            base_url: provider.base_url.clone(),
            model: provider.model.clone(),
            api_key_ref: provider.api_key_ref.clone(),
            hotkey: String::new(),
            kind: provider.kind.clone(),
            extra_json: provider.extra_json.clone(),
        };
        return test_doubao_connection(&config);
    }

    if is_dashscope_multimodal_provider(&provider.kind, &provider.model) {
        let api_key = resolve_api_key(&provider.api_key_ref)?;
        let audio = dashscope_connection_test_audio_base64();
        let payload = build_dashscope_multimodal_payload(&provider.model, &audio, "audio/wav");
        let started_at = std::time::Instant::now();
        let response = send_dashscope_multimodal_request(
            &provider.base_url,
            &api_key,
            &payload,
            PROVIDER_TEST_TIMEOUT,
        )
        .map_err(|error| format!("连接失败：{error}"))?;
        let status = response.status();
        let elapsed_ms = started_at.elapsed().as_millis();
        if status.is_success() {
            return Ok(format!("连接成功（{elapsed_ms}ms）"));
        }
        let body = response.text().unwrap_or_default();
        return Err(provider_http_error(status, &body));
    }

    let api_key = resolve_api_key(&provider.api_key_ref)?;
    let url = format!(
        "{}/chat/completions",
        provider.base_url.trim().trim_end_matches('/')
    );
    let payload = serde_json::json!({
        "model": provider.model,
        "messages": [{ "role": "user", "content": "ping" }],
        "max_completion_tokens": 1,
        "stream": false
    });
    let request = http_client()?.post(&url).timeout(PROVIDER_TEST_TIMEOUT);
    let request = if provider.provider.eq_ignore_ascii_case("mimo")
        || provider
            .base_url
            .to_ascii_lowercase()
            .contains("xiaomimimo.com")
    {
        request.header("api-key", api_key)
    } else {
        request.bearer_auth(api_key)
    };
    let started_at = std::time::Instant::now();
    let response = request
        .json(&payload)
        .send()
        .map_err(|error| format!("连接失败：{error}"))?;
    let status = response.status();
    let elapsed_ms = started_at.elapsed().as_millis();
    if status.is_success() {
        return Ok(format!("连接成功（{elapsed_ms}ms）"));
    }
    let body = response.text().unwrap_or_default();
    Err(provider_http_error(status, &body))
}

fn provider_http_error(status: reqwest::StatusCode, body: &str) -> String {
    let summary = body.chars().take(300).collect::<String>();
    match status.as_u16() {
        401 | 403 => format!("鉴权失败（HTTP {status}），请检查 API Key。{summary}"),
        404 => format!("接口或模型不存在（HTTP {status}），请检查 URL 和模型名。{summary}"),
        _ => format!("请求失败（HTTP {status}）：{summary}"),
    }
}

pub fn delete_provider_config_data(db: &AppDb, id: i64) -> Result<Vec<ProviderConfig>, String> {
    db.delete_provider_config(id)
        .map_err(|error| error.to_string())?;
    list_provider_configs_data(db)
}

pub fn list_records_data(db: &AppDb) -> Result<Vec<RecognitionRecord>, String> {
    db.list_records(200).map_err(|error| error.to_string())
}

pub fn list_correction_records_data(db: &AppDb) -> Result<Vec<CorrectionRecord>, String> {
    db.list_correction_records(200)
        .map_err(|error| error.to_string())
}

pub fn list_learning_rules_data(db: &AppDb) -> Result<Vec<LearningRule>, String> {
    db.list_learning_rules(200)
        .map_err(|error| error.to_string())
}

const ALLOWED_RULE_STATUSES: [&str; 6] = [
    "observed",
    "candidate",
    "active",
    "pinned",
    "rejected",
    "disabled",
];

pub fn update_learning_rule_status_data(
    db: &AppDb,
    id: i64,
    status: String,
) -> Result<Vec<LearningRule>, String> {
    let status = status.trim();
    if !ALLOWED_RULE_STATUSES.contains(&status) {
        return Err(format!("不支持的规则状态：{status}"));
    }
    db.update_learning_rule_status(id, status)
        .map_err(|error| error.to_string())?;
    list_learning_rules_data(db)
}

pub fn delete_learning_rule_data(db: &AppDb, id: i64) -> Result<Vec<LearningRule>, String> {
    db.delete_learning_rule(id)
        .map_err(|error| error.to_string())?;
    list_learning_rules_data(db)
}

pub fn clear_learning_rules_data(db: &AppDb) -> Result<Vec<LearningRule>, String> {
    db.clear_learning_rules()
        .map_err(|error| error.to_string())?;
    list_learning_rules_data(db)
}

pub fn clear_correction_records_data(db: &AppDb) -> Result<Vec<CorrectionRecord>, String> {
    db.clear_correction_records()
        .map_err(|error| error.to_string())?;
    list_correction_records_data(db)
}

pub fn delete_record_data(db: &AppDb, id: i64) -> Result<Vec<RecognitionRecord>, String> {
    db.delete_recognition_record(id)
        .map_err(|error| error.to_string())?;
    list_records_data(db)
}

pub fn clear_records_data(db: &AppDb) -> Result<Vec<RecognitionRecord>, String> {
    db.clear_recognition_records()
        .map_err(|error| error.to_string())?;
    list_records_data(db)
}

pub fn refresh_learning_rules_data(db: &AppDb) -> Result<Vec<LearningRule>, String> {
    let corrections = db
        .list_correction_records(200)
        .map_err(|error| error.to_string())?;
    let rules = extract_learning_rules(&corrections);
    for rule in rules {
        db.upsert_learning_rule(&rule)
            .map_err(|error| error.to_string())?;
    }
    list_learning_rules_data(db)
}

pub fn run_learning_engine_data(db: &AppDb, force: bool) -> Result<Vec<LearningRule>, String> {
    let config = db
        .get_learning_engine_config()
        .map_err(|error| error.to_string())?;
    eprintln!(
        "[saynow] learning engine requested; enabled={} mode={} force={} min_new_samples={}",
        config.enabled, config.run_mode, force, config.min_new_corrections
    );
    if !config.enabled && !force {
        eprintln!("[saynow] learning engine skipped: disabled");
        return list_learning_rules_data(db);
    }

    let limit = config.min_new_corrections.max(1) as usize;
    let correction_limit = if force { 200 } else { limit };
    let recognition_limit = if force { 120 } else { (limit * 4).max(20) };
    let corrections = db
        .list_unprocessed_correction_records(correction_limit)
        .map_err(|error| error.to_string())?;
    let recognition_history = db
        .list_unprocessed_recognition_records(recognition_limit)
        .map_err(|error| error.to_string())?;
    eprintln!(
        "[saynow] learning engine loaded samples; corrections={} recognition_history={}",
        corrections.len(),
        recognition_history.len()
    );
    let sample_count = corrections.len() + recognition_history.len();
    if sample_count == 0 || (!force && sample_count < limit) {
        eprintln!("[saynow] learning engine skipped: insufficient samples");
        return list_learning_rules_data(db);
    }

    let rules = if config.run_mode == "localOnly" {
        eprintln!("[saynow] learning engine using localOnly extractor");
        extract_learning_rules(&corrections)
    } else {
        run_llm_learning(
            &config,
            &corrections,
            &recognition_history,
            &db.list_learning_rules(50)
                .map_err(|error| error.to_string())?,
        )?
    };

    eprintln!(
        "[saynow] learning engine generated rules; count={}",
        rules.len()
    );
    for rule in &rules {
        eprintln!(
            "[saynow] learning rule upsert; type={} status={} risk={} confidence={} correction_evidence={} recognition_evidence={}",
            rule.rule_type,
            rule.status,
            rule.risk,
            rule.confidence,
            rule.evidence_correction_ids,
            rule.evidence_recognition_ids
        );
        db.upsert_learning_rule(rule)
            .map_err(|error| error.to_string())?;
    }
    let ids = corrections
        .iter()
        .map(|correction| correction.id)
        .collect::<Vec<_>>();
    db.mark_corrections_learning_processed(&ids)
        .map_err(|error| error.to_string())?;
    let recognition_ids = recognition_history
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    db.mark_recognition_records_learning_processed(&recognition_ids)
        .map_err(|error| error.to_string())?;
    list_learning_rules_data(db)
}

fn run_llm_learning(
    config: &LearningEngineConfig,
    corrections: &[CorrectionRecord],
    recognition_history: &[RecognitionRecord],
    existing_rules: &[LearningRule],
) -> Result<Vec<LearningRule>, String> {
    if !config.has_complete_provider() {
        return Err("学习引擎 LLM 配置不完整。".to_string());
    }
    let api_key = resolve_api_key(&config.api_key_ref)?;
    let payload = build_llm_learning_payload(
        &config.model,
        corrections,
        recognition_history,
        existing_rules,
    );
    eprintln!(
        "[saynow] learning engine sending LLM request; provider={} model={} corrections={} recognition_history={} payload_chars={}",
        config.provider,
        config.model,
        corrections.len(),
        recognition_history.len(),
        payload.to_string().chars().count()
    );
    let url = format!(
        "{}/chat/completions",
        config.base_url.trim().trim_end_matches('/')
    );
    let client = http_client()?;
    let response = client
        .post(&url)
        .timeout(LEARNING_TIMEOUT)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .map_err(|error| format!("学习引擎请求失败：{error}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| format!("学习引擎响应读取失败：{error}"))?;
    eprintln!(
        "[saynow] learning engine response; status={} body_chars={}",
        status,
        body.chars().count()
    );
    if !status.is_success() {
        return Err(format!(
            "学习引擎响应失败：HTTP {status} {}",
            body.chars().take(500).collect::<String>()
        ));
    }
    let value: Value =
        serde_json::from_str(&body).map_err(|error| format!("学习引擎响应不是 JSON：{error}"))?;
    let text = extract_openai_compatible_text(&value)
        .or_else(|| {
            value
                .pointer("/choices/0/message/content")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| "学习引擎响应中没有可解析内容。".to_string())?;
    eprintln!(
        "[saynow] learning engine content extracted; chars={}",
        text.chars().count()
    );
    parse_llm_learning_rules(&text)
}

pub fn save_correction_data(
    db: &AppDb,
    input: SaveCorrectionInput,
) -> Result<CorrectionRecord, String> {
    let raw_text = input.raw_text.trim();
    let corrected_text = input.corrected_text.trim();
    if raw_text.is_empty() {
        return Err("原始识别文本为空，无法保存纠错。".to_string());
    }
    if corrected_text.is_empty() {
        return Err("修正文案为空，无法保存纠错。".to_string());
    }
    if raw_text == corrected_text {
        return Err("修正文案与原始文本相同，已跳过纠错记录。".to_string());
    }

    let replacement_error = if input.apply_replacement {
        replace_last_injected_text(corrected_text).err()
    } else {
        None
    };
    let correction = db
        .insert_correction_record(
            input.recognition_record_id,
            raw_text,
            corrected_text,
            input.source.trim(),
            input.apply_replacement && replacement_error.is_none(),
            replacement_error.as_deref(),
        )
        .map_err(|error| error.to_string())?;
    Ok(correction)
}

pub fn undo_last_injected_text_data() -> Result<(), String> {
    crate::platform::undo_last_injected_text()
}

pub fn start_native_recording_data(recorder: &NativeAudioRecorder) -> Result<(), String> {
    recorder.start()
}

pub fn stop_native_recording_data(
    recorder: &NativeAudioRecorder,
) -> Result<Option<NativeRecordedAudio>, String> {
    recorder.stop()
}

pub fn cancel_native_recording_data(recorder: &NativeAudioRecorder) -> Result<(), String> {
    recorder.cancel()
}

pub fn list_vocabulary_data(db: &AppDb) -> Result<Vec<VocabularyItem>, String> {
    db.list_vocabulary().map_err(|error| error.to_string())
}

pub fn add_vocabulary_data(
    db: &AppDb,
    item: VocabularyItem,
) -> Result<Vec<VocabularyItem>, String> {
    db.add_vocabulary(&item)
        .map_err(|error| error.to_string())?;
    list_vocabulary_data(db)
}

pub fn add_vocabulary_terms_data(
    db: &AppDb,
    terms: Vec<String>,
) -> Result<Vec<VocabularyItem>, String> {
    db.add_vocabulary_terms(&terms)
        .map_err(|error| error.to_string())?;
    list_vocabulary_data(db)
}

pub fn delete_vocabulary_data(db: &AppDb, id: i64) -> Result<Vec<VocabularyItem>, String> {
    db.delete_vocabulary(id)
        .map_err(|error| error.to_string())?;
    list_vocabulary_data(db)
}

pub fn update_vocabulary_data(
    db: &AppDb,
    item: VocabularyItem,
) -> Result<Vec<VocabularyItem>, String> {
    if item.term.trim().is_empty() {
        return Err("词条不能为空。".to_string());
    }
    db.update_vocabulary(&item)
        .map_err(|error| error.to_string())?;
    list_vocabulary_data(db)
}

pub fn list_style_prompts_data(db: &AppDb) -> Result<Vec<StylePrompt>, String> {
    db.list_style_prompts().map_err(|error| error.to_string())
}

pub fn add_style_prompt_data(db: &AppDb, item: StylePrompt) -> Result<Vec<StylePrompt>, String> {
    db.add_style_prompt(&item)
        .map_err(|error| error.to_string())?;
    list_style_prompts_data(db)
}

pub fn update_style_prompt_data(db: &AppDb, item: StylePrompt) -> Result<Vec<StylePrompt>, String> {
    db.update_style_prompt(&item)
        .map_err(|error| error.to_string())?;
    list_style_prompts_data(db)
}

pub fn delete_style_prompt_data(db: &AppDb, id: i64) -> Result<Vec<StylePrompt>, String> {
    db.delete_style_prompt(id)
        .map_err(|error| error.to_string())?;
    list_style_prompts_data(db)
}

pub fn get_personalization_preferences_data(
    db: &AppDb,
) -> Result<PersonalizationPreferences, String> {
    db.get_personalization_preferences()
        .map_err(|error| error.to_string())
}

pub fn save_personalization_preferences_data(
    db: &AppDb,
    preferences: PersonalizationPreferences,
) -> Result<PersonalizationPreferences, String> {
    db.save_personalization_preferences(&preferences)
        .map_err(|error| error.to_string())?;
    Ok(preferences)
}

pub fn get_learning_engine_config_data(db: &AppDb) -> Result<LearningEngineConfig, String> {
    db.get_learning_engine_config()
        .map_err(|error| error.to_string())
}

pub fn save_learning_engine_config_data(
    db: &AppDb,
    mut config: LearningEngineConfig,
) -> Result<LearningEngineConfig, String> {
    config.provider = config.provider.trim().to_string();
    config.base_url = config.base_url.trim().to_string();
    config.model = config.model.trim().to_string();
    config.api_key_ref = config.api_key_ref.trim().to_string();
    config.run_mode = if config.run_mode == "localOnly" {
        "localOnly".to_string()
    } else {
        "llmAssist".to_string()
    };
    config.min_new_corrections = config.min_new_corrections.max(1);
    config.idle_seconds = config.idle_seconds.max(5);
    if config.enabled && config.run_mode == "llmAssist" && !config.has_complete_provider() {
        return Err("启用学习引擎前，请填写完整的 LLM 供应商、URL、模型和 API Key。".to_string());
    }
    db.save_learning_engine_config(&config)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

pub fn recognize_audio_data(
    db: &AppDb,
    input: RecognitionAudioInput,
) -> Result<RecognitionRecord, String> {
    recognize_audio_data_with_transcript(db, input, |_| {})
}

pub fn recognize_audio_data_with_transcript<F>(
    db: &AppDb,
    input: RecognitionAudioInput,
    on_transcript: F,
) -> Result<RecognitionRecord, String>
where
    F: FnMut(String),
{
    let started_at = std::time::Instant::now();
    let start_message = format!(
        "[saynow] recognize_audio started; duration={}s mime={} base64_len={}",
        input.duration_seconds,
        input.mime_type,
        input.audio_base64.len()
    );
    eprintln!("{start_message}");
    crate::runtime_log::write_line(&start_message);
    if input.audio_base64.trim().is_empty() {
        eprintln!("[saynow] recognize_audio rejected empty audio");
        return insert_failed_record(db, "录音数据为空。", input.duration_seconds.max(1));
    }

    let config = db.get_config().map_err(|error| error.to_string())?;
    if !config.has_complete_provider() {
        return insert_failed_record(
            db,
            "请先在配置页添加并启用识别供应商。",
            input.duration_seconds.max(1),
        );
    }
    let vocabulary = db.list_vocabulary().map_err(|error| error.to_string())?;

    let request_started_at = std::time::Instant::now();
    let recognition_result = if is_doubao_provider(&config.kind, &config.provider) {
        let hotwords: Vec<String> = vocabulary
            .iter()
            .filter(|item| item.enabled)
            .map(|item| item.term.trim().to_string())
            .filter(|term| !term.is_empty())
            .collect();
        eprintln!(
            "[saynow] recognize_audio building doubao request; provider={} resource={} mime={} hotwords={}",
            config.provider,
            config.model,
            input.mime_type,
            hotwords.len()
        );
        recognize_doubao_with_transcript(&config, &input, &hotwords, on_transcript)
    } else if is_dashscope_multimodal_provider(&config.kind, &config.model) {
        eprintln!(
            "[saynow] recognize_audio building DashScope multimodal request; provider={} model={} mime={}",
            config.provider, config.model, input.mime_type
        );
        let payload = build_dashscope_multimodal_payload(
            &config.model,
            &input.audio_base64,
            &input.mime_type,
        );
        call_dashscope_multimodal_with_transcript(&config, payload, on_transcript)
    } else {
        let styles = db.list_style_prompts().map_err(|error| error.to_string())?;
        let records = db.list_records(10).map_err(|error| error.to_string())?;
        let learning_rules = db
            .list_learning_rules(20)
            .map_err(|error| error.to_string())?;
        let prompt = build_prompt_context(&vocabulary, &styles, &records, &learning_rules);
        eprintln!(
            "[saynow] recognize_audio building request; provider={} model={} mime={} prompt_chars={} learning_rules={}",
            config.provider,
            config.model,
            input.mime_type,
            prompt.chars().count(),
            learning_rules.len()
        );
        let payload = build_openai_compatible_payload(
            &config.provider,
            &config.model,
            &prompt,
            &input.audio_base64,
            &input.mime_type,
        );
        call_openai_compatible_chat_with_transcript(&config, payload, on_transcript)
    };
    let request_elapsed_ms = request_started_at.elapsed().as_millis();
    crate::runtime_log::write_line(&format!(
        "[saynow] recognition provider request finished; elapsed_ms={request_elapsed_ms}"
    ));
    let text = match recognition_result {
        Ok(text) => text,
        Err(error) => return insert_failed_record(db, &error, input.duration_seconds.max(1)),
    };

    let preferences = db
        .get_personalization_preferences()
        .map_err(|error| error.to_string())?;
    let text = apply_text_preferences(text, &preferences);
    let result = persist_recognized_text(db, text, input.duration_seconds, inject_text);
    crate::runtime_log::write_line(&format!(
        "[saynow] recognize_audio finished; elapsed_ms={} success={}",
        started_at.elapsed().as_millis(),
        result.is_ok()
    ));
    result
}

fn send_dashscope_multimodal_request(
    base_url: &str,
    api_key: &str,
    payload: &Value,
    timeout: Duration,
) -> Result<reqwest::blocking::Response, String> {
    let url = dashscope_multimodal_url(base_url);
    eprintln!("[saynow] sending DashScope multimodal request; url={url}");
    http_client()?
        .post(url)
        .timeout(timeout)
        .bearer_auth(api_key)
        .header("X-DashScope-SSE", "disable")
        .json(payload)
        .send()
        .map_err(|error| format!("DashScope 请求失败：{error}"))
}

fn call_dashscope_multimodal_with_transcript<F>(
    config: &crate::db::AppConfig,
    payload: Value,
    mut on_transcript: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    let api_key = resolve_api_key(&config.api_key_ref)?;
    let response = send_dashscope_multimodal_request(
        &config.base_url,
        &api_key,
        &payload,
        RECOGNITION_TIMEOUT,
    )?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| format!("读取 DashScope 识别响应失败：{error}"))?;
    if !status.is_success() {
        return Err(format!("DashScope 识别请求返回 {status}：{body}"));
    }
    let json: Value = serde_json::from_str(&body)
        .map_err(|error| format!("DashScope 识别响应不是有效 JSON：{error}"))?;
    let text = extract_dashscope_multimodal_text(&json)
        .ok_or_else(|| "DashScope 识别响应中没有可用文本。".to_string())?;
    on_transcript(text.clone());
    Ok(text)
}

fn apply_text_preferences(text: String, preferences: &PersonalizationPreferences) -> String {
    let mut text = remove_spaces_between_han_characters(&text);
    if !preferences.remove_trailing_period {
        return text;
    }

    if matches!(text.chars().last(), Some('。' | '.')) {
        text.pop();
    }
    text
}

fn remove_spaces_between_han_characters(text: &str) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    let mut normalized = String::with_capacity(text.len());
    let mut last_emitted = None;
    let mut index = 0;

    while index < characters.len() {
        let character = characters[index];
        if is_horizontal_whitespace(character) && last_emitted.is_some_and(is_han_character) {
            let whitespace_start = index;
            while index < characters.len() && is_horizontal_whitespace(characters[index]) {
                index += 1;
            }
            if index < characters.len() && is_han_character(characters[index]) {
                continue;
            }
            for whitespace in &characters[whitespace_start..index] {
                normalized.push(*whitespace);
                last_emitted = Some(*whitespace);
            }
            continue;
        }

        normalized.push(character);
        last_emitted = Some(character);
        index += 1;
    }

    normalized
}

fn is_horizontal_whitespace(character: char) -> bool {
    character.is_whitespace() && !matches!(character, '\n' | '\r')
}

fn is_han_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2EE5F
            | 0x2F800..=0x2FA1F
            | 0x30000..=0x323AF
    )
}

fn persist_recognized_text<F>(
    db: &AppDb,
    text: String,
    duration_seconds: u32,
    inject: F,
) -> Result<RecognitionRecord, String>
where
    F: FnOnce(&str) -> Result<(), String>,
{
    let duration_seconds = duration_seconds.max(1);
    if text.trim().is_empty() {
        return insert_cancelled_record(db, "未识别到可插入文本，已取消插入。", duration_seconds);
    }

    let injection_error = inject(&text).err();
    if let Some(error) = injection_error.as_deref() {
        eprintln!("[saynow] text injection failed: {error}");
    } else {
        eprintln!("[saynow] text injection finished");
    }
    db.insert_record_with_error(
        &text,
        duration_seconds,
        RecognitionStatus::Success,
        injection_error.as_deref(),
    )
    .map_err(|error| error.to_string())?;
    latest_record(db)
}

fn call_openai_compatible_chat_with_transcript<F>(
    config: &crate::db::AppConfig,
    payload: Value,
    mut on_transcript: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    let api_key = resolve_api_key(&config.api_key_ref)?;
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    eprintln!(
        "[saynow] sending recognition request; url={} model={}",
        url, config.model
    );
    let request = http_client()?.post(url).timeout(RECOGNITION_TIMEOUT);
    let request = if uses_mimo_api_key_header(config) {
        request.header("api-key", api_key)
    } else {
        request.bearer_auth(api_key)
    };
    let response = request
        .json(&payload)
        .send()
        .map_err(|error| format!("识别请求失败：{error}"))?;
    let status = response.status();
    eprintln!("[saynow] recognition response status={status}");
    if !status.is_success() {
        let body = response
            .text()
            .map_err(|error| format!("读取识别响应失败：{error}"))?;
        return Err(format!("识别请求返回 {status}：{body}"));
    }

    let text = if uses_stream_response(&payload) {
        let mut content = String::new();
        let mut reasoning_content = String::new();
        let reader = std::io::BufReader::new(response);
        for line in reader.lines() {
            let line = line.map_err(|error| format!("读取识别响应失败：{error}"))?;
            if let Some(text) =
                push_openai_compatible_stream_line(&line, &mut content, &mut reasoning_content)
            {
                on_transcript(text);
            }
        }
        first_openai_compatible_stream_text(content, reasoning_content)
            .ok_or_else(|| "识别响应中没有可用文本。".to_string())?
    } else {
        let body = response
            .text()
            .map_err(|error| format!("读取识别响应失败：{error}"))?;
        let json: Value = serde_json::from_str(&body)
            .map_err(|error| format!("识别响应不是有效 JSON：{error}"))?;
        extract_openai_compatible_text(&json)
            .ok_or_else(|| "识别响应中没有可用文本。".to_string())?
    };
    eprintln!(
        "[saynow] recognition response parsed; text_chars={}",
        text.chars().count()
    );
    Ok(text)
}

fn uses_stream_response(payload: &Value) -> bool {
    payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn uses_mimo_api_key_header(config: &crate::db::AppConfig) -> bool {
    config.provider.eq_ignore_ascii_case("mimo")
        || config
            .base_url
            .to_ascii_lowercase()
            .contains("xiaomimimo.com")
}

fn resolve_api_key(api_key_ref: &str) -> Result<String, String> {
    let normalized = api_key_ref.trim();
    if normalized.is_empty() {
        return Err("API Key 为空。".to_string());
    }

    let value = if let Some(env_name) = normalized.strip_prefix("env:") {
        std::env::var(env_name).map_err(|_| format!("环境变量 {env_name} 未设置。"))?
    } else if normalized == "credential-manager:mimo" {
        std::env::var("SAYNOW_MIMO_API_KEY")
            .map_err(|_| "环境变量 SAYNOW_MIMO_API_KEY 未设置。".to_string())?
    } else if normalized == "credential-manager:qwen" {
        std::env::var("SAYNOW_QWEN_API_KEY")
            .map_err(|_| "环境变量 SAYNOW_QWEN_API_KEY 未设置。".to_string())?
    } else if let Some(key) = normalized.strip_prefix("literal:") {
        key.to_string()
    } else {
        normalized.to_string()
    };

    if value.trim().is_empty() {
        Err("API Key 为空。".to_string())
    } else {
        Ok(value)
    }
}

fn insert_failed_record(
    db: &AppDb,
    error: &str,
    duration_seconds: u32,
) -> Result<RecognitionRecord, String> {
    eprintln!("[saynow] recognize_audio failed: {error}");
    db.insert_record_with_error("", duration_seconds, RecognitionStatus::Failed, Some(error))
        .map_err(|db_error| db_error.to_string())?;
    let _ = latest_record(db)?;
    Err(error.to_string())
}

fn insert_cancelled_record(
    db: &AppDb,
    message: &str,
    duration_seconds: u32,
) -> Result<RecognitionRecord, String> {
    eprintln!("[saynow] recognize_audio cancelled: {message}");
    db.insert_record_with_error(
        "",
        duration_seconds,
        RecognitionStatus::Failed,
        Some(message),
    )
    .map_err(|error| error.to_string())?;
    latest_record(db)
}

fn latest_record(db: &AppDb) -> Result<RecognitionRecord, String> {
    db.list_records(1)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "failed to load recognition record".to_string())
}

pub fn open_external_url_data(url: &str) -> Result<(), String> {
    match url {
        "https://github.com/liujuntao123/saynow"
        | "https://github.com/liujuntao123/saynow/releases" => open_external_url_impl(url),
        _ => Err("不允许打开该链接。".to_string()),
    }
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
fn open_external_url_impl(url: &str) -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn()
        .map_err(|error| format!("无法打开浏览器：{error}"))?;
    Ok(())
}

#[cfg(all(feature = "desktop", target_os = "macos"))]
fn open_external_url_impl(url: &str) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(url)
        .spawn()
        .map_err(|error| format!("无法打开浏览器：{error}"))?;
    Ok(())
}

#[cfg(all(feature = "desktop", target_os = "linux"))]
fn open_external_url_impl(url: &str) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map_err(|error| format!("无法打开浏览器：{error}"))?;
    Ok(())
}

#[cfg(feature = "desktop")]
mod tauri_commands {
    use tauri::{Emitter, Manager, State};

    use super::*;

    #[tauri::command]
    pub fn get_dashboard(db: State<'_, AppDb>) -> Result<DashboardData, String> {
        dashboard_data(&db)
    }

    #[tauri::command]
    pub fn get_config(db: State<'_, AppDb>) -> Result<AppConfig, String> {
        get_config_data(&db)
    }

    #[tauri::command]
    pub fn save_config(db: State<'_, AppDb>, config: AppConfig) -> Result<AppConfig, String> {
        save_config_data(&db, config)
    }

    #[tauri::command]
    pub fn list_provider_configs(db: State<'_, AppDb>) -> Result<Vec<ProviderConfig>, String> {
        list_provider_configs_data(&db)
    }

    #[tauri::command]
    pub fn save_provider_config(
        db: State<'_, AppDb>,
        provider: ProviderConfig,
    ) -> Result<Vec<ProviderConfig>, String> {
        save_provider_config_data(&db, provider)
    }

    #[tauri::command]
    pub fn select_provider_config(db: State<'_, AppDb>, id: i64) -> Result<AppConfig, String> {
        select_provider_config_data(&db, id)
    }

    #[tauri::command]
    pub fn delete_provider_config(
        db: State<'_, AppDb>,
        id: i64,
    ) -> Result<Vec<ProviderConfig>, String> {
        delete_provider_config_data(&db, id)
    }

    #[tauri::command]
    pub fn list_records(db: State<'_, AppDb>) -> Result<Vec<RecognitionRecord>, String> {
        list_records_data(&db)
    }

    #[tauri::command]
    pub fn list_correction_records(db: State<'_, AppDb>) -> Result<Vec<CorrectionRecord>, String> {
        list_correction_records_data(&db)
    }

    #[tauri::command]
    pub fn save_correction(
        db: State<'_, AppDb>,
        input: SaveCorrectionInput,
    ) -> Result<CorrectionRecord, String> {
        save_correction_data(&db, input)
    }

    #[tauri::command]
    pub fn list_learning_rules(db: State<'_, AppDb>) -> Result<Vec<LearningRule>, String> {
        list_learning_rules_data(&db)
    }

    #[tauri::command]
    pub fn update_learning_rule_status(
        db: State<'_, AppDb>,
        id: i64,
        status: String,
    ) -> Result<Vec<LearningRule>, String> {
        update_learning_rule_status_data(&db, id, status)
    }

    #[tauri::command]
    pub fn delete_learning_rule(db: State<'_, AppDb>, id: i64) -> Result<Vec<LearningRule>, String> {
        delete_learning_rule_data(&db, id)
    }

    #[tauri::command]
    pub fn clear_learning_rules(db: State<'_, AppDb>) -> Result<Vec<LearningRule>, String> {
        clear_learning_rules_data(&db)
    }

    #[tauri::command]
    pub fn clear_correction_records(
        db: State<'_, AppDb>,
    ) -> Result<Vec<CorrectionRecord>, String> {
        clear_correction_records_data(&db)
    }

    #[tauri::command]
    pub fn delete_record(db: State<'_, AppDb>, id: i64) -> Result<Vec<RecognitionRecord>, String> {
        delete_record_data(&db, id)
    }

    #[tauri::command]
    pub fn clear_records(db: State<'_, AppDb>) -> Result<Vec<RecognitionRecord>, String> {
        clear_records_data(&db)
    }

    #[tauri::command]
    pub async fn test_provider_connection(provider: ProviderConfig) -> Result<String, String> {
        tauri::async_runtime::spawn_blocking(move || test_provider_connection_data(provider))
            .await
            .map_err(|error| format!("测试任务执行失败：{error}"))?
    }

    #[tauri::command]
    pub fn refresh_learning_rules(db: State<'_, AppDb>) -> Result<Vec<LearningRule>, String> {
        refresh_learning_rules_data(&db)
    }

    #[tauri::command]
    pub async fn run_learning_engine<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        force: bool,
    ) -> Result<Vec<LearningRule>, String> {
        // 学习请求可能长达数十秒，放到阻塞线程池，避免卡住主线程。
        tauri::async_runtime::spawn_blocking(move || {
            let db = app.state::<AppDb>();
            run_learning_engine_data(&db, force)
        })
        .await
        .map_err(|error| format!("学习任务执行失败：{error}"))?
    }

    #[tauri::command]
    pub fn list_vocabulary(db: State<'_, AppDb>) -> Result<Vec<VocabularyItem>, String> {
        list_vocabulary_data(&db)
    }

    #[tauri::command]
    pub fn add_vocabulary(
        db: State<'_, AppDb>,
        item: VocabularyItem,
    ) -> Result<Vec<VocabularyItem>, String> {
        add_vocabulary_data(&db, item)
    }

    #[tauri::command]
    pub fn add_vocabulary_terms(
        db: State<'_, AppDb>,
        terms: Vec<String>,
    ) -> Result<Vec<VocabularyItem>, String> {
        add_vocabulary_terms_data(&db, terms)
    }

    #[tauri::command]
    pub fn delete_vocabulary(db: State<'_, AppDb>, id: i64) -> Result<Vec<VocabularyItem>, String> {
        delete_vocabulary_data(&db, id)
    }

    #[tauri::command]
    pub fn update_vocabulary(
        db: State<'_, AppDb>,
        item: VocabularyItem,
    ) -> Result<Vec<VocabularyItem>, String> {
        update_vocabulary_data(&db, item)
    }

    #[tauri::command]
    pub fn list_style_prompts(db: State<'_, AppDb>) -> Result<Vec<StylePrompt>, String> {
        list_style_prompts_data(&db)
    }

    #[tauri::command]
    pub fn add_style_prompt(
        db: State<'_, AppDb>,
        item: StylePrompt,
    ) -> Result<Vec<StylePrompt>, String> {
        add_style_prompt_data(&db, item)
    }

    #[tauri::command]
    pub fn update_style_prompt(
        db: State<'_, AppDb>,
        item: StylePrompt,
    ) -> Result<Vec<StylePrompt>, String> {
        update_style_prompt_data(&db, item)
    }

    #[tauri::command]
    pub fn delete_style_prompt(db: State<'_, AppDb>, id: i64) -> Result<Vec<StylePrompt>, String> {
        delete_style_prompt_data(&db, id)
    }

    #[tauri::command]
    pub fn get_personalization_preferences(
        db: State<'_, AppDb>,
    ) -> Result<PersonalizationPreferences, String> {
        get_personalization_preferences_data(&db)
    }

    #[tauri::command]
    pub fn save_personalization_preferences(
        db: State<'_, AppDb>,
        preferences: PersonalizationPreferences,
    ) -> Result<PersonalizationPreferences, String> {
        save_personalization_preferences_data(&db, preferences)
    }

    #[tauri::command]
    pub fn get_learning_engine_config(
        db: State<'_, AppDb>,
    ) -> Result<LearningEngineConfig, String> {
        get_learning_engine_config_data(&db)
    }

    #[tauri::command]
    pub fn save_learning_engine_config(
        db: State<'_, AppDb>,
        config: LearningEngineConfig,
    ) -> Result<LearningEngineConfig, String> {
        save_learning_engine_config_data(&db, config)
    }

    #[tauri::command]
    pub async fn recognize_audio<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        input: RecognitionAudioInput,
    ) -> Result<RecognitionRecord, String> {
        crate::runtime_log::write_line("[saynow] tauri command recognize_audio invoked");
        recognize_audio_in_background(app, input).await
    }

    /// 用保留的上一段失败音频重试识别；无可重试音频时报错。
    #[tauri::command]
    pub async fn retry_last_recognition<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
    ) -> Result<RecognitionRecord, String> {
        crate::runtime_log::write_line("[saynow] tauri command retry_last_recognition invoked");
        let input = app
            .state::<LastRecognitionAudio>()
            .take_for_retry()
            .ok_or_else(|| "没有可重试的录音。".to_string())?;
        recognize_audio_in_background(app, input).await
    }

    async fn recognize_audio_in_background<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        input: RecognitionAudioInput,
    ) -> Result<RecognitionRecord, String> {
        // 识别请求走阻塞线程池，主线程保持响应（窗口、托盘、热键事件不受阻塞）。
        let task_app = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let db = task_app.state::<AppDb>();
            let stream_app = task_app.clone();
            let result = recognize_audio_data_with_transcript(&db, input.clone(), move |text| {
                let _ = stream_app.emit_to(
                    "recorder",
                    "recorder-transcript",
                    RecorderTranscriptPayload { text, done: false },
                );
            });
            let last_audio = task_app.state::<LastRecognitionAudio>();
            match &result {
                Ok(record) => {
                    last_audio.clear();
                    let _ = task_app.emit_to(
                        "recorder",
                        "recorder-transcript",
                        RecorderTranscriptPayload {
                            text: record.text.clone(),
                            done: true,
                        },
                    );
                }
                Err(_) => last_audio.store(&input),
            }
            result
        })
        .await
        .map_err(|error| format!("识别任务执行失败：{error}"))?;
        result
    }

    #[tauri::command]
    pub fn show_recorder_overlay_no_activate<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
    ) -> Result<(), String> {
        crate::runtime_log::write_line("[saynow] tauri command show_recorder_overlay_no_activate invoked");
        let window = recorder_window(&app)?;

        #[cfg(target_os = "windows")]
        {
            let hwnd = window
                .hwnd()
                .map_err(|error| format!("无法获取录音浮窗句柄：{error}"))?;
            crate::platform::configure_no_activate_window(hwnd.0 as isize)?;
            crate::platform::show_no_activate_window(hwnd.0 as isize)
        }

        #[cfg(not(target_os = "windows"))]
        {
            window.show().map_err(|error| error.to_string())
        }
    }

    #[tauri::command]
    pub fn show_recorder_overlay_focus<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
    ) -> Result<(), String> {
        crate::runtime_log::write_line("[saynow] tauri command show_recorder_overlay_focus invoked");
        let window = recorder_window(&app)?;

        #[cfg(target_os = "windows")]
        {
            let hwnd = window
                .hwnd()
                .map_err(|error| format!("无法获取录音浮窗句柄：{error}"))?;
            crate::platform::make_window_focusable(hwnd.0 as isize)?;
        }

        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn hide_recorder_overlay<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
    ) -> Result<(), String> {
        crate::runtime_log::write_line("[saynow] tauri command hide_recorder_overlay invoked");
        let window = recorder_window(&app)?;

        #[cfg(target_os = "windows")]
        {
            let hwnd = window
                .hwnd()
                .map_err(|error| format!("无法获取录音浮窗句柄：{error}"))?;
            crate::platform::hide_window(hwnd.0 as isize)
        }

        #[cfg(not(target_os = "windows"))]
        {
            window.hide().map_err(|error| error.to_string())
        }
    }

    #[tauri::command]
    pub fn set_recorder_overlay_position<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        x: i32,
        y: i32,
    ) -> Result<(), String> {
        recorder_window(&app)?
            .set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|error| error.to_string())
    }

    #[tauri::command]
    pub fn set_recorder_overlay_size<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        width: f64,
        height: f64,
    ) -> Result<(), String> {
        recorder_window(&app)?
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(|error| error.to_string())
    }

    fn recorder_window<R: tauri::Runtime>(
        app: &tauri::AppHandle<R>,
    ) -> Result<tauri::WebviewWindow<R>, String> {
        app.get_webview_window("recorder")
            .ok_or_else(|| "录音浮窗不存在。".to_string())
    }

    #[tauri::command]
    pub fn set_hotkey_monitor<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        parts: Option<Vec<String>>,
    ) -> Result<(), String> {
        crate::runtime_log::write_line(&format!(
            "[saynow] tauri command set_hotkey_monitor invoked; parts={parts:?}"
        ));
        crate::platform::set_hotkey_monitor(app, parts)
    }

    #[tauri::command]
    pub fn remember_input_target() -> Result<(), String> {
        crate::runtime_log::write_line("[saynow] tauri command remember_input_target invoked");
        crate::platform::remember_input_target()
    }

    #[tauri::command]
    pub fn write_runtime_log(message: String) {
        write_runtime_log_data(message);
    }

    #[tauri::command]
    pub fn get_runtime_log_path() -> Result<String, String> {
        get_runtime_log_path_data()
    }

    #[tauri::command]
    pub fn open_external_url(url: String) -> Result<(), String> {
        open_external_url_data(&url)
    }

    #[tauri::command]
    pub fn restore_input_target() -> Result<(), String> {
        crate::platform::restore_input_target()
    }

    #[tauri::command]
    pub fn undo_last_injected_text() -> Result<(), String> {
        undo_last_injected_text_data()
    }

    #[tauri::command]
    pub fn start_native_recording(recorder: State<'_, NativeAudioRecorder>) -> Result<(), String> {
        start_native_recording_data(&recorder)
    }

    #[tauri::command]
    pub fn stop_native_recording(
        recorder: State<'_, NativeAudioRecorder>,
    ) -> Result<Option<NativeRecordedAudio>, String> {
        stop_native_recording_data(&recorder)
    }

    #[tauri::command]
    pub fn cancel_native_recording(recorder: State<'_, NativeAudioRecorder>) -> Result<(), String> {
        cancel_native_recording_data(&recorder)
    }

    pub fn handlers<R: tauri::Runtime>(
    ) -> Box<dyn Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static> {
        Box::new(tauri::generate_handler![
            get_dashboard,
            get_config,
            save_config,
            list_provider_configs,
            save_provider_config,
            select_provider_config,
            delete_provider_config,
            test_provider_connection,
            list_records,
            delete_record,
            clear_records,
            list_correction_records,
            clear_correction_records,
            save_correction,
            list_learning_rules,
            update_learning_rule_status,
            delete_learning_rule,
            clear_learning_rules,
            refresh_learning_rules,
            run_learning_engine,
            list_vocabulary,
            add_vocabulary,
            add_vocabulary_terms,
            update_vocabulary,
            delete_vocabulary,
            list_style_prompts,
            add_style_prompt,
            update_style_prompt,
            delete_style_prompt,
            get_personalization_preferences,
            save_personalization_preferences,
            get_learning_engine_config,
            save_learning_engine_config,
            recognize_audio,
            retry_last_recognition,
            show_recorder_overlay_no_activate,
            show_recorder_overlay_focus,
            hide_recorder_overlay,
            set_recorder_overlay_position,
            set_recorder_overlay_size,
            set_hotkey_monitor,
            remember_input_target,
            write_runtime_log,
            get_runtime_log_path,
            open_external_url,
            restore_input_target,
            undo_last_injected_text,
            start_native_recording,
            stop_native_recording,
            cancel_native_recording
        ])
    }
}

#[cfg(feature = "desktop")]
pub use tauri_commands::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_manage_vocabulary_and_style_prompts() {
        let db = AppDb::in_memory().unwrap();

        let vocabulary =
            add_vocabulary_terms_data(&db, vec!["Qwen".to_string(), "MiMo".to_string()]).unwrap();
        assert_eq!(vocabulary.len(), 2);

        let vocabulary = delete_vocabulary_data(&db, vocabulary[0].id).unwrap();
        assert_eq!(vocabulary.len(), 1);

        let styles = add_style_prompt_data(
            &db,
            StylePrompt {
                id: 0,
                name: "书面语".to_string(),
                prompt: "整理为书面语".to_string(),
                enabled: true,
            },
        )
        .unwrap();
        let mut style = styles[0].clone();
        style.enabled = false;

        let styles = update_style_prompt_data(&db, style).unwrap();
        assert!(!styles[0].enabled);

        let styles = delete_style_prompt_data(&db, styles[0].id).unwrap();
        assert!(styles.is_empty());
    }

    #[test]
    fn commands_manage_provider_configs() {
        let db = AppDb::in_memory().unwrap();

        assert!(list_provider_configs_data(&db).unwrap().is_empty());

        let providers = save_provider_config_data(
            &db,
            ProviderConfig {
                id: 0,
                provider: "Qwen".to_string(),
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                model: "qwen3.5-omni-plus".to_string(),
                api_key_ref: "credential-manager:qwen".to_string(),
                enabled: true,
                kind: "openai-compatible".to_string(),
                extra_json: "{}".to_string(),
            },
        )
        .unwrap();
        let qwen = providers
            .iter()
            .find(|provider| provider.provider == "Qwen")
            .unwrap();
        assert!(qwen.enabled);
        assert_eq!(get_config_data(&db).unwrap().model, "qwen3.5-omni-plus");

        let providers = save_provider_config_data(
            &db,
            ProviderConfig {
                id: 0,
                provider: "Custom".to_string(),
                base_url: "https://example.test/v1".to_string(),
                model: "custom-model".to_string(),
                api_key_ref: "literal:test-key".to_string(),
                enabled: false,
                kind: "openai-compatible".to_string(),
                extra_json: "{}".to_string(),
            },
        )
        .unwrap();
        let custom = providers
            .iter()
            .find(|provider| provider.provider == "Custom")
            .unwrap();
        let config = select_provider_config_data(&db, custom.id).unwrap();
        assert_eq!(config.provider, "Custom");

        let providers = delete_provider_config_data(&db, qwen.id).unwrap();
        assert!(providers.iter().all(|provider| provider.provider != "Qwen"));
    }

    #[test]
    fn saves_manual_correction_without_replacement() {
        let db = AppDb::in_memory().unwrap();

        let correction = save_correction_data(
            &db,
            SaveCorrectionInput {
                recognition_record_id: 42,
                raw_text: "status不应该会是一吧".to_string(),
                corrected_text: "status不应该会是1吧".to_string(),
                source: "post-insert-overlay".to_string(),
                apply_replacement: false,
            },
        )
        .unwrap();

        assert_eq!(correction.recognition_record_id, 42);
        assert_eq!(correction.raw_text, "status不应该会是一吧");
        assert_eq!(correction.corrected_text, "status不应该会是1吧");
        assert_eq!(correction.source, "post-insert-overlay");
        assert!(!correction.applied);
        assert!(correction.error_message.is_none());
        assert_eq!(list_correction_records_data(&db).unwrap().len(), 1);
        assert!(list_learning_rules_data(&db).unwrap().is_empty());
    }

    #[test]
    fn skips_unchanged_manual_correction() {
        let db = AppDb::in_memory().unwrap();

        let error = save_correction_data(
            &db,
            SaveCorrectionInput {
                recognition_record_id: 42,
                raw_text: "没有变化".to_string(),
                corrected_text: "没有变化".to_string(),
                source: "post-insert-overlay".to_string(),
                apply_replacement: false,
            },
        )
        .unwrap_err();

        assert!(error.contains("相同"));
        assert!(list_correction_records_data(&db).unwrap().is_empty());
    }

    #[test]
    fn repeated_corrections_promote_learning_rule_candidate() {
        let db = AppDb::in_memory().unwrap();
        save_learning_engine_config_data(
            &db,
            LearningEngineConfig {
                enabled: true,
                run_mode: "localOnly".to_string(),
                min_new_corrections: 2,
                ..LearningEngineConfig::default()
            },
        )
        .unwrap();

        for (raw_text, corrected_text) in [
            ("status不应该会是一吧", "status不应该会是1吧"),
            ("这个status又是一", "这个status又是1"),
        ] {
            save_correction_data(
                &db,
                SaveCorrectionInput {
                    recognition_record_id: 42,
                    raw_text: raw_text.to_string(),
                    corrected_text: corrected_text.to_string(),
                    source: "post-insert-overlay".to_string(),
                    apply_replacement: false,
                },
            )
            .unwrap();
        }

        run_learning_engine_data(&db, false).unwrap();
        let rules = list_learning_rules_data(&db).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].status, "candidate");
        assert_eq!(rules[0].evidence_correction_ids, "1,2");
        assert_eq!(rules[0].evidence_recognition_ids, "");
        assert!(rules[0].confidence > 0.7);
    }

    #[test]
    fn learning_engine_processes_successful_recognition_history_samples() {
        let db = AppDb::in_memory().unwrap();
        save_learning_engine_config_data(
            &db,
            LearningEngineConfig {
                enabled: true,
                run_mode: "localOnly".to_string(),
                min_new_corrections: 3,
                ..LearningEngineConfig::default()
            },
        )
        .unwrap();

        for text in [
            "今天继续讨论 payload 字段的解析逻辑",
            "payload 里面的 status 还是要单独处理",
            "这个 payload 转换和 status 校验再看一下",
        ] {
            db.insert_record(text, 8, RecognitionStatus::Success).unwrap();
        }

        run_learning_engine_data(&db, false).unwrap();

        assert!(db.list_unprocessed_recognition_records(10).unwrap().is_empty());
        assert!(list_learning_rules_data(&db).unwrap().is_empty());
    }

    #[test]
    fn manages_learning_engine_config() {
        let db = AppDb::in_memory().unwrap();

        let default_config = get_learning_engine_config_data(&db).unwrap();
        assert!(!default_config.enabled);
        assert_eq!(default_config.run_mode, "llmAssist");

        let missing_provider = save_learning_engine_config_data(
            &db,
            LearningEngineConfig {
                enabled: true,
                ..LearningEngineConfig::default()
            },
        )
        .unwrap_err();
        assert!(missing_provider.contains("完整"));

        let saved = save_learning_engine_config_data(
            &db,
            LearningEngineConfig {
                enabled: true,
                provider: "OpenAI".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-4.1-mini".to_string(),
                api_key_ref: "credential-manager:openai".to_string(),
                run_mode: "llmAssist".to_string(),
                min_new_corrections: 0,
                idle_seconds: 1,
            },
        )
        .unwrap();

        assert!(saved.enabled);
        assert_eq!(saved.min_new_corrections, 1);
        assert_eq!(saved.idle_seconds, 5);
        assert_eq!(
            get_learning_engine_config_data(&db).unwrap().model,
            "gpt-4.1-mini"
        );
    }

    #[test]
    fn resolves_literal_api_keys_without_environment() {
        assert_eq!(resolve_api_key("literal:test-key").unwrap(), "test-key");
        assert_eq!(resolve_api_key("direct-key").unwrap(), "direct-key");
    }

    #[test]
    fn failed_recognition_persists_error_record() {
        let db = AppDb::in_memory().unwrap();
        db.save_config(&AppConfig {
            provider: "MiMo".to_string(),
            base_url: "https://api.xiaomimimo.com/v1".to_string(),
            model: "mimo-v2.5".to_string(),
            api_key_ref: "credential-manager:mimo".to_string(),
            hotkey: "LCtrl+Meta".to_string(),
            ..Default::default()
        })
        .unwrap();

        let error = recognize_audio_data(
            &db,
            RecognitionAudioInput {
                audio_base64: "YXVkaW8=".to_string(),
                duration_seconds: 2,
                mime_type: "audio/webm".to_string(),
            },
        )
        .unwrap_err();
        let records = db.list_records(1).unwrap();

        assert!(error.contains("环境变量 SAYNOW_MIMO_API_KEY 未设置"));
        assert_eq!(records[0].status, RecognitionStatus::Failed);
        assert_eq!(records[0].duration_seconds, 2);
    }

    #[test]
    fn recognition_without_provider_config_persists_clear_error() {
        let db = AppDb::in_memory().unwrap();

        let error = recognize_audio_data(
            &db,
            RecognitionAudioInput {
                audio_base64: "YXVkaW8=".to_string(),
                duration_seconds: 2,
                mime_type: "audio/webm".to_string(),
            },
        )
        .unwrap_err();
        let records = db.list_records(1).unwrap();

        assert!(error.contains("请先在配置页添加并启用识别供应商"));
        assert_eq!(records[0].provider, "");
        assert_eq!(records[0].model, "");
        assert_eq!(records[0].status, RecognitionStatus::Failed);
    }

    #[test]
    fn empty_recognition_text_cancels_insertion_without_injecting() {
        let db = AppDb::in_memory().unwrap();
        let mut injected = false;

        let record = persist_recognized_text(&db, " \n\t ".to_string(), 0, |_| {
            injected = true;
            Ok(())
        })
        .unwrap();

        assert!(!injected);
        assert_eq!(record.status, RecognitionStatus::Failed);
        assert_eq!(record.text, "");
        assert_eq!(record.duration_seconds, 1);
        assert!(record
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("已取消插入"));
    }

    #[test]
    fn personalization_preferences_default_hide_correction_strip() {
        let db = AppDb::in_memory().unwrap();

        let defaults = get_personalization_preferences_data(&db).unwrap();
        assert!(!defaults.show_correction_strip);

        let saved = save_personalization_preferences_data(
            &db,
            PersonalizationPreferences {
                remove_trailing_period: true,
                show_correction_strip: true,
            },
        )
        .unwrap();
        assert!(saved.show_correction_strip);

        let loaded = get_personalization_preferences_data(&db).unwrap();
        assert!(loaded.remove_trailing_period);
        assert!(loaded.show_correction_strip);
    }

    #[test]
    fn text_preferences_remove_trailing_period_only_when_enabled() {
        let enabled = PersonalizationPreferences {
            remove_trailing_period: true,
            show_correction_strip: false,
        };
        let disabled = PersonalizationPreferences {
            remove_trailing_period: false,
            show_correction_strip: false,
        };

        assert_eq!(
            apply_text_preferences("你好。".to_string(), &enabled),
            "你好"
        );
        assert_eq!(
            apply_text_preferences("hello.".to_string(), &enabled),
            "hello"
        );
        assert_eq!(
            apply_text_preferences("你好！".to_string(), &enabled),
            "你好！"
        );
        assert_eq!(
            apply_text_preferences("你好。".to_string(), &disabled),
            "你好。"
        );
    }

    #[test]
    fn text_preferences_remove_horizontal_spaces_only_between_han_characters() {
        let preferences = PersonalizationPreferences {
            remove_trailing_period: false,
            show_correction_strip: false,
        };

        assert_eq!(
            apply_text_preferences("你 好  世\t界".to_string(), &preferences),
            "你好世界"
        );
        assert_eq!(
            apply_text_preferences("你好 world 2026 年".to_string(), &preferences),
            "你好 world 2026 年"
        );
        assert_eq!(
            apply_text_preferences("第一段\n第二段".to_string(), &preferences),
            "第一段\n第二段"
        );
    }
}
