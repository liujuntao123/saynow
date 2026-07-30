use std::io::{Read, Write};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::Value;

pub const MSG_FULL_CLIENT_REQUEST: u8 = 0x1;
pub const MSG_AUDIO_ONLY_REQUEST: u8 = 0x2;
pub const MSG_FULL_SERVER_RESPONSE: u8 = 0x9;
pub const MSG_ERROR_RESPONSE: u8 = 0xf;

pub const FLAG_POSITIVE_SEQUENCE: u8 = 0x1;
pub const FLAG_NEGATIVE_SEQUENCE: u8 = 0x3;

pub const SERIALIZATION_NONE: u8 = 0x0;
pub const SERIALIZATION_JSON: u8 = 0x1;
pub const COMPRESSION_NONE: u8 = 0x0;
pub const COMPRESSION_GZIP: u8 = 0x1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedServerResponse {
    pub text: String,
    pub is_last: bool,
    pub sequence: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerError {
    pub code: u32,
    pub message: String,
}

/// 构造豆包二进制帧。
/// - full client request: msg_type=0x1, JSON payload, 通常 gzip
/// - audio only: msg_type=0x2, raw PCM, 通常无压缩
pub fn build_message(
    msg_type: u8,
    flags: u8,
    sequence: i32,
    payload: &[u8],
    compression_type: u8,
) -> Result<Vec<u8>, String> {
    let final_payload = if compression_type == COMPRESSION_GZIP {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(payload)
            .map_err(|error| format!("Gzip 压缩失败：{error}"))?;
        encoder
            .finish()
            .map_err(|error| format!("Gzip 收尾失败：{error}"))?
    } else {
        payload.to_vec()
    };

    let serialization = if msg_type == MSG_FULL_CLIENT_REQUEST {
        SERIALIZATION_JSON
    } else {
        SERIALIZATION_NONE
    };

    let mut msg = vec![
        0x11, // protocol v1, header size = 4 bytes
        (msg_type << 4) | (flags & 0x0f),
        (serialization << 4) | (compression_type & 0x0f),
        0x00,
    ];
    msg.extend_from_slice(&sequence.to_be_bytes());
    msg.extend_from_slice(&(final_payload.len() as u32).to_be_bytes());
    msg.extend_from_slice(&final_payload);
    Ok(msg)
}

pub fn build_full_client_request(payload_json: &[u8], sequence: i32) -> Result<Vec<u8>, String> {
    build_message(
        MSG_FULL_CLIENT_REQUEST,
        FLAG_POSITIVE_SEQUENCE,
        sequence,
        payload_json,
        COMPRESSION_GZIP,
    )
}

pub fn build_audio_only_request(pcm: &[u8], sequence: i32) -> Result<Vec<u8>, String> {
    build_message(
        MSG_AUDIO_ONLY_REQUEST,
        FLAG_POSITIVE_SEQUENCE,
        sequence,
        pcm,
        COMPRESSION_NONE,
    )
}

pub fn build_last_audio_request(sequence: i32) -> Result<Vec<u8>, String> {
    // 结束包：负序号 + flags 0x3，空 payload，无压缩。
    build_message(
        MSG_AUDIO_ONLY_REQUEST,
        FLAG_NEGATIVE_SEQUENCE,
        sequence,
        &[],
        COMPRESSION_NONE,
    )
}

pub fn parse_response(data: &[u8]) -> Result<ParsedServerResponse, String> {
    if data.len() < 4 {
        return Err(format!("响应太短：{} bytes", data.len()));
    }

    let header_size = (data[0] & 0x0f) as usize * 4;
    if header_size < 4 || data.len() < header_size {
        return Err("响应 header 不完整。".to_string());
    }

    let message_type = data[1] >> 4;
    let message_flags = data[1] & 0x0f;
    let compression = data[2] & 0x0f;
    let mut offset = header_size;

    if message_type == MSG_ERROR_RESPONSE {
        let error = parse_error_payload(data, offset)?;
        return Err(format!(
            "豆包服务端错误 code={}：{}",
            error.code, error.message
        ));
    }

    if message_type != MSG_FULL_SERVER_RESPONSE && message_type != 0x0 {
        // 未知类型尽量继续解析 payload；若失败再报错。
    }

    let mut sequence = None;
    if message_flags & 0x01 != 0 {
        if data.len() < offset + 4 {
            return Err("响应缺少 sequence。".to_string());
        }
        sequence = Some(i32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
    }

    if data.len() < offset + 4 {
        return Err("响应缺少 payload size。".to_string());
    }
    let payload_size = u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;

    if data.len() < offset + payload_size {
        return Err(format!(
            "响应 payload 不完整：需要 {} bytes，实际 {} bytes",
            offset + payload_size,
            data.len()
        ));
    }

    let payload_data = &data[offset..offset + payload_size];
    let json_str = if payload_size == 0 {
        String::new()
    } else if compression == COMPRESSION_GZIP {
        let mut decoder = GzDecoder::new(payload_data);
        let mut s = String::new();
        decoder
            .read_to_string(&mut s)
            .map_err(|error| format!("解压响应失败：{error}"))?;
        s
    } else {
        String::from_utf8(payload_data.to_vec())
            .map_err(|error| format!("响应不是有效 UTF-8：{error}"))?
    };

    let is_last = message_flags & 0x02 != 0;
    if json_str.trim().is_empty() {
        return Ok(ParsedServerResponse {
            text: String::new(),
            is_last,
            sequence,
        });
    }

    let result: Value = serde_json::from_str(&json_str)
        .map_err(|error| format!("响应 JSON 无效：{error}; body={json_str}"))?;

    let text = extract_result_text(&result);
    Ok(ParsedServerResponse {
        text,
        is_last,
        sequence,
    })
}

fn parse_error_payload(data: &[u8], mut offset: usize) -> Result<ServerError, String> {
    if data.len() < offset + 8 {
        return Ok(ServerError {
            code: 0,
            message: "未知错误".to_string(),
        });
    }
    let code = u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]);
    offset += 4;
    let message_size = u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]) as usize;
    offset += 4;
    let message = if data.len() >= offset + message_size {
        String::from_utf8_lossy(&data[offset..offset + message_size]).to_string()
    } else {
        String::from_utf8_lossy(&data[offset..]).to_string()
    };
    Ok(ServerError { code, message })
}

