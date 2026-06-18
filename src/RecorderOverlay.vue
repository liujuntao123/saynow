<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { emitTo } from '@tauri-apps/api/event';
import AppIcon from './components/AppIcon.vue';
import { writeRuntimeLog } from './api/tauri';

type OverlayState = 'recording' | 'processing' | 'error' | 'correctionPrompt' | 'correctionEdit';

const state = ref<OverlayState>('recording');
const transcript = ref('');
const correctionRecordId = ref<number | null>(null);
const correctionRawText = ref('');
const correctionDraft = ref('');
let unlistenState: (() => void) | undefined;
let unlistenTranscript: (() => void) | undefined;
let unlistenReset: (() => void) | undefined;

const label = computed(() => {
  if (state.value === 'correctionPrompt') return '已输入';
  if (state.value === 'correctionEdit') return '纠正文本';
  if (state.value === 'processing') return '正在识别';
  if (state.value === 'error') return '识别失败';
  return '正在录音';
});

const transcriptPreview = computed(() => transcript.value.trim());
const correctionPreview = computed(() => correctionRawText.value.trim());
const stripText = computed(() => {
  if (state.value === 'correctionPrompt') return correctionPreview.value || label.value;
  return transcriptPreview.value || label.value;
});
const ariaLabel = computed(() => {
  const preview = state.value.startsWith('correction') ? correctionPreview.value : transcriptPreview.value;
  if (preview) return `${label.value}：${preview}`;
  return label.value;
});

function debugLog(message: string, details?: unknown) {
  const formatted = details === undefined ? `[saynow] recorder ${message}` : `[saynow] recorder ${message} ${formatLogDetails(details)}`;
  void writeRuntimeLog(formatted).catch(() => undefined);
}

function formatLogDetails(details: unknown) {
  try {
    return JSON.stringify(details);
  } catch {
    return String(details);
  }
}

function resetCorrection() {
  correctionRecordId.value = null;
  correctionRawText.value = '';
  correctionDraft.value = '';
}

async function beginCorrectionEdit() {
  if (!correctionRawText.value.trim()) return;
  correctionDraft.value = correctionRawText.value;
  state.value = 'correctionEdit';
  await emitTo('main', 'correction-edit-requested');
}

async function submitCorrection() {
  if (!correctionRecordId.value) return;
  const correctedText = correctionDraft.value.trim();
  if (!correctedText || correctedText === correctionRawText.value.trim()) {
    await dismissCorrection();
    return;
  }
  await emitTo('main', 'correction-submit', {
    recognitionRecordId: correctionRecordId.value,
    rawText: correctionRawText.value,
    correctedText,
  });
}

async function dismissCorrection() {
  await emitTo('main', 'correction-dismiss');
}

async function undoCorrectionTarget() {
  await emitTo('main', 'correction-undo');
}

function handleEditKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault();
    void dismissCorrection();
  }
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    void submitCorrection();
  }
}

