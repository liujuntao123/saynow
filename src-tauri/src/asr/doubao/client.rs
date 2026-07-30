use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::time::timeout;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};
use uuid::Uuid;

use super::config::{build_context_json, DoubaoAuthMode, DoubaoProviderSettings};
use super::protocol::{
    build_audio_only_request, build_full_client_request, build_last_audio_request, parse_response,
};

/// 200ms @ 16kHz mono s16le = 16000 * 0.2 * 2 = 6400 bytes.
/// 文档建议双向流式 200ms 最优；这里用 200ms。
const AUDIO_CHUNK_BYTES: usize = 6_400;
const RECOGNITION_TIMEOUT: Duration = Duration::from_secs(90);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn recognize_pcm_stream<F>(
    settings: &DoubaoProviderSettings,
    pcm: &[u8],
    hotwords: &[String],
    mut on_transcript: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    let (mut write, mut read) = connect_ws(settings).await?;

    let full_request = build_full_client_payload(settings, hotwords);
    let payload_bytes = serde_json::to_vec(&full_request)
        .map_err(|error| format!("构造豆包请求失败：{error}"))?;
    let frame = build_full_client_request(&payload_bytes, 1)?;
    write
        .send(Message::Binary(frame.into()))
        .await
        .map_err(|error| format!("发送豆包 full client request 失败：{error}"))?;

    // 读取初始响应（通常无文本）。
    if let Ok(Some(msg)) = timeout(READ_TIMEOUT, read.next()).await {
        match msg {
            Ok(Message::Binary(data)) => match parse_response(&data) {
                Ok(parsed) if !parsed.text.is_empty() => {
                    on_transcript(parsed.text.clone());
                }
                Ok(_) => {}
                Err(error) => {
                    // 初始帧偶发无文本/非结果，仅记录；真正错误（含 code=）直接失败。
                    if error.contains("code=") {
                        return Err(error);
                    }
                    eprintln!("[saynow] doubao initial response: {error}");
                }
            },
            Ok(Message::Close(frame)) => {
                return Err(format!("豆包连接在初始化后关闭：{frame:?}"));
            }
            Ok(_) => {}
            Err(error) => return Err(format!("读取豆包初始响应失败：{error}")),
        }
    }

    let mut sequence = 1i32;
    for chunk in pcm.chunks(AUDIO_CHUNK_BYTES) {
        sequence += 1;
        let frame = build_audio_only_request(chunk, sequence)?;
        write
            .send(Message::Binary(frame.into()))
            .await
            .map_err(|error| format!("发送豆包音频失败：{error}"))?;
    }

    sequence += 1;
    let last_seq = -sequence;
    let finish = build_last_audio_request(last_seq)?;
    write
        .send(Message::Binary(finish.into()))
        .await
        .map_err(|error| format!("发送豆包结束包失败：{error}"))?;

    let mut accumulated = String::new();
    let deadline = tokio::time::Instant::now() + RECOGNITION_TIMEOUT;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        let msg = match timeout(remaining.min(READ_TIMEOUT), read.next()).await {
            Ok(Some(msg)) => msg,
            Ok(None) => break,
            Err(_) => {
                if accumulated.is_empty() {
                    return Err("豆包识别超时。".to_string());
                }
                break;
            }
        };

        match msg {
            Ok(Message::Binary(data)) => match parse_response(&data) {
                Ok(parsed) => {
                    if !parsed.text.is_empty() {
                        accumulated = parsed.text;
                        on_transcript(accumulated.clone());
                    }
                    if parsed.is_last {
                        break;
                    }
                }
                Err(error) => {
                    if error.contains("code=") {
                        return Err(error);
                    }
                    eprintln!("[saynow] doubao response parse: {error}");
                }
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(payload)) => {
                let _ = write.send(Message::Pong(payload)).await;
            }
            Ok(_) => {}
            Err(error) => return Err(format!("豆包 WebSocket 错误：{error}")),
        }
    }

    let _ = write.close().await;
    if accumulated.trim().is_empty() {
        Err("豆包识别响应中没有可用文本。".to_string())
    } else {
        Ok(accumulated)
    }
}