fn extract_result_text(result: &Value) -> String {
    if let Some(text) = result
        .pointer("/result/text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return text.to_string();
    }

    // 部分响应把 result 做成数组。
    if let Some(text) = result
        .pointer("/result/0/text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return text.to_string();
    }

    // enable_nonstream 时优先拼接 definite 分句。
    if let Some(utterances) = result
        .pointer("/result/utterances")
        .and_then(Value::as_array)
    {
        let definite: Vec<&str> = utterances
            .iter()
            .filter(|item| item.get("definite").and_then(Value::as_bool).unwrap_or(false))
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect();
        if !definite.is_empty() {
            return definite.join("");
        }
        let all: Vec<&str> = utterances
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .collect();
        if !all.is_empty() {
            return all.join("");
        }
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_full_client_request_frame_layout() {
        let payload = br#"{"user":{"uid":"1"}}"#;
        let frame = build_full_client_request(payload, 1).unwrap();
        assert_eq!(frame[0], 0x11);
        assert_eq!(frame[1] >> 4, MSG_FULL_CLIENT_REQUEST);
        assert_eq!(frame[1] & 0x0f, FLAG_POSITIVE_SEQUENCE);
        assert_eq!(frame[2] >> 4, SERIALIZATION_JSON);
        assert_eq!(frame[2] & 0x0f, COMPRESSION_GZIP);
        let seq = i32::from_be_bytes(frame[4..8].try_into().unwrap());
        assert_eq!(seq, 1);
        let size = u32::from_be_bytes(frame[8..12].try_into().unwrap()) as usize;
        assert_eq!(frame.len(), 12 + size);
    }

    #[test]
    fn builds_last_audio_packet_with_negative_sequence() {
        let frame = build_last_audio_request(-3).unwrap();
        assert_eq!(frame[1] >> 4, MSG_AUDIO_ONLY_REQUEST);
        assert_eq!(frame[1] & 0x0f, FLAG_NEGATIVE_SEQUENCE);
        let seq = i32::from_be_bytes(frame[4..8].try_into().unwrap());
        assert_eq!(seq, -3);
        let size = u32::from_be_bytes(frame[8..12].try_into().unwrap());
        assert_eq!(size, 0);
        assert_eq!(frame.len(), 12);
    }

    #[test]
    fn roundtrips_gzip_server_response() {
        let body = "{\"result\":{\"text\":\"hello\"}}".as_bytes();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(body).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut frame = vec![
            0x11,
            (MSG_FULL_SERVER_RESPONSE << 4) | FLAG_POSITIVE_SEQUENCE,
            (SERIALIZATION_JSON << 4) | COMPRESSION_GZIP,
            0x00,
        ];
        frame.extend_from_slice(&1i32.to_be_bytes());
        frame.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
        frame.extend_from_slice(&compressed);

        let parsed = parse_response(&frame).unwrap();
        assert_eq!(parsed.text, "hello");
        assert!(!parsed.is_last);
        assert_eq!(parsed.sequence, Some(1));
    }

    #[test]
    fn parses_error_frame() {
        let message = b"invalid auth";
        let mut frame = vec![0x11, MSG_ERROR_RESPONSE << 4, 0x00, 0x00];
        frame.extend_from_slice(&45000001u32.to_be_bytes());
        frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
        frame.extend_from_slice(message);
        let err = parse_response(&frame).unwrap_err();
        assert!(err.contains("45000001"));
        assert!(err.contains("invalid auth"));
    }

    #[test]
    fn marks_last_packet_from_flags() {
        let body = b"{\"result\":{\"text\":\"done\"}}";
        let mut frame = vec![
            0x11,
            (MSG_FULL_SERVER_RESPONSE << 4) | FLAG_NEGATIVE_SEQUENCE,
            (SERIALIZATION_JSON << 4) | COMPRESSION_NONE,
            0x00,
        ];
        frame.extend_from_slice(&(-2i32).to_be_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
        frame.extend_from_slice(body);
        let parsed = parse_response(&frame).unwrap();
        assert!(parsed.is_last);
        assert_eq!(parsed.text, "done");
    }
}
