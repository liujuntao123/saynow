use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::{AppConfig, ProviderConfig};

pub const KIND_OPENAI_COMPATIBLE: &str = "openai-compatible";
pub const KIND_DOUBAO_WS: &str = "doubao-ws";
pub const DEFAULT_DOUBAO_ENDPOINT: &str =
    "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";
pub const DEFAULT_DOUBAO_RESOURCE_ID: &str = "volc.seedasr.sauc.duration";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DoubaoAuthMode {
    /// 旧控制台：X-Api-App-Key + X-Api-Access-Key
    Legacy,
    /// 新控制台：仅 X-Api-Key
    ApiKey,
}

impl DoubaoAuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::ApiKey => "apiKey",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "apiKey" | "api_key" | "apikey" | "new" => Self::ApiKey,
            _ => Self::Legacy,
        }
    }
}

impl Default for DoubaoAuthMode {
    fn default() -> Self {
        Self::Legacy
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DoubaoExtra {
    #[serde(default)]
    pub auth_mode: String,
    /// 旧控制台 Access Token；新控制台可为空。
    #[serde(default)]
    pub access_key: String,
}

#[derive(Debug, Clone)]
pub struct DoubaoProviderSettings {
    pub endpoint: String,
    pub resource_id: String,
    pub auth_mode: DoubaoAuthMode,
    /// legacy: App Key；apiKey: X-Api-Key
    pub primary_key: String,
    /// legacy only
    pub access_key: String,
}

pub fn is_doubao_provider(kind: &str, provider: &str) -> bool {
    let kind = kind.trim().to_ascii_lowercase();
    if kind == KIND_DOUBAO_WS || kind == "doubao" {
        return true;
    }
    let provider = provider.to_ascii_lowercase();
    provider.contains("doubao") || provider.contains("豆包")
}

pub fn normalize_provider_kind(kind: &str, provider: &str) -> String {
    if is_doubao_provider(kind, provider) {
        KIND_DOUBAO_WS.to_string()
    } else if kind.trim().is_empty() {
        KIND_OPENAI_COMPATIBLE.to_string()
    } else {
        kind.trim().to_string()
    }
}

pub fn parse_extra_json(raw: &str) -> DoubaoExtra {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return DoubaoExtra::default();
    }
    serde_json::from_str::<DoubaoExtra>(trimmed).unwrap_or_default()
}

pub fn extra_json_string(extra: &DoubaoExtra) -> String {
    serde_json::to_string(extra).unwrap_or_else(|_| "{}".to_string())
}

pub fn settings_from_app_config(config: &AppConfig) -> Result<DoubaoProviderSettings, String> {
    settings_from_parts(
        &config.kind,
        &config.provider,
        &config.base_url,
        &config.model,
        &config.api_key_ref,
        &config.extra_json,
    )
}

pub fn settings_from_provider(provider: &ProviderConfig) -> Result<DoubaoProviderSettings, String> {
    settings_from_parts(
        &provider.kind,
        &provider.provider,
        &provider.base_url,
        &provider.model,
        &provider.api_key_ref,
        &provider.extra_json,
    )
}

fn settings_from_parts(
    kind: &str,
    provider: &str,
    base_url: &str,
    model: &str,
    api_key_ref: &str,
    extra_json: &str,
) -> Result<DoubaoProviderSettings, String> {
    if !is_doubao_provider(kind, provider) {
        return Err("当前供应商不是豆包 ASR。".to_string());
    }

    let extra = parse_extra_json(extra_json);
    let auth_mode = if extra.auth_mode.trim().is_empty() {
        if extra.access_key.trim().is_empty() {
            DoubaoAuthMode::ApiKey
        } else {
            DoubaoAuthMode::Legacy
        }
    } else {
        DoubaoAuthMode::parse(&extra.auth_mode)
    };

    let primary_key = resolve_secret(api_key_ref)?;
    let access_key = if auth_mode == DoubaoAuthMode::Legacy {
        let raw = extra.access_key.trim();
        if raw.is_empty() {
            return Err("豆包旧控制台需要填写 Access Key。".to_string());
        }
        resolve_secret(raw)?
    } else {
        String::new()
    };

    if primary_key.trim().is_empty() {
        return Err(match auth_mode {
            DoubaoAuthMode::Legacy => "豆包旧控制台需要填写 App Key。".to_string(),
            DoubaoAuthMode::ApiKey => "豆包新控制台需要填写 API Key。".to_string(),
        });
    }

    let endpoint = {
        let trimmed = base_url.trim();
        if trimmed.is_empty() {
            DEFAULT_DOUBAO_ENDPOINT.to_string()
        } else {
            trimmed.trim_end_matches('/').to_string()
        }
    };
    let resource_id = {
        let trimmed = model.trim();
        if trimmed.is_empty() {
            DEFAULT_DOUBAO_RESOURCE_ID.to_string()
        } else {
            trimmed.to_string()
        }
    };

    Ok(DoubaoProviderSettings {
        endpoint,
        resource_id,
        auth_mode,
        primary_key,
        access_key,
    })
}

fn resolve_secret(raw: &str) -> Result<String, String> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return Err("凭证为空。".to_string());
    }
    let value = if let Some(env_name) = normalized.strip_prefix("env:") {
        std::env::var(env_name).map_err(|_| format!("环境变量 {env_name} 未设置。"))?
    } else if let Some(key) = normalized.strip_prefix("literal:") {
        key.to_string()
    } else {
        normalized.to_string()
    };
    if value.trim().is_empty() {
        Err("凭证为空。".to_string())
    } else {
        Ok(value)
    }
}

