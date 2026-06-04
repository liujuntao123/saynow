use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    db::{AppConfig, AppDb},
    models::{RecognitionRecord, RecognitionStatus, StylePrompt, VocabularyItem},
    platform::{current_platform_status, inject_text, PlatformStatus},
    prompt::build_prompt_context,
    provider::{build_openai_compatible_payload, extract_openai_compatible_text},
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

pub fn simulate_recognition_data(db: &AppDb) -> Result<RecognitionRecord, String> {
    let vocabulary = db.list_vocabulary().map_err(|error| error.to_string())?;
    let styles = db.list_style_prompts().map_err(|error| error.to_string())?;
    let records = db.list_records(10).map_err(|error| error.to_string())?;
    let context = build_prompt_context(&vocabulary, &styles, &records);
    let text = if context.contains("书面语") {
        "这是一次模拟语音识别结果，已按照书面语风格整理。"
    } else {
        "这是一次模拟语音识别结果。"
    };
    db.insert_record(text, 6, RecognitionStatus::Success)
        .map_err(|error| error.to_string())?;
    db.list_records(1)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "failed to load simulated record".to_string())
}

pub fn recognize_audio_data(
    db: &AppDb,
    input: RecognitionAudioInput,
) -> Result<RecognitionRecord, String> {
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
    let vocabulary = db.list_vocabulary().map_err(|error| error.to_string())?;
    let styles = db.list_style_prompts().map_err(|error| error.to_string())?;
    let records = db.list_records(10).map_err(|error| error.to_string())?;
    let prompt = build_prompt_context(&vocabulary, &styles, &records);
    let format = audio_format_from_mime(&input.mime_type);
    eprintln!(
        "[saynow] recognize_audio building request; provider={} model={} format={} prompt_chars={}",
        config.provider,
        config.model,
        format,
        prompt.chars().count()
    );
    let payload =
        build_openai_compatible_payload(&config.model, &prompt, &input.audio_base64, &format);

    let recognition_result = call_openai_compatible_chat(&config, payload);
    let text = match recognition_result {
        Ok(text) => text,
        Err(error) => return insert_failed_record(db, &error, input.duration_seconds.max(1)),
    };

    let injection_error = inject_text(&text).err();
    if let Some(error) = injection_error.as_deref() {
        eprintln!("[saynow] text injection failed: {error}");
    } else {
        eprintln!("[saynow] text injection finished");
    }
    db.insert_record_with_error(
        &text,
        input.duration_seconds.max(1),
        RecognitionStatus::Success,
        injection_error.as_deref(),
    )
    .map_err(|error| error.to_string())?;
    latest_record(db)
}

fn call_openai_compatible_chat(
    config: &crate::db::AppConfig,
    payload: Value,
) -> Result<String, String> {
    let api_key = resolve_api_key(&config.api_key_ref)?;
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    eprintln!(
        "[saynow] sending recognition request; url={} model={}",
        url, config.model
    );
    let response = reqwest::blocking::Client::new()
        .post(url)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .map_err(|error| format!("识别请求失败：{error}"))?;
    let status = response.status();
    eprintln!("[saynow] recognition response status={status}");
    let body = response
        .text()
        .map_err(|error| format!("读取识别响应失败：{error}"))?;
    if !status.is_success() {
        return Err(format!("识别请求返回 {status}：{body}"));
    }

    let json: Value =
        serde_json::from_str(&body).map_err(|error| format!("识别响应不是有效 JSON：{error}"))?;
    let text = extract_openai_compatible_text(&json)
        .ok_or_else(|| "识别响应中没有可用文本。".to_string())?;
    eprintln!(
        "[saynow] recognition response parsed; text_chars={}",
        text.chars().count()
    );
    Ok(text)
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

fn audio_format_from_mime(mime_type: &str) -> String {
    let mime = mime_type.to_ascii_lowercase();
    if mime.contains("wav") {
        "wav"
    } else if mime.contains("mpeg") || mime.contains("mp3") {
        "mp3"
    } else if mime.contains("ogg") {
        "ogg"
    } else if mime.contains("webm") {
        "webm"
    } else if mime.contains("mp4") || mime.contains("m4a") {
        "mp4"
    } else {
        "webm"
    }
    .to_string()
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

fn latest_record(db: &AppDb) -> Result<RecognitionRecord, String> {
    db.list_records(1)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "failed to load recognition record".to_string())
}

#[cfg(feature = "desktop")]
mod tauri_commands {
    use tauri::State;

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
    pub fn simulate_recognition(db: State<'_, AppDb>) -> Result<RecognitionRecord, String> {
        simulate_recognition_data(&db)
    }

    #[tauri::command]
    pub fn recognize_audio(
        db: State<'_, AppDb>,
        input: RecognitionAudioInput,
    ) -> Result<RecognitionRecord, String> {
        recognize_audio_data(&db, input)
    }

    #[tauri::command]
    pub fn set_modifier_hotkey_monitor<R: tauri::Runtime>(
        app: tauri::AppHandle<R>,
        parts: Option<Vec<String>>,
    ) -> Result<(), String> {
        crate::platform::set_modifier_hotkey_monitor(app, parts)
    }

    pub fn handlers<R: tauri::Runtime>(
    ) -> Box<dyn Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static> {
        Box::new(tauri::generate_handler![
            get_dashboard,
            get_config,
            save_config,
            list_records,
            list_vocabulary,
            add_vocabulary,
            add_vocabulary_terms,
            delete_vocabulary,
            list_style_prompts,
            add_style_prompt,
            update_style_prompt,
            delete_style_prompt,
            simulate_recognition,
            recognize_audio,
            set_modifier_hotkey_monitor
        ])
    }
}

#[cfg(feature = "desktop")]
pub use tauri_commands::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulated_recognition_persists_a_success_record() {
        let db = AppDb::in_memory().unwrap();

        let record = simulate_recognition_data(&db).unwrap();
        let dashboard = dashboard_data(&db).unwrap();

        assert_eq!(record.status, RecognitionStatus::Success);
        assert_eq!(dashboard.stats.total_records, 1);
        assert_eq!(dashboard.records[0].text, record.text);
    }

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
    fn resolves_supported_audio_formats_from_mime_types() {
        assert_eq!(audio_format_from_mime("audio/wav"), "wav");
        assert_eq!(audio_format_from_mime("audio/webm;codecs=opus"), "webm");
        assert_eq!(audio_format_from_mime("audio/mpeg"), "mp3");
    }

    #[test]
    fn resolves_literal_api_keys_without_environment() {
        assert_eq!(resolve_api_key("literal:test-key").unwrap(), "test-key");
        assert_eq!(resolve_api_key("direct-key").unwrap(), "direct-key");
    }

    #[test]
    fn failed_recognition_persists_error_record() {
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

        assert!(error.contains("环境变量 SAYNOW_MIMO_API_KEY 未设置"));
        assert_eq!(records[0].status, RecognitionStatus::Failed);
        assert_eq!(records[0].duration_seconds, 2);
    }
}
