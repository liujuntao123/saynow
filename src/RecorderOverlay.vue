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

const isLoading = computed(() => state.value === 'recording' || state.value === 'processing');
const loadingDots = [0, 1, 2, 3, 4];
const transcriptPreview = computed(() => transcript.value.trim());
const correctionPreview = computed(() => correctionRawText.value.trim());
const stripText = computed(() => {
  if (state.value === 'correctionPrompt') return correctionPreview.value || label.value;
  if (state.value === 'error') return '识别失败，可点击重试';
  // 录音 / 识别中只展示 loading 点，识别完成后由纠错/编辑状态展示最终文本
  return '';
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

async function retryRecognition() {
  await emitTo('main', 'recognition-retry');
}

/** 悬停时通知主窗口暂停纠错浮窗自动关闭。 */
async function notifyHover(hovering: boolean) {
  if (state.value !== 'correctionPrompt') return;
  await emitTo('main', 'correction-hover', { hovering }).catch(() => undefined);
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
    if (event.payload.state === 'processing') {
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
    // 流式转写只用于识别过程推进，不在“正在识别”状态展示；识别完成后由纠错/编辑状态展示最终文本
    if (state.value === 'processing') return;
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
    @mouseenter="notifyHover(true)"
    @mouseleave="notifyHover(false)"
  >
    <div v-if="state !== 'correctionEdit'" class="hud-strip">
      <div v-if="isLoading" class="loading-dots" aria-hidden="true">
        <span v-for="dotIndex in loadingDots" :key="dotIndex" class="loading-dot"></span>
      </div>

      <div v-if="stripText" class="strip-content">
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

      <div v-if="state === 'error'" class="correction-actions">
        <button type="button" class="correction-button" @click="retryRecognition">
          <AppIcon name="undo" />
          <span>重试</span>
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
  min-width: 0;
  max-width: min(640px, calc(100vw - 10px));
  min-height: 40px;
  align-items: center;
  gap: 8px;
  overflow: visible;
  border-radius: 20px;
  padding: 4px 5px;
  background: #e5efff;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.55),
    inset 0 -1px 0 rgba(15, 23, 42, 0.05);
  transition:
    background 0.4s ease,
    box-shadow 0.4s ease,
    transform 0.4s cubic-bezier(0.34, 1.56, 0.64, 1),
    padding 0.4s ease,
    min-width 0.5s cubic-bezier(0.22, 1, 0.36, 1),
    max-width 0.5s cubic-bezier(0.22, 1, 0.36, 1);
}

/* 录音与识别合并为同一个阶段：窄条 + 动态 loading 点 */
.morph-hud.recording .hud-strip {
  min-width: 150px;
  max-width: 150px;
  justify-content: center;
}

.morph-hud.processing .hud-strip {
  min-width: 150px;
  max-width: 150px;
  justify-content: center;
}

.loading-dots {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 12px;
}

.loading-dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: #1d4ed8;
  animation: loading-dot-march 1.2s linear infinite;
}

.loading-dot:nth-child(2) {
  animation-delay: -120ms;
}

.loading-dot:nth-child(3) {
  animation-delay: -240ms;
}

.loading-dot:nth-child(4) {
  animation-delay: -360ms;
}

.loading-dot:nth-child(5) {
  animation-delay: -480ms;
}

/* 失败：红色胶囊 */
.morph-hud.error .hud-strip {
  min-width: 0;
  max-width: min(640px, calc(100vw - 10px));
  background: #fee2e2;
}

/* 纠错：蓝色胶囊 */
.morph-hud.correctionPrompt .hud-strip {
  background: #e5efff;
  padding-left: 16px;
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
  animation: strip-content-in 0.45s cubic-bezier(0.22, 1, 0.36, 1) both;
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

.correction-actions {
  display: flex;
  align-items: center;
  gap: 5px;
  flex: 0 0 auto;
  margin-left: 2px;
  padding-left: 8px;
  border-left: 1px solid rgba(15, 143, 131, 0.12);
  animation: strip-content-in 0.45s cubic-bezier(0.22, 1, 0.36, 1) both;
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
  border: 1px solid rgba(15, 23, 42, 0.08);
  background: #ffffff;
  box-shadow: 0 6px 24px rgba(15, 23, 42, 0.16);
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

/* 点队列从左到右循环前进：右侧淡出后从左侧重新进入 */
@keyframes loading-dot-march {
  0% {
    opacity: 0;
    transform: translateX(-36px) scale(0.9);
  }
  12% {
    opacity: 1;
    transform: translateX(-30px) scale(1);
  }
  88% {
    opacity: 1;
    transform: translateX(30px) scale(1);
  }
  100% {
    opacity: 0;
    transform: translateX(36px) scale(0.9);
  }
}

@keyframes strip-content-in {
  from {
    opacity: 0;
    transform: translateX(-6px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

@keyframes text-shimmer {
  to { background-position: 200% center; }
}

@keyframes cursor-blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}
</style>
