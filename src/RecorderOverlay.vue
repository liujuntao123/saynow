<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import AppIcon from './components/AppIcon.vue';

const state = ref<'recording' | 'processing' | 'error'>('recording');
const transcript = ref('');
let unlistenState: (() => void) | undefined;
let unlistenTranscript: (() => void) | undefined;
let unlistenReset: (() => void) | undefined;

const label = computed(() => {
  if (state.value === 'processing') return '正在识别';
  if (state.value === 'error') return '识别失败';
  return '正在录音';
});

const transcriptPreview = computed(() => transcript.value.trim());
const ariaLabel = computed(() => {
  if (transcriptPreview.value) return `${label.value}：${transcriptPreview.value}`;
  return label.value;
});

onMounted(async () => {
  document.documentElement.classList.add('recorder-root');
  document.body.classList.add('recorder-body');
  const currentWindow = getCurrentWindow();
  unlistenState = await currentWindow.listen<{ state: 'recording' | 'processing' | 'error' }>('recorder-state', (event) => {
    state.value = event.payload.state;
  });
  unlistenTranscript = await currentWindow.listen<{ text: string; done?: boolean }>('recorder-transcript', (event) => {
    transcript.value = event.payload.text ?? '';
  });
  unlistenReset = await currentWindow.listen('recorder-reset', () => {
    state.value = 'recording';
    transcript.value = '';
  });
});

onBeforeUnmount(() => {
  document.documentElement.classList.remove('recorder-root');
  document.body.classList.remove('recorder-body');
  unlistenState?.();
  unlistenTranscript?.();
  unlistenReset?.();
});
</script>

<template>
  <div
    class="morph-hud recorder-overlay"
    :class="[state, { 'has-transcript': transcriptPreview }]"
    role="status"
    :aria-label="ariaLabel"
  >
    <div class="hud-orb">
      <div class="orb-halo"></div>
      <div class="orb-ring"></div>
      <div class="orb-icon-wrapper">
        <AppIcon :name="state === 'processing' ? 'activity' : 'mic'" class="orb-icon" />
      </div>
    </div>

    <div class="hud-strip">
      <div class="strip-content">
        <span class="strip-text">{{ transcriptPreview }}</span>
        <span class="strip-cursor"></span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.morph-hud {
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent !important;
  border: none !important;
  box-shadow: none !important;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
  width: fit-content !important;
  max-width: 100vw !important;
  height: max-content !important;
  padding: 12px !important;
}

.hud-orb {
  position: relative;
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.7);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.12),
    inset 0 1px 1px rgba(255, 255, 255, 1),
    inset 0 0 0 1px rgba(255, 255, 255, 0.6);

  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  flex-shrink: 0;
  overflow: visible;
  transition:
    background 0.4s ease,
    box-shadow 0.4s ease,
    transform 0.4s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.orb-halo {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  opacity: 0.15;
  transition: all 0.5s ease;
}

.orb-ring {
  position: absolute;
  inset: -2px;
  border-radius: 50%;
  pointer-events: none;
  opacity: 0;
  transition: all 0.4s ease;
}

.orb-ring::after {
  position: absolute;
  content: '';
  inset: 0;
  border-radius: inherit;
}