onMounted(async () => {
  const mountedAt = performance.now();
  document.documentElement.classList.add('recorder-root');
  document.body.classList.add('recorder-body');
  const currentWindow = getCurrentWindow();
  debugLog('mounted');
  unlistenState = await currentWindow.listen<{
    state: OverlayState;
    recordId?: number;
    text?: string;
  }>('recorder-state', (event) => {
    debugLog('state event received', {
      state: event.payload.state,
      mountedMs: Math.round(performance.now() - mountedAt),
    });
    state.value = event.payload.state;
    if (event.payload.state === 'correctionPrompt') {
      correctionRecordId.value = event.payload.recordId ?? null;
      correctionRawText.value = event.payload.text ?? '';
      correctionDraft.value = correctionRawText.value;
      transcript.value = '';
    }
    if (event.payload.state === 'recording' || event.payload.state === 'processing' || event.payload.state === 'error') {
      resetCorrection();
    }
  });
  unlistenTranscript = await currentWindow.listen<{ text: string; done?: boolean }>('recorder-transcript', (event) => {
    debugLog('transcript event received', {
      done: Boolean(event.payload.done),
      textLength: event.payload.text?.length ?? 0,
    });
    transcript.value = event.payload.text ?? '';
  });
  unlistenReset = await currentWindow.listen('recorder-reset', () => {
    debugLog('reset event received');
    state.value = 'recording';
    transcript.value = '';
    resetCorrection();
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
    :class="[state, { 'has-transcript': transcriptPreview, 'has-correction': correctionPreview }]"
    role="status"
    :aria-label="ariaLabel"
  >
    <div v-if="state !== 'correctionEdit'" class="hud-strip">
      <div class="hud-orb">
        <div class="orb-halo"></div>
        <div class="orb-ring"></div>
        <div class="orb-icon-wrapper">
          <AppIcon :name="state === 'processing' ? 'activity' : state === 'correctionPrompt' ? 'text' : 'mic'" class="orb-icon" />
        </div>
      </div>

      <div class="strip-content">
        <span class="strip-text">{{ stripText }}</span>
        <span class="strip-cursor"></span>
      </div>

      <div v-if="state === 'correctionPrompt'" class="correction-actions">
        <button type="button" class="correction-button" @click="beginCorrectionEdit">
          <AppIcon name="text" />
          <span>编辑</span>
        </button>
        <button type="button" class="correction-icon-button" aria-label="撤销刚才输入" @click="undoCorrectionTarget">
          <AppIcon name="undo" />
        </button>
        <button type="button" class="correction-icon-button" aria-label="关闭" @click="dismissCorrection">
          <AppIcon name="x" />
        </button>
      </div>
    </div>

    <form v-if="state === 'correctionEdit'" class="correction-editor" @submit.prevent="submitCorrection">
      <textarea
        v-model="correctionDraft"
        class="correction-textarea"
        rows="4"
        autofocus
        @keydown="handleEditKeydown"
      ></textarea>
      <div class="correction-editor-actions">
        <button type="button" class="correction-secondary" @click="dismissCorrection">取消</button>
        <button type="submit" class="correction-primary">
          <AppIcon name="check" />
          <span>确认替换</span>
        </button>
      </div>
    </form>
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
  padding: 5px !important;
}

.hud-strip {
  position: relative;
  z-index: 1;
  display: flex;
  width: fit-content;
  min-width: 136px;
  max-width: min(640px, calc(100vw - 10px));
  min-height: 40px;
  align-items: center;
  gap: 8px;
  overflow: hidden;
  border-radius: 20px;
  padding: 4px 5px;
  background: #ffffff;
  box-shadow:
    0 10px 28px rgba(15, 23, 42, 0.14),
    inset 0 0 0 1px rgba(15, 143, 131, 0.12),
    inset 0 1px 1px rgba(255, 255, 255, 0.95);
}

.hud-orb {
  position: relative;
  flex: 0 0 30px;
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: #f8fbfa;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: inset 0 0 0 1px rgba(15, 143, 131, 0.12);

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
  opacity: 1;
  transition: all 0.5s ease;
}

.orb-ring {
  position: absolute;
  inset: -1px;
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
  font-size: 13px;
  transition: color 0.4s ease;
}

.morph-hud.recording .orb-halo {
  background: #d7faf4;
  animation: orb-breathe 1.7s ease-in-out infinite alternate;
}
.morph-hud.recording .hud-orb {
  background: #effdf9;
  box-shadow: inset 0 0 0 1px rgba(15, 143, 131, 0.16);
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
  background: #fff3d6;
  opacity: 1;
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
  background: #fee2e2;
  opacity: 1;
}
.morph-hud.error .orb-icon {
  color: #ef4444;
  animation: icon-shake 0.4s ease both;
}

.morph-hud.correctionPrompt .orb-halo {
  background: #e5efff;
  opacity: 1;
}

.morph-hud.correctionPrompt .hud-orb {
  background: #f8fbff;
  box-shadow: inset 0 0 0 1px rgba(29, 78, 216, 0.14);
}

.morph-hud.correctionPrompt .orb-icon {
  color: #1d4ed8;
}

.morph-hud.has-transcript .hud-strip {
  max-width: min(640px, calc(100vw - 10px));
}

.morph-hud.has-correction .hud-strip {
  max-width: min(640px, calc(100vw - 10px));
}

.strip-content {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
}

.strip-text {
  display: block;
  min-width: 0;
  font-size: 12px;
  font-weight: 650;
  color: #0f172a;
  letter-spacing: 0;
  text-shadow: 0 1px 1px rgba(255, 255, 255, 0.95);
  overflow: hidden;
  text-overflow: ellipsis;
}

.strip-cursor {
  flex: 0 0 auto;
  display: inline-block;
  width: 2px;
  height: 12px;
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
  0% { transform: scale(0.96); }
  100% { transform: scale(1.06); }
}

.correction-actions {
  display: flex;
  align-items: center;
  gap: 5px;
  flex: 0 0 auto;
  margin-left: 2px;
  padding-left: 8px;
  border-left: 1px solid rgba(15, 143, 131, 0.12);
}

.correction-button,
.correction-icon-button,
.correction-primary,
.correction-secondary {
  border: 0;
  color: #0f172a;
  background: transparent;
  cursor: pointer;
  letter-spacing: 0;
}

.correction-button,
.correction-primary {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 28px;
  border-radius: 8px;
  padding: 0 9px;
  font-size: 12px;
  font-weight: 750;
}

.correction-button {
  background: #e7f7f5;
  color: #08776f;
}

.correction-icon-button {
  display: grid;
  width: 28px;
  height: 28px;
  place-items: center;
  border-radius: 8px;
  color: #64748b;
}

.correction-editor {
  display: grid;
  gap: 8px;
  width: min(600px, calc(100vw - 20px));
  padding: 10px;
  border-radius: 12px;
  background: #ffffff;
  box-shadow:
    0 18px 42px rgba(15, 23, 42, 0.2),
    inset 0 0 0 1px rgba(15, 143, 131, 0.12);
}

.correction-textarea {
  width: 100%;
  min-height: 82px;
  max-height: 126px;
  resize: vertical;
  border: 1px solid rgba(15, 143, 131, 0.18);
  border-radius: 8px;
  padding: 8px 10px;
  color: #0f172a;
  background: #ffffff;
  font-size: 13px;
  line-height: 1.5;
}

.correction-textarea:focus {
  border-color: rgba(15, 143, 131, 0.48);
  box-shadow: 0 0 0 3px rgba(15, 143, 131, 0.1);
}

.correction-editor-actions {
  display: flex;
  justify-content: flex-end;
  gap: 7px;
}

.correction-secondary {
  min-height: 30px;
  border-radius: 8px;
  padding: 0 11px;
  color: #64748b;
  background: #f1f5f9;
  font-size: 12px;
  font-weight: 700;
}

.correction-primary {
  min-height: 30px;
  color: #ffffff;
  background: #0f8f83;
  box-shadow: 0 8px 18px rgba(15, 143, 131, 0.18);
}

@keyframes orb-recording-pop {
  0%, 100% {
    transform: scale(1);
    box-shadow: inset 0 0 0 1px rgba(15, 143, 131, 0.16);
  }
  50% {
    transform: scale(1.04);
    box-shadow: inset 0 0 0 1px rgba(15, 143, 131, 0.24);
  }
}

@keyframes mic-listening {
  0%, 100% { transform: translateY(0) scale(1); }
  45% { transform: translateY(-1px) scale(1.08); }
}

@keyframes ring-ripple {
  0% { transform: scale(1); opacity: 0.8; }
  100% { transform: scale(1.34); opacity: 0; }
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
