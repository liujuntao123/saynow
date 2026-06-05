use std::io::BufRead;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    db::{AppConfig, AppDb, ProviderConfig},
    models::{
        PersonalizationPreferences, RecognitionRecord, RecognitionStatus, StylePrompt,
        VocabularyItem,
    },
    platform::{current_platform_status, inject_text, PlatformStatus},
    prompt::build_prompt_context,
    provider::{
        build_openai_compatible_payload, extract_openai_compatible_text, first_qwen_stream_text,
        is_qwen_provider, push_qwen_stream_line,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionAudioInput {
    pub audio_base64: String,
    pub duration_seconds: u32,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderTranscriptPayload {
    pub text: String,
    pub done: bool,
}

pub fn dashboard_data(db: &AppDb) -> Result<DashboardData, String> {
    let records = db.list_records(50).map_err(|error| error.to_string())?;
    Ok(DashboardData {
        stats: aggregate_usage_stats(&records),
        records,
        platform: current_platform_status(),
    })
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

pub fn delete_provider_config_data(db: &AppDb, id: i64) -> Result<Vec<ProviderConfig>, String> {
    db.delete_provider_config(id)
        .map_err(|error| error.to_string())?;
    list_provider_configs_data(db)
}

pub fn list_records_data(db: &AppDb) -> Result<Vec<RecognitionRecord>, String> {
    db.list_records(200).map_err(|error| error.to_string())
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
    eprintln!(
        "[saynow] recognize_audio started; duration={}s mime={} base64_len={}",
        input.duration_seconds,
        input.mime_type,
        input.audio_base64.len()
    );
    if input.audio_base64.trim().is_empty() {
        eprintln!("[saynow] recognize_audio rejected empty audio");
        return insert_failed_record(db, "录音数据为空。", input.duration_seconds.max(1));
    }

    let config = db.get_config().map_err(|error| error.to_string())?;
    if !config.has_complete_provider() {
        return insert_failed_record(
            db,
            "请先在配置页添加并启用大模型供应商。",
            input.duration_seconds.max(1),
        );
    }
    let vocabulary = db.list_vocabulary().map_err(|error| error.to_string())?;
    let styles = db.list_style_prompts().map_err(|error| error.to_string())?;
    let records = db.list_records(10).map_err(|error| error.to_string())?;
    let prompt = build_prompt_context(&vocabulary, &styles, &records);
    eprintln!(
        "[saynow] recognize_audio building request; provider={} model={} mime={} prompt_chars={}",
        config.provider,
        config.model,
        input.mime_type,
        prompt.chars().count()
    );
    let payload = build_openai_compatible_payload(
        &config.provider,
        &config.model,
        &prompt,
        &input.audio_base64,
        &input.mime_type,
    );

    let recognition_result =
        call_openai_compatible_chat_with_transcript(&config, payload, on_transcript);
    let text = match recognition_result {
        Ok(text) => text,
        Err(error) => return insert_failed_record(db, &error, input.duration_seconds.max(1)),
    };

    let preferences = db
        .get_personalization_preferences()
        .map_err(|error| error.to_string())?;
    let text = apply_text_preferences(text, &preferences);
    persist_recognized_text(db, text, input.duration_seconds, inject_text)
}

fn apply_text_preferences(text: String, preferences: &PersonalizationPreferences) -> String {
    if !preferences.remove_trailing_period {
        return text;
    }

    let mut text = text;
    if matches!(text.chars().last(), Some('。' | '.')) {
        text.pop();
    }
    text
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
    let request = reqwest::blocking::Client::new().post(url);
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

    let text = if uses_qwen_stream_response(config) {
        let mut content = String::new();
        let mut reasoning_content = String::new();
        let reader = std::io::BufReader::new(response);
        for line in reader.lines() {
            let line = line.map_err(|error| format!("读取识别响应失败：{error}"))?;
            if let Some(text) = push_qwen_stream_line(&line, &mut content, &mut reasoning_content) {
                on_transcript(text);
            }
        }
        first_qwen_stream_text(content, reasoning_content)
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

fn uses_qwen_stream_response(config: &crate::db::AppConfig) -> bool {
    is_qwen_provider(&config.provider) || config.model.to_ascii_lowercase().contains("qwen")
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
    pub fn recognize_audio<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        db: State<'_, AppDb>,
        input: RecognitionAudioInput,
    ) -> Result<RecognitionRecord, String> {
        let stream_app = app.clone();
        let result = recognize_audio_data_with_transcript(&db, input, move |text| {
            let _ = stream_app.emit_to(
                "recorder",
                "recorder-transcript",
                RecorderTranscriptPayload { text, done: false },
            );
        });
        if let Ok(record) = &result {
            let _ = app.emit_to(
                "recorder",
                "recorder-transcript",
                RecorderTranscriptPayload {
                    text: record.text.clone(),
                    done: true,
                },
            );
        }
        result
    }

    #[tauri::command]
    pub fn show_recorder_overlay_no_activate<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
    ) -> Result<(), String> {
        let window = recorder_window(&app)?;

        #[cfg(target_os = "windows")]
        {
            let hwnd = window
                .hwnd()
                .map_err(|error| format!("无法获取录音浮窗句柄：{error}"))?;
            crate::platform::show_no_activate_window(hwnd.0 as isize)
        }

        #[cfg(not(target_os = "windows"))]
        {
            window.show().map_err(|error| error.to_string())
        }
    }

    #[tauri::command]
    pub fn hide_recorder_overlay<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
    ) -> Result<(), String> {
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
        crate::platform::set_hotkey_monitor(app, parts)
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
            list_records,
            list_vocabulary,
            add_vocabulary,
            add_vocabulary_terms,
            delete_vocabulary,
            list_style_prompts,
            add_style_prompt,
            update_style_prompt,
            delete_style_prompt,
            get_personalization_preferences,
            save_personalization_preferences,
            recognize_audio,
            show_recorder_overlay_no_activate,
            hide_recorder_overlay,
            set_recorder_overlay_position,
            set_hotkey_monitor
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
            hotkey: "Alt".to_string(),
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

        assert!(error.contains("请先在配置页添加并启用大模型供应商"));
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
    fn text_preferences_remove_trailing_period_only_when_enabled() {
        let enabled = PersonalizationPreferences {
            remove_trailing_period: true,
        };
        let disabled = PersonalizationPreferences {
            remove_trailing_period: false,
        };

        assert_eq!(apply_text_preferences("你好。".to_string(), &enabled), "你好");
        assert_eq!(apply_text_preferences("hello.".to_string(), &enabled), "hello");
        assert_eq!(apply_text_preferences("你好！".to_string(), &enabled), "你好！");
        assert_eq!(apply_text_preferences("你好。".to_string(), &disabled), "你好。");
    }
}
