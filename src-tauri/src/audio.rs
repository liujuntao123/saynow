use std::{
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use base64::{engine::general_purpose, Engine};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream,
};
use serde::Serialize;

#[derive(Default)]
pub struct NativeAudioRecorder {
    input: Mutex<Option<WarmInputStream>>,
}

struct WarmInputStream {
    _stream: Stream,
    samples: Arc<Mutex<Vec<i16>>>,
    recording: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
    started_at: Option<Instant>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRecordedAudio {
    pub audio_base64: String,
    pub duration_seconds: u32,
    pub mime_type: String,
}

impl NativeAudioRecorder {
    pub fn warm_up(&self) -> Result<(), String> {
        let mut input = self
            .input
            .lock()
            .map_err(|_| "无法锁定原生录音状态。".to_string())?;
        if input.is_none() {
            *input = Some(create_warm_input_stream()?);
        }
        Ok(())
    }

    pub fn start(&self) -> Result<(), String> {
        let mut input = self
            .input
            .lock()
            .map_err(|_| "无法锁定原生录音状态。".to_string())?;
        if input.is_none() {
            *input = Some(create_warm_input_stream()?);
        }
        let input = input.as_mut().expect("input stream just initialized");
        if input.recording.load(Ordering::SeqCst) {
            return Ok(());
        }
        input
            .samples
            .lock()
            .map_err(|_| "无法清空原生录音采样。".to_string())?
            .clear();
        input.started_at = Some(Instant::now());
        input.recording.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn stop(&self) -> Result<Option<NativeRecordedAudio>, String> {
        let mut input = self
            .input
            .lock()
            .map_err(|_| "无法锁定原生录音状态。".to_string())?;
        let Some(input) = input.as_mut() else {
            return Ok(None);
        };
        if !input.recording.swap(false, Ordering::SeqCst) {
            return Ok(None);
        }

        let duration_seconds = input
            .started_at
            .take()
            .map(|started_at| started_at.elapsed().as_secs().max(1) as u32)
            .unwrap_or(1);
        let samples = {
            let mut buffer = input
                .samples
                .lock()
                .map_err(|_| "无法读取原生录音采样。".to_string())?;
            let samples = buffer.clone();
            buffer.clear();
            samples
        };
        if samples.is_empty() {
            return Ok(None);
        }

        let wav = encode_wav_pcm16(&samples, input.sample_rate, input.channels)
            .map_err(|error| format!("无法编码 WAV 音频：{error}"))?;
        Ok(Some(NativeRecordedAudio {
            audio_base64: general_purpose::STANDARD.encode(wav),
            duration_seconds,
            mime_type: "audio/wav".to_string(),
        }))
    }

    pub fn cancel(&self) -> Result<(), String> {
        let mut input = self
            .input
            .lock()
            .map_err(|_| "无法锁定原生录音状态。".to_string())?;
        let Some(input) = input.as_mut() else {
            return Ok(());
        };
        input.recording.store(false, Ordering::SeqCst);
        input.started_at = None;
        input
            .samples
            .lock()
            .map_err(|_| "无法清空原生录音采样。".to_string())?
            .clear();
        Ok(())
    }
}

fn create_warm_input_stream() -> Result<WarmInputStream, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "未找到可用麦克风输入设备。".to_string())?;
    let supported_config = device
        .default_input_config()
        .map_err(|error| format!("无法读取麦克风默认配置：{error}"))?;
    let sample_format = supported_config.sample_format();
    let config = supported_config.config();
    let sample_rate = config.sample_rate.0;
    let channels = config.channels;
    let samples = Arc::new(Mutex::new(Vec::<i16>::with_capacity(
        sample_rate as usize * channels as usize * 10,
    )));
    let recording = Arc::new(AtomicBool::new(false));

    let stream = build_input_stream(
        &device,
        &config,
        sample_format,
        Arc::clone(&samples),
        Arc::clone(&recording),
    )?;
    stream
        .play()
        .map_err(|error| format!("无法启动原生录音流：{error}"))?;

