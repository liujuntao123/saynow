export interface RecordedAudio {
  audioBase64: string;
  durationSeconds: number;
  mimeType: string;
}

const supportedMimeTypes = ['audio/webm;codecs=opus', 'audio/webm', 'audio/mp4', 'audio/wav'];

export function createAudioRecorder() {
  let mediaRecorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let chunks: Blob[] = [];
  let startedAt = 0;
  let stopPromise: Promise<RecordedAudio> | null = null;

  return {
    get active() {
      return Boolean(mediaRecorder && mediaRecorder.state !== 'inactive');
    },
    async start() {
      if (mediaRecorder && mediaRecorder.state !== 'inactive') return;
      chunks = [];
      stopPromise = null;
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mimeType = supportedMimeTypes.find((type) => MediaRecorder.isTypeSupported(type)) ?? '';
      mediaRecorder = new MediaRecorder(stream, mimeType ? { mimeType } : undefined);
      mediaRecorder.addEventListener('dataavailable', (event) => {
        if (event.data.size > 0) chunks.push(event.data);
      });
      await new Promise<void>((resolve, reject) => {
        const recorder = mediaRecorder;
        if (!recorder) {
          reject(new Error('录音器初始化失败。'));
          return;
        }
        recorder.addEventListener(
          'start',
          () => {
            startedAt = Date.now();
            resolve();
          },
          { once: true },
        );
        recorder.addEventListener(
          'error',
          (event) => {
            cleanup();
            reject(event instanceof ErrorEvent ? event.error : new Error('录音启动失败。'));
          },
          { once: true },
        );
        try {
          recorder.start();
        } catch (error) {
          cleanup();
          reject(error);
        }
      });
    },
    async stop() {
      if (!mediaRecorder || mediaRecorder.state === 'inactive') {
        cleanup();
        return null;
      }

      if (!stopPromise) {
        const recorder = mediaRecorder;
        stopPromise = new Promise<RecordedAudio>((resolve, reject) => {
          recorder.addEventListener(
            'stop',
            async () => {
              try {
                const blob = new Blob(chunks, { type: recorder.mimeType || 'audio/webm' });
                const durationSeconds = Math.max(1, Math.round((Date.now() - startedAt) / 1000));
                const wavBlob = await convertBlobToWav(blob);
                const audioBase64 = await blobToBase64(wavBlob);
                resolve({ audioBase64, durationSeconds, mimeType: wavBlob.type });
              } catch (error) {
                reject(error);
              } finally {
                cleanup();
              }
            },
            { once: true },
          );
          try {
            recorder.stop();
          } catch (error) {
            cleanup();
            reject(error);
          }
        });
      }

      return stopPromise;
    },
    cancel() {
      cleanup();
    },
  };

  function cleanup() {
    stopMediaStream(stream);
    stream = null;
    mediaRecorder = null;
    chunks = [];
    startedAt = 0;
    stopPromise = null;
  }
}

function stopMediaStream(mediaStream: MediaStream | null) {
  mediaStream?.getTracks().forEach((track) => track.stop());
}

async function blobToBase64(blob: Blob): Promise<string> {
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener('loadend', () => resolve(String(reader.result)));
    reader.addEventListener('error', () => reject(reader.error));
    reader.readAsDataURL(blob);
  });
  return dataUrl.split(',', 2)[1] ?? '';
}

async function convertBlobToWav(blob: Blob): Promise<Blob> {
  if (blob.type === 'audio/wav') return blob;

  const audioContext = new AudioContext();
  try {
    const arrayBuffer = await blob.arrayBuffer();
    const audioBuffer = await audioContext.decodeAudioData(arrayBuffer.slice(0));
    const wavBuffer = encodeWav(audioBuffer);
    return new Blob([wavBuffer], { type: 'audio/wav' });
  } finally {
    await audioContext.close().catch(() => undefined);
  }
}

function encodeWav(audioBuffer: AudioBuffer): ArrayBuffer {
  const channelCount = audioBuffer.numberOfChannels;
  const sampleRate = audioBuffer.sampleRate;
  const samplesPerChannel = audioBuffer.length;
  const bytesPerSample = 2;
  const blockAlign = channelCount * bytesPerSample;
  const dataSize = samplesPerChannel * blockAlign;
  const buffer = new ArrayBuffer(44 + dataSize);
  const view = new DataView(buffer);
  const channels = Array.from({ length: channelCount }, (_, index) => audioBuffer.getChannelData(index));

  writeAscii(view, 0, 'RIFF');
  view.setUint32(4, 36 + dataSize, true);
  writeAscii(view, 8, 'WAVE');
  writeAscii(view, 12, 'fmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, channelCount, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * blockAlign, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, bytesPerSample * 8, true);
  writeAscii(view, 36, 'data');
  view.setUint32(40, dataSize, true);

  let offset = 44;
  for (let sampleIndex = 0; sampleIndex < samplesPerChannel; sampleIndex += 1) {
    for (let channelIndex = 0; channelIndex < channelCount; channelIndex += 1) {
      const sample = Math.max(-1, Math.min(1, channels[channelIndex][sampleIndex]));
      view.setInt16(offset, sample < 0 ? sample * 0x8000 : sample * 0x7fff, true);
      offset += bytesPerSample;
    }
  }

  return buffer;
}

function writeAscii(view: DataView, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}