.orb-icon-wrapper {
  position: relative;
  z-index: 10;
  display: flex;
  transition: transform 0.4s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.orb-icon {
  font-size: 16px;
  transition: color 0.4s ease;
}

.morph-hud.recording .orb-halo {
  background: linear-gradient(135deg, #0f8f83, #2dd4bf);
  animation: orb-breathe 1.7s ease-in-out infinite alternate;
}
.morph-hud.recording .hud-orb {
  background: rgba(244, 255, 253, 0.86);
  box-shadow:
    0 10px 28px rgba(15, 143, 131, 0.22),
    0 0 0 0 rgba(15, 143, 131, 0.28),
    inset 0 1px 1px rgba(255, 255, 255, 1),
    inset 0 0 0 1px rgba(255, 255, 255, 0.75);
  animation: orb-recording-pop 1.4s ease-in-out infinite;
}
.morph-hud.recording .orb-icon {
  color: #0f8f83;
  animation: mic-listening 1.2s ease-in-out infinite;
}
.morph-hud.recording .orb-ring {
  opacity: 1;
  border: 1.5px solid rgba(15, 143, 131, 0.4);
  animation: ring-ripple 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) infinite;
}
.morph-hud.recording .orb-ring::after {
  border: 1px solid rgba(15, 143, 131, 0.24);
  animation: ring-ripple 1.4s cubic-bezier(0.2, 0.8, 0.2, 1) 0.45s infinite;
}

.morph-hud.processing .orb-halo {
  background: linear-gradient(135deg, #f59e0b, #f97316);
  opacity: 0.25;
}
.morph-hud.processing .orb-icon {
  color: #d97706;
}
.morph-hud.processing .orb-icon-wrapper {
  transform: scale(0.85);
}
.morph-hud.processing .orb-ring {
  opacity: 1;
  border: 2px solid transparent;
  border-top-color: #f59e0b;
  border-right-color: rgba(245, 158, 11, 0.3);
  animation: ring-spin 0.8s cubic-bezier(0.5, 0.1, 0.5, 0.9) infinite;
}

.morph-hud.error .orb-halo {
  background: #ef4444;
  opacity: 0.25;
}
.morph-hud.error .orb-icon {
  color: #ef4444;
  animation: icon-shake 0.4s ease both;
}

.hud-strip {
  position: relative;
  z-index: 1;
  max-width: 0;
  opacity: 0;
  height: 32px;
  margin-left: -19px;

  background: linear-gradient(90deg, rgba(255, 255, 255, 0.96), rgba(250, 253, 252, 0.88));
  backdrop-filter: blur(16px) saturate(170%);
  -webkit-backdrop-filter: blur(16px) saturate(170%);
  border-radius: 16px;
  box-shadow:
    0 8px 24px rgba(15, 23, 42, 0.14),
    inset 0 0 0 1px rgba(15, 143, 131, 0.12),
    inset 0 1px 1px rgba(255, 255, 255, 0.95);

  transform: scaleX(0);
  transform-origin: left center;
  transition:
    max-width 0.6s cubic-bezier(0.34, 1.56, 0.64, 1),
    opacity 0.4s ease,
    transform 0.6s cubic-bezier(0.34, 1.56, 0.64, 1),
    padding 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);

  overflow: hidden;
  display: flex;
  align-items: center;
}

.morph-hud.has-transcript .hud-strip {
  max-width: calc(100vw - 108px);
  opacity: 1;
  transform: scaleX(1);
  padding: 0 16px 0 28px;
}

.strip-content {
  display: flex;
  align-items: center;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
  -webkit-mask-image: linear-gradient(90deg, #000 85%, transparent 100%);
  mask-image: linear-gradient(90deg, #000 85%, transparent 100%);
}

.strip-text {
  display: block;
  min-width: 0;
  font-size: 14px;
  font-weight: 650;
  color: #0f172a;
  letter-spacing: 0;
  text-shadow: 0 1px 1px rgba(255, 255, 255, 0.95);
  overflow: hidden;
  text-overflow: ellipsis;
}

.strip-cursor {
  display: inline-block;
  width: 2px;
  height: 14px;
  background: #0f8f83;
  margin-left: 4px;
  border-radius: 2px;
  opacity: 0;
}

.morph-hud.recording.has-transcript .strip-cursor {
  opacity: 1;
  animation: cursor-blink 1s step-end infinite;
}

.morph-hud.processing .strip-text {
  background: linear-gradient(110deg, #64748b 0%, #cbd5e1 50%, #64748b 100%);
  background-size: 200% auto;
  color: transparent;
  -webkit-background-clip: text;
  background-clip: text;
  animation: text-shimmer 1.5s linear infinite;
  text-shadow: none;
}

@keyframes orb-breathe {
  0% { transform: scale(0.9); opacity: 0.1; }
  100% { transform: scale(1.1); opacity: 0.25; }
}

@keyframes orb-recording-pop {
  0%, 100% {
    transform: scale(1);
    box-shadow:
      0 10px 28px rgba(15, 143, 131, 0.22),
      0 0 0 0 rgba(15, 143, 131, 0.28),
      inset 0 1px 1px rgba(255, 255, 255, 1),
      inset 0 0 0 1px rgba(255, 255, 255, 0.75);
  }
  50% {
    transform: scale(1.06);
    box-shadow:
      0 12px 32px rgba(15, 143, 131, 0.26),
      0 0 0 8px rgba(15, 143, 131, 0),
      inset 0 1px 1px rgba(255, 255, 255, 1),
      inset 0 0 0 1px rgba(255, 255, 255, 0.9);
  }
}

@keyframes mic-listening {
  0%, 100% { transform: translateY(0) scale(1); }
  45% { transform: translateY(-1px) scale(1.08); }
}

@keyframes ring-ripple {
  0% { transform: scale(1); opacity: 0.8; }
  100% { transform: scale(1.6); opacity: 0; }
}

@keyframes ring-spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

@keyframes text-shimmer {
  to { background-position: 200% center; }
}

@keyframes cursor-blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}

@keyframes icon-shake {
  0%, 100% { transform: translateX(0); }
  25% { transform: translateX(-2px); }
  75% { transform: translateX(2px); }
}
</style>