    Ok(WarmInputStream {
        _stream: stream,
        samples,
        recording,
        sample_rate,
        channels,
        started_at: None,
    })
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    samples: Arc<Mutex<Vec<i16>>>,
    recording: Arc<AtomicBool>,
) -> Result<Stream, String> {
    let err_fn = |error| eprintln!("[saynow] native audio input stream error: {error}");
    match sample_format {
        SampleFormat::I8 => build_stream(device, config, samples, recording, err_fn, convert_i8),
        SampleFormat::I16 => build_stream(device, config, samples, recording, err_fn, convert_i16),
        SampleFormat::I32 => build_stream(device, config, samples, recording, err_fn, convert_i32),
        SampleFormat::I64 => build_stream(device, config, samples, recording, err_fn, convert_i64),
        SampleFormat::U8 => build_stream(device, config, samples, recording, err_fn, convert_u8),
        SampleFormat::U16 => build_stream(device, config, samples, recording, err_fn, convert_u16),
        SampleFormat::U32 => build_stream(device, config, samples, recording, err_fn, convert_u32),
        SampleFormat::U64 => build_stream(device, config, samples, recording, err_fn, convert_u64),
        SampleFormat::F32 => build_stream(device, config, samples, recording, err_fn, convert_f32),
        SampleFormat::F64 => build_stream(device, config, samples, recording, err_fn, convert_f64),
        _ => Err(format!("不支持的麦克风采样格式：{sample_format:?}")),
    }
}

fn build_stream<T, F>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<i16>>>,
    recording: Arc<AtomicBool>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
    convert: F,
) -> Result<Stream, String>
where
    T: cpal::SizedSample,
    F: Fn(T) -> i16 + Send + Sync + Copy + 'static,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if recording.load(Ordering::SeqCst) {
                    if let Ok(mut buffer) = samples.lock() {
                        buffer.extend(data.iter().copied().map(convert));
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|error| format!("无法创建原生录音流：{error}"))
}

fn convert_i8(sample: i8) -> i16 {
    (sample as i16) << 8
}

fn convert_i16(sample: i16) -> i16 {
    sample
}

fn convert_i32(sample: i32) -> i16 {
    (sample >> 16) as i16
}

fn convert_i64(sample: i64) -> i16 {
    (sample >> 48) as i16
}

fn convert_u8(sample: u8) -> i16 {
    ((sample as i16) - 128) << 8
}

fn convert_u16(sample: u16) -> i16 {
    (sample as i32 - 32_768) as i16
}

fn convert_u32(sample: u32) -> i16 {
    ((sample as i64 - 2_147_483_648) >> 16) as i16
}

fn convert_u64(sample: u64) -> i16 {
    ((sample as i128 - 9_223_372_036_854_775_808) >> 48) as i16
}

fn convert_f32(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn convert_f64(sample: f64) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f64) as i16
}

fn encode_wav_pcm16(samples: &[i16], sample_rate: u32, channels: u16) -> std::io::Result<Vec<u8>> {
    let data_size = samples.len() as u32 * 2;
    let byte_rate = sample_rate * channels as u32 * 2;
    let block_align = channels * 2;
    let mut wav = Vec::with_capacity(44 + data_size as usize);

    wav.write_all(b"RIFF")?;
    wav.write_all(&(36 + data_size).to_le_bytes())?;
    wav.write_all(b"WAVE")?;
    wav.write_all(b"fmt ")?;
    wav.write_all(&16u32.to_le_bytes())?;
    wav.write_all(&1u16.to_le_bytes())?;
    wav.write_all(&channels.to_le_bytes())?;
    wav.write_all(&sample_rate.to_le_bytes())?;
    wav.write_all(&byte_rate.to_le_bytes())?;
    wav.write_all(&block_align.to_le_bytes())?;
    wav.write_all(&16u16.to_le_bytes())?;
    wav.write_all(b"data")?;
    wav.write_all(&data_size.to_le_bytes())?;
    for sample in samples {
        wav.write_all(&sample.to_le_bytes())?;
    }

    Ok(wav)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_pcm16_wav_header() {
        let wav = encode_wav_pcm16(&[0, i16::MAX, i16::MIN], 16_000, 1).unwrap();

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 6);
        assert_eq!(wav.len(), 50);
    }

    #[test]
    fn converts_unsigned_samples_to_signed_pcm16() {
        assert_eq!(convert_u8(0), i16::MIN);
        assert_eq!(convert_u8(128), 0);
        assert_eq!(convert_u16(32_768), 0);
        assert_eq!(convert_u32(2_147_483_648), 0);
    }

    #[test]
    #[ignore = "requires a default microphone input device"]
    fn records_from_default_input_device() {
        let recorder = NativeAudioRecorder::default();

        recorder.start().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let audio = recorder.stop().unwrap().expect("recorded audio");

        assert_eq!(audio.mime_type, "audio/wav");
        assert!(audio.duration_seconds >= 1);
        assert!(audio.audio_base64.len() > 64);
    }
}
