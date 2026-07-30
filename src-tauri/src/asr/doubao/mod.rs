mod audio;
mod client;
pub mod config;
mod protocol;

pub use config::{
    is_doubao_provider, DoubaoAuthMode, DoubaoProviderSettings, DEFAULT_DOUBAO_ENDPOINT,
    DEFAULT_DOUBAO_RESOURCE_ID, KIND_DOUBAO_WS, KIND_OPENAI_COMPATIBLE,
};

use crate::commands::RecognitionAudioInput;
use crate::db::AppConfig;

use audio::pcm16le_from_recognition_input;
use client::{recognize_pcm_stream, test_connection};
use config::settings_from_app_config;

/// 录完再识别：WAV/base64 → 16k mono PCM → 豆包 WebSocket 分包上传。
pub fn recognize_doubao_with_transcript<F>(
    config: &AppConfig,
    input: &RecognitionAudioInput,
    hotwords: &[String],
    on_transcript: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    let settings = settings_from_app_config(config)?;
    let pcm = pcm16le_from_recognition_input(&input.audio_base64, &input.mime_type)?;
    if pcm.is_empty() {
        return Err("录音数据为空。".to_string());
    }

    eprintln!(
        "[saynow] doubao asr start; endpoint={} resource={} pcm_bytes={} hotwords={}",
        settings.endpoint,
        settings.resource_id,
        pcm.len(),
        hotwords.len()
    );

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("无法创建识别运行时：{error}"))?;

    runtime.block_on(recognize_pcm_stream(
        &settings,
        &pcm,
        hotwords,
        on_transcript,
    ))
}

pub fn test_doubao_connection(config: &AppConfig) -> Result<String, String> {
    let settings = settings_from_app_config(config)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("无法创建连接测试运行时：{error}"))?;
    runtime.block_on(test_connection(&settings))
}
