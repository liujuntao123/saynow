import { cancelNativeRecording, startNativeRecording, stopNativeRecording } from '../api/tauri';
import type { RecordedAudio } from '../types';

export function createAudioRecorder() {
  let active = false;
  let stopPromise: Promise<RecordedAudio | null> | null = null;

  return {
    get active() {
      return active;
    },
    async start() {
      if (active) return;
      stopPromise = null;
      await startNativeRecording();
      active = true;
    },
    async stop() {
      if (!active) return null;
      if (!stopPromise) {
        stopPromise = stopNativeRecording().finally(() => {
          active = false;
          stopPromise = null;
        });
      }
      return stopPromise;
    },
    cancel() {
      if (!active) return;
      active = false;
      stopPromise = null;
      void cancelNativeRecording().catch(() => undefined);
    },
  };
}
