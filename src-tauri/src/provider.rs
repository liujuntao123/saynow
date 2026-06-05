use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub api_key_ref: String,
}

pub fn build_openai_compatible_payload(
    provider: &str,
    model: &str,
    prompt: &str,
    audio_base64: &str,
    mime_type: &str,
) -> Value {
    if is_qwen_provider(provider) || model.to_ascii_lowercase().contains("qwen") {
        return build_qwen_omni_payload(model, prompt, audio_base64, mime_type);
    }

    build_mimo_payload(model, prompt, audio_base64, mime_type)
}

fn build_mimo_payload(model: &str, prompt: &str, audio_base64: &str, mime_type: &str) -> Value {
    let audio_data = audio_data_url_with_mime(audio_base64, mime_type);
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": prompt
            },
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": "请识别这段音频，只输出最终文本。" },
                    {
                        "type": "input_audio",
                        "input_audio": {
                            "data": audio_data
                        }
                    }
                ]
            }
        ],
        "temperature": 0.1,
        "max_completion_tokens": 1024
    })
}

fn build_qwen_omni_payload(
    model: &str,
    prompt: &str,
    audio_base64: &str,
    mime_type: &str,
) -> Value {
    let audio_data = audio_data_url_without_mime(audio_base64);
    let audio_format = audio_format_from_mime(mime_type);
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": prompt
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "input_audio",
                        "input_audio": {
                            "data": audio_data,
                            "format": audio_format
                        }
                    },
                    { "type": "text", "text": "请识别这段音频，只输出最终文本。" }
                ]
            }
        ],
        "modalities": ["text"],
        "stream": true,
        "stream_options": {
            "include_usage": true
        }
    })
}

pub fn extract_openai_compatible_text(response: &Value) -> Option<String> {
    [
        "/choices/0/message/content",
        "/choices/0/message/reasoning_content",
    ]
    .iter()
    .filter_map(|path| response.pointer(path).and_then(Value::as_str))
    .map(str::trim)
    .find(|text| !text.is_empty())
    .map(ToOwned::to_owned)
}

pub fn extract_qwen_stream_text(body: &str) -> Option<String> {
    let mut content = String::new();
    let mut reasoning_content = String::new();

    for line in body.lines() {
        push_qwen_stream_line(line, &mut content, &mut reasoning_content);
    }

    first_qwen_stream_text(content, reasoning_content)
}

pub fn push_qwen_stream_line(
    line: &str,
    content: &mut String,
    reasoning_content: &mut String,
) -> Option<String> {
    if let Some(text) = extract_qwen_stream_delta(line, "/choices/0/delta/content") {
        content.push_str(&text);
        return Some(content.clone());
    }
    if let Some(text) = extract_qwen_stream_delta(line, "/choices/0/delta/reasoning_content") {
        reasoning_content.push_str(&text);
    }
    None
}

pub fn first_qwen_stream_text(content: String, reasoning_content: String) -> Option<String> {
    first_non_empty_text([content, reasoning_content])
}

fn extract_qwen_stream_delta(line: &str, path: &str) -> Option<String> {
    let line = line.trim();
    let data = line.strip_prefix("data:")?.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }

    let chunk = serde_json::from_str::<Value>(data).ok()?;
    chunk
        .pointer(path)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn first_non_empty_text<const N: usize>(candidates: [String; N]) -> Option<String> {
    candidates
        .into_iter()
        .map(|text| text.trim().to_string())
        .find(|text| !text.is_empty())
}

fn audio_data_url_with_mime(audio_base64: &str, mime_type: &str) -> String {
    let trimmed = audio_base64.trim();
    if trimmed.starts_with("data:") {
        trimmed.to_string()
    } else {
        format!(
            "data:{};base64,{}",
            normalize_audio_mime_type(mime_type),
            trimmed
        )
    }
}

fn audio_data_url_without_mime(audio_base64: &str) -> String {
    let trimmed = audio_base64.trim();
    if trimmed.starts_with("data:") {
        trimmed.to_string()
    } else {
        format!("data:;base64,{trimmed}")
    }
}

fn audio_format_from_mime(mime_type: &str) -> &'static str {
    let mime = mime_type.to_ascii_lowercase();
    if mime.contains("wav") {
        "wav"
    } else if mime.contains("mpeg") || mime.contains("mp3") {
        "mp3"
    } else if mime.contains("flac") {
        "flac"
    } else if mime.contains("ogg") {
        "ogg"
    } else if mime.contains("mp4") || mime.contains("m4a") {
        "m4a"
    } else {
        "wav"
    }
}

fn normalize_audio_mime_type(mime_type: &str) -> &'static str {
    let mime = mime_type.to_ascii_lowercase();
    if mime.contains("wav") {
        "audio/wav"
    } else if mime.contains("mpeg") || mime.contains("mp3") {
        "audio/mpeg"
    } else if mime.contains("flac") {
        "audio/flac"
    } else if mime.contains("ogg") {
        "audio/ogg"
    } else if mime.contains("mp4") || mime.contains("m4a") {
        "audio/mp4"
    } else {
        "audio/wav"
    }
}

pub fn is_qwen_provider(provider: &str) -> bool {
    provider.to_ascii_lowercase().contains("qwen")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_mimo_payload_with_model_prompt_and_audio() {
        let payload =
            build_openai_compatible_payload("MiMo", "mimo-v2.5", "prompt", "AAA", "audio/wav");

        assert_eq!(payload["model"], "mimo-v2.5");
        assert_eq!(payload["messages"][0]["content"], "prompt");
        assert_eq!(payload["messages"][1]["content"][0]["type"], "text");
        assert_eq!(
            payload["messages"][1]["content"][1]["input_audio"]["data"],
            "data:audio/wav;base64,AAA"
        );
        assert!(payload["messages"][1]["content"][1]["input_audio"]["format"].is_null());
    }

    #[test]
    fn builds_qwen_payload_with_required_streaming_audio_fields() {
        let payload = build_openai_compatible_payload(
            "Qwen",
            "qwen3.5-omni-plus",
            "prompt",
            "AAA",
            "audio/wav",
        );

        assert_eq!(payload["model"], "qwen3.5-omni-plus");
        assert_eq!(payload["stream"], true);
        assert_eq!(payload["modalities"][0], "text");
        assert_eq!(
            payload["messages"][1]["content"][0]["input_audio"]["data"],
            "data:;base64,AAA"
        );
        assert_eq!(
            payload["messages"][1]["content"][0]["input_audio"]["format"],
            "wav"
        );
    }

    #[test]
    fn extracts_text_from_chat_completion_response() {
        let response = json!({
            "choices": [
                { "message": { "content": "  识别结果  " } }
            ]
        });

        assert_eq!(
            extract_openai_compatible_text(&response),
            Some("识别结果".to_string())
        );
    }

    #[test]
    fn extracts_text_from_mimo_reasoning_content_response() {
        let response = json!({
            "choices": [
                { "message": { "content": "", "reasoning_content": "  Good morning.  " } }
            ]
        });

        assert_eq!(
            extract_openai_compatible_text(&response),
            Some("Good morning.".to_string())
        );
    }

    #[test]
    fn extracts_text_from_qwen_stream_response() {
        let body = r#"
data: {"choices":[{"delta":{"content":"早"}}]}
data: {"choices":[{"delta":{"content":"上好"}}]}
data: {"choices":[],"usage":{"total_tokens":10}}
"#;

        assert_eq!(extract_qwen_stream_text(body), Some("早上好".to_string()));
    }
}