pub fn doubao_complete(provider: &str, base_url: &str, model: &str, api_key_ref: &str, extra_json: &str) -> bool {
    if provider.trim().is_empty() || api_key_ref.trim().is_empty() {
        return false;
    }
    // endpoint / resource 可有默认值，但 UI 通常会填；允许空并在 settings 里回填默认。
    let _ = (base_url, model);
    let extra = parse_extra_json(extra_json);
    let auth_mode = if extra.auth_mode.trim().is_empty() {
        if extra.access_key.trim().is_empty() {
            DoubaoAuthMode::ApiKey
        } else {
            DoubaoAuthMode::Legacy
        }
    } else {
        DoubaoAuthMode::parse(&extra.auth_mode)
    };
    match auth_mode {
        DoubaoAuthMode::Legacy => !extra.access_key.trim().is_empty(),
        DoubaoAuthMode::ApiKey => true,
    }
}

pub fn build_context_json(hotwords: &[String]) -> String {
    let mut context = serde_json::json!({
        "context_type": "dialog_ctx",
        "context_data": [
            {"text": "当前场景为技术听写，中英文混合"},
            {"text": "保留英文专有名词和技术术语，如 Kubernetes, GPT-4o, Claude"},
            {"text": "保留语气词，去除尾部句号"},
        ]
    });

    // 双向流式 hotwords 约 100 tokens；保守截断到 40 个词。
    let limited: Vec<Value> = hotwords
        .iter()
        .map(|w| w.trim())
        .filter(|w| !w.is_empty())
        .take(40)
        .map(|w| serde_json::json!({ "word": w }))
        .collect();
    if !limited.is_empty() {
        context["hotwords"] = Value::Array(limited);
    }
    context.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_doubao_kind_and_name() {
        assert!(is_doubao_provider(KIND_DOUBAO_WS, "x"));
        assert!(is_doubao_provider("", "Doubao ASR"));
        assert!(is_doubao_provider("", "豆包语音"));
        assert!(!is_doubao_provider(KIND_OPENAI_COMPATIBLE, "Qwen"));
    }

    #[test]
    fn legacy_requires_access_key() {
        assert!(!doubao_complete(
            "Doubao",
            DEFAULT_DOUBAO_ENDPOINT,
            DEFAULT_DOUBAO_RESOURCE_ID,
            "app-id",
            r#"{"authMode":"legacy"}"#
        ));
        assert!(doubao_complete(
            "Doubao",
            DEFAULT_DOUBAO_ENDPOINT,
            DEFAULT_DOUBAO_RESOURCE_ID,
            "app-id",
            r#"{"authMode":"legacy","accessKey":"token"}"#
        ));
    }

    #[test]
    fn api_key_mode_only_needs_primary() {
        assert!(doubao_complete(
            "Doubao",
            "",
            "",
            "my-api-key",
            r#"{"authMode":"apiKey"}"#
        ));
    }

    #[test]
    fn builds_hotword_context_string() {
        let raw = build_context_json(&["说文".to_string(), "saynow".to_string()]);
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["context_type"], "dialog_ctx");
        assert_eq!(parsed["hotwords"].as_array().unwrap().len(), 2);
    }
}
