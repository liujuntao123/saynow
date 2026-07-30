use base64::{engine::general_purpose, Engine};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// 将识别输入（通常为 base64 WAV）转为 16kHz mono PCM s16le 字节流。
pub fn pcm16le_from_recognition_input(audio_base64: &str, mime_type: &str) -> Result<Vec<u8>, String> {
    let bytes = decode_audio_bytes(audio_base64)?;
    if looks_like_wav(&bytes) || mime_type.to_ascii_lowercase().contains("wav") {
        let (samples, sample_rate, channels) = decode_wav_pcm16(&bytes)?;
        let mono = to_mono(&samples, channels);
        let resampled = resample_linear(&mono, sample_rate, TARGET_SAMPLE_RATE);
        return Ok(i16_slice_to_le_bytes(&resampled));
    }

    // 非 WAV 时按 raw PCM s16le 处理，假定已是目标格式（兜底）。
    if bytes.len() % 2 != 0 {
        return Err("原始 PCM 长度无效。".to_string());
    }
    Ok(bytes)
}

fn decode_audio_bytes(audio_base64: &str) -> Result<Vec<u8>, String> {
    let trimmed = audio_base64.trim();
    let payload = if let Some(idx) = trimmed.find("base64,") {
        &trimmed[idx + "base64,".len()..]
    } else {
        trimmed
    };
    general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|error| format!("无法解码音频 base64：{error}"))
}

fn looks_like_wav(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
}

fn decode_wav_pcm16(bytes: &[u8]) -> Result<(Vec<i16>, u32, u16), String> {
    if bytes.len() < 44 {
        return Err("WAV 数据过短。".to_string());
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("不是有效的 WAV 文件。".to_string());
    }

    let mut offset = 12usize;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut audio_format = 0u16;
    let mut data: Option<&[u8]> = None;

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size =
            u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let chunk_data_start = offset + 8;
        let chunk_data_end = chunk_data_start
            .checked_add(chunk_size)
            .ok_or_else(|| "WAV chunk 大小溢出。".to_string())?;
        if chunk_data_end > bytes.len() {
            return Err("WAV chunk 越界。".to_string());
        }
        let chunk_data = &bytes[chunk_data_start..chunk_data_end];

        if chunk_id == b"fmt " {
            if chunk_data.len() < 16 {
                return Err("WAV fmt chunk 过短。".to_string());
            }
            audio_format = u16::from_le_bytes(chunk_data[0..2].try_into().unwrap());
            channels = u16::from_le_bytes(chunk_data[2..4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(chunk_data[4..8].try_into().unwrap());
            bits_per_sample = u16::from_le_bytes(chunk_data[14..16].try_into().unwrap());
        } else if chunk_id == b"data" {
            data = Some(chunk_data);
        }

        // chunk 大小为奇数时需要 word 对齐填充。
        offset = chunk_data_end + (chunk_size % 2);
    }

    let data = data.ok_or_else(|| "WAV 缺少 data chunk。".to_string())?;
    if audio_format != 1 {
        return Err(format!("仅支持 PCM WAV，当前 format={audio_format}"));
    }
    if bits_per_sample != 16 {
        return Err(format!("仅支持 16-bit PCM，当前 bits={bits_per_sample}"));
    }
    if channels == 0 || sample_rate == 0 {
        return Err("WAV 采样参数无效。".to_string());
    }
    if data.len() % 2 != 0 {
        return Err("WAV PCM 数据长度无效。".to_string());
    }

    let mut samples = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok((samples, sample_rate, channels))
}

fn to_mono(samples: &[i16], channels: u16) -> Vec<i16> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    samples
        .chunks(ch)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|s| i32::from(*s)).sum();
            (sum / ch as i32) as i16
        })
        .collect()
}

fn resample_linear(samples: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if samples.is_empty() || from_rate == 0 {
        return Vec::new();
    }
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let out_len = ((samples.len() as u64) * u64::from(to_rate) / u64::from(from_rate)) as usize;
    if out_len == 0 {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(out_len);
    let last_index = samples.len() - 1;
    for i in 0..out_len {
        let src_pos = i as f64 * f64::from(from_rate) / f64::from(to_rate);
        let left = src_pos.floor() as usize;
        let right = (left + 1).min(last_index);
        let frac = src_pos - left as f64;
        let left_s = samples[left] as f64;
        let right_s = samples[right] as f64;
        let value = left_s + (right_s - left_s) * frac;
        output.push(value.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    output
}

fn i16_slice_to_le_bytes(samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose, Engine};

    fn encode_wav_pcm16(samples: &[i16], sample_rate: u32, channels: u16) -> Vec<u8> {
        let data_size = samples.len() as u32 * 2;
        let byte_rate = sample_rate * u32::from(channels) * 2;
        let block_align = channels * 2;
        let mut wav = Vec::with_capacity(44 + data_size as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_size).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    #[test]
    fn decodes_wav_and_resamples_to_16k_mono() {
        // 48k stereo, 0.1s → 4800 frames * 2 = 9600 samples
        let frames = 4_800usize;
        let mut samples = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let v = ((i % 100) as i16) * 10;
            samples.push(v);
            samples.push(v);
        }
        let wav = encode_wav_pcm16(&samples, 48_000, 2);
        let b64 = general_purpose::STANDARD.encode(&wav);
        let pcm = pcm16le_from_recognition_input(&b64, "audio/wav").unwrap();
        // 0.1s @ 16k mono = 1600 samples * 2 bytes
        assert_eq!(pcm.len(), 1_600 * 2);
    }

    #[test]
    fn keeps_16k_mono_length() {
        let samples = vec![0i16, 100, -100, 50];
        let wav = encode_wav_pcm16(&samples, 16_000, 1);
        let b64 = general_purpose::STANDARD.encode(&wav);
        let pcm = pcm16le_from_recognition_input(&b64, "audio/wav").unwrap();
        assert_eq!(pcm.len(), samples.len() * 2);
    }
}
