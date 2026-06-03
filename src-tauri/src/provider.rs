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
    model: &str,
    prompt: &str,
    audio_base64: &str,
    audio_format: &str,
) -> Value {
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
                    { "type": "input_text", "text": "请识别这段音频，只输出最终文本。" },
                    {
                        "type": "input_audio",
                        "input_audio": {
                            "data": audio_base64,
                            "format": audio_format
                        }
                    }
                ]
            }
        ],
        "temperature": 0.1
    })
}

pub fn extract_openai_compatible_text(response: &Value) -> Option<String> {
    response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_payload_with_model_prompt_and_audio() {
        let payload = build_openai_compatible_payload("mimo-v2.5", "prompt", "AAA", "wav");

        assert_eq!(payload["model"], "mimo-v2.5");
        assert_eq!(payload["messages"][0]["content"], "prompt");
        assert_eq!(
            payload["messages"][1]["content"][1]["input_audio"]["data"],
            "AAA"
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
}