pub async fn test_connection(settings: &DoubaoProviderSettings) -> Result<String, String> {
    let started = std::time::Instant::now();
    let (mut write, mut read) = connect_ws(settings).await?;

    let full_request = build_full_client_payload(settings, &[]);
    let payload_bytes = serde_json::to_vec(&full_request)
        .map_err(|error| format!("构造豆包请求失败：{error}"))?;
    let frame = build_full_client_request(&payload_bytes, 1)?;
    write
        .send(Message::Binary(frame.into()))
        .await
        .map_err(|error| format!("发送豆包测试请求失败：{error}"))?;

    // 发结束包，避免会话悬挂。
    let finish = build_last_audio_request(-2)?;
    let _ = write.send(Message::Binary(finish.into())).await;

    let mut saw_ok = false;
    let mut last_error = None;
    for _ in 0..3 {
        match timeout(Duration::from_secs(8), read.next()).await {
            Ok(Some(Ok(Message::Binary(data)))) => match parse_response(&data) {
                Ok(_) => {
                    saw_ok = true;
                    break;
                }
                Err(error) => {
                    if error.contains("code=") {
                        last_error = Some(error);
                        break;
                    }
                    saw_ok = true;
                    break;
                }
            },
            Ok(Some(Ok(Message::Close(frame)))) => {
                last_error = Some(format!("连接关闭：{frame:?}"));
                break;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(error))) => {
                last_error = Some(error.to_string());
                break;
            }
            Ok(None) => break,
            Err(_) => {
                last_error = Some("等待响应超时".to_string());
                break;
            }
        }
    }
    let _ = write.close().await;
    let elapsed_ms = started.elapsed().as_millis();
    if saw_ok {
        Ok(format!("豆包连接成功（{elapsed_ms}ms）"))
    } else {
        Err(format!(
            "豆包连接失败：{}",
            last_error.unwrap_or_else(|| "无有效响应".to_string())
        ))
    }
}

async fn connect_ws(
    settings: &DoubaoProviderSettings,
) -> Result<
    (
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
        futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    ),
    String,
> {
    let connect_id = Uuid::new_v4().to_string();
    let mut request = settings
        .endpoint
        .as_str()
        .into_client_request()
        .map_err(|error| format!("无效的豆包 WebSocket 地址：{error}"))?;

    let headers = request.headers_mut();
    headers.insert(
        "X-Api-Resource-Id",
        header_value(&settings.resource_id)?,
    );
    headers.insert("X-Api-Connect-Id", header_value(&connect_id)?);

    match settings.auth_mode {
        DoubaoAuthMode::Legacy => {
            headers.insert("X-Api-App-Key", header_value(&settings.primary_key)?);
            headers.insert("X-Api-Access-Key", header_value(&settings.access_key)?);
        }
        DoubaoAuthMode::ApiKey => {
            headers.insert("X-Api-Key", header_value(&settings.primary_key)?);
        }
    }

    // 部分环境需要显式 Host；IntoClientRequest 通常已带。
    let host = settings
        .endpoint
        .trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .split('/')
        .next()
        .unwrap_or("openspeech.bytedance.com");
    if !headers.contains_key("Host") {
        headers.insert("Host", header_value(host)?);
    }

    let (ws_stream, response) = timeout(CONNECT_TIMEOUT, connect_async(request))
        .await
        .map_err(|_| "连接豆包 WebSocket 超时。".to_string())?
        .map_err(|error| format!("连接豆包 WebSocket 失败：{error}"))?;

    if let Some(log_id) = response.headers().get("X-Tt-Logid") {
        eprintln!(
            "[saynow] doubao connected; logid={} connect_id={}",
            log_id.to_str().unwrap_or("?"),
            connect_id
        );
    } else {
        eprintln!("[saynow] doubao connected; connect_id={connect_id}");
    }

    Ok(ws_stream.split())
}

fn header_value(value: &str) -> Result<HeaderValue, String> {
    HeaderValue::from_str(value).map_err(|error| format!("无效请求头值：{error}"))
}

fn build_full_client_payload(
    settings: &DoubaoProviderSettings,
    hotwords: &[String],
) -> serde_json::Value {
    let uid = match settings.auth_mode {
        DoubaoAuthMode::Legacy => settings.primary_key.clone(),
        DoubaoAuthMode::ApiKey => "saynow".to_string(),
    };
    let context = build_context_json(hotwords);
    json!({
        "user": { "uid": uid },
        "audio": {
            "format": "pcm",
            "rate": 16000,
            "bits": 16,
            "channel": 1
        },
        "request": {
            "model_name": "bigmodel",
            "enable_nonstream": true,
            "enable_itn": true,
            "enable_punc": true,
            "enable_ddc": true,
            "show_utterances": true,
            "result_type": "full",
            "corpus": {
                "context": context
            }
        }
    })
}
