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
      startedAt = Date.now();
      mediaRecorder.start();
    },
    async stop() {
      if (!mediaRecorder || mediaRecorder.state === 'inactive') {
        cleanup();
        return null;
      }

      if (!stopPromise) {
        const recorder = mediaRecorder;
        stopPromise = new Promise<RecordedAudio>((resolve) => {
          recorder.addEventListener(
            'stop',
            async () => {
              const blob = new Blob(chunks, { type: recorder.mimeType || 'audio/webm' });
              const durationSeconds = Math.max(1, Math.round((Date.now() - startedAt) / 1000));
              const audioBase64 = await blobToBase64(blob);
              cleanup();
              resolve({ audioBase64, durationSeconds, mimeType: blob.type || 'audio/webm' });
            },
            { once: true },
          );
          recorder.stop();
        });
      }

      return stopPromise;
    },
  };

  function cleanup() {
    stream?.getTracks().forEach((track) => track.stop());
    stream = null;
    mediaRecorder = null;
    chunks = [];
    startedAt = 0;
    stopPromise = null;
  }
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
