<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { emitTo, listen } from '@tauri-apps/api/event';
import { currentMonitor, cursorPosition, monitorFromPoint, primaryMonitor, type Monitor } from '@tauri-apps/api/window';
import AppShell from './components/AppShell.vue';
import ToastHost from './components/ToastHost.vue';
import ConfirmDialog from './components/ConfirmDialog.vue';
import { useToast } from './composables/useToast';
import { useConfirm } from './composables/useConfirm';
import {
  addStylePrompt,
  addVocabularyTerms,
  clearCorrectionRecords,
  clearLearningRules,
  clearRecords,
  deleteLearningRule,
  deleteRecord,
  deleteStylePrompt,
  deleteVocabulary,
  getConfig,
  getDashboard,
  getLearningEngineConfig,
  hideRecorderOverlayWindow,
  listCorrectionRecords,
  listLearningRules,
  listProviderConfigs,
  listRecords,
  listStylePrompts,
  listVocabulary,
  rememberInputTarget,
  recognizeAudio,
  retryLastRecognition,
  saveConfig,
  saveLearningEngineConfig,
  saveProviderConfig,
  deleteProviderConfig,
  selectProviderConfig,
  getPersonalizationPreferences,
  saveCorrection,
  setHotkeyMonitor,
  setRecorderOverlayPosition,
  setRecorderOverlaySize,
  showRecorderOverlayNoActivate,
  showRecorderOverlayFocus,
  savePersonalizationPreferences,
  runLearningEngine,
  undoLastInjectedText,
  updateLearningRuleStatus,
  updateStylePrompt,
  updateVocabulary,
  writeRuntimeLog,
} from './api/tauri';
import { createAudioRecorder } from './domain/audioRecorder';
import { isEventPartOfHotkey, toHotkeyParts } from './domain/hotkeyPresets';
import { calculateRecorderOverlayPosition } from './domain/recorderOverlayPosition';
import ConfigPage from './pages/ConfigPage.vue';
import DataPage from './pages/DataPage.vue';
import FeedbackPage from './pages/FeedbackPage.vue';
import HomePage from './pages/HomePage.vue';
import PersonalizationPage from './pages/PersonalizationPage.vue';
import type {
  AppConfig,
  CorrectionRecord,
  DashboardData,
  LearningEngineConfig,
  LearningRule,
  PersonalizationPreferences,
  ProviderConfig,
  RecognitionRecord,
  StylePrompt,
  VocabularyItem,
} from './types';

const activePage = ref('home');
const toast = useToast();
const { confirm } = useConfirm();
const dashboard = ref<DashboardData | null>(null);
const config = ref<AppConfig | null>(null);
const providers = ref<ProviderConfig[]>([]);
const records = ref<RecognitionRecord[]>([]);
const correctionRecords = ref<CorrectionRecord[]>([]);
const vocabulary = ref<VocabularyItem[]>([]);
const styles = ref<StylePrompt[]>([]);
const personalizationPreferences = ref<PersonalizationPreferences>({ removeTrailingPeriod: false });
const learningEngineConfig = ref<LearningEngineConfig>({
  enabled: false,
  provider: '',
  baseUrl: '',
  model: '',
  apiKeyRef: '',
  runMode: 'llmAssist',
  minNewCorrections: 5,
  idleSeconds: 30,
});
const learningRules = ref<LearningRule[]>([]);
const busy = ref(false);
const saving = ref(false);
const hotkeyRecording = ref(false);
const audioRecorder = createAudioRecorder();
let recordingSession: RecordingSession | null = null;
let unlistenHotkeyState: (() => void) | null = null;
let recordingGuardTimer: ReturnType<typeof window.setTimeout> | null = null;
let hotkeyMonitorRetryTimer: ReturnType<typeof window.setTimeout> | null = null;
let hotkeyStateListenRetryTimer: ReturnType<typeof window.setTimeout> | null = null;
let correctionPromptTimer: ReturnType<typeof window.setTimeout> | null = null;
let errorOverlayHideTimer: ReturnType<typeof window.setTimeout> | null = null;
let learningEngineIdleTimer: ReturnType<typeof window.setTimeout> | null = null;
let unlistenCorrectionEditRequested: (() => void) | null = null;
let unlistenCorrectionSubmit: (() => void) | null = null;
let unlistenCorrectionDismiss: (() => void) | null = null;
let unlistenCorrectionUndo: (() => void) | null = null;
let unlistenCorrectionHover: (() => void) | null = null;
let unlistenRecognitionRetry: (() => void) | null = null;
let mounted = false;
const RECORDER_OVERLAY_COMPACT_SIZE = { width: 660, height: 54 };
const RECORDER_OVERLAY_EDITOR_SIZE = { width: 640, height: 152 };
const MAX_HOTKEY_RECORDING_MS = 120_000;
const HOTKEY_MONITOR_RETRY_MS = 1500;
const HOTKEY_STATE_LISTEN_RETRY_MS = 1500;
const CORRECTION_PROMPT_TIMEOUT_MS = 7000;

type RecordingPhase = 'starting' | 'recording' | 'stopping';

interface RecordingSession {
  id: number;
  phase: RecordingPhase;
  startedAt: number;
  startPromise: Promise<void>;
  stopRequested: boolean;
}

const currentPage = computed(() => activePage.value);
const runtimeHotkeyEnabled = computed(() => Boolean(config.value?.hotkey) && !saving.value);
const isTauriRuntime = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

function debugLog(message: string, details?: unknown) {
  const formatted = details === undefined ? `[saynow] ${message}` : `[saynow] ${message} ${formatLogDetails(details)}`;
  void writeRuntimeLog(formatted).catch(() => undefined);
  if (details === undefined) {
    console.info(`[saynow] ${message}`);
  } else {
    console.info(`[saynow] ${message}`, details);
  }
}

function formatLogDetails(details: unknown) {
  try {
    return JSON.stringify(details);
  } catch {
    return String(details);
  }
}

async function refreshRecognitionData() {
  const [nextDashboard, nextRecords] = await Promise.all([getDashboard(), listRecords()]);
  dashboard.value = nextDashboard;
  records.value = nextRecords;
}

async function refreshLearningData() {
  const [nextCorrections, nextRules] = await Promise.all([
    listCorrectionRecords(),
    listLearningRules(),
  ]);
  correctionRecords.value = nextCorrections;
  learningRules.value = nextRules;
}

async function refreshProviderData() {
  const [nextConfig, nextProviders] = await Promise.all([getConfig(), listProviderConfigs()]);
  config.value = nextConfig;
  providers.value = nextProviders;
}

async function refreshAll() {
  debugLog('refreshing app data');
  const [
    nextDashboard,
    nextConfig,
    nextProviders,
    nextRecords,
    nextCorrections,
    nextVocabulary,
    nextStyles,
    nextPreferences,
    nextLearningEngineConfig,
    nextLearningRules,
  ] = await Promise.all([
    getDashboard(),
    getConfig(),
    listProviderConfigs(),
    listRecords(),
    listCorrectionRecords(),
    listVocabulary(),
    listStylePrompts(),
    getPersonalizationPreferences(),
    getLearningEngineConfig(),
    listLearningRules(),
  ]);
  dashboard.value = nextDashboard;
  config.value = nextConfig;
  providers.value = nextProviders;
  records.value = nextRecords;
  correctionRecords.value = nextCorrections;
  vocabulary.value = nextVocabulary;
  styles.value = nextStyles;
  personalizationPreferences.value = nextPreferences;
  learningEngineConfig.value = nextLearningEngineConfig;
  learningRules.value = nextLearningRules;
  debugLog('app data refreshed', {
    hotkey: config.value.hotkey,
    providers: providers.value.length,
    records: records.value.length,
    corrections: correctionRecords.value.length,
    vocabulary: vocabulary.value.length,
    styles: styles.value.length,
    removeTrailingPeriod: personalizationPreferences.value.removeTrailingPeriod,
    learningEngineEnabled: learningEngineConfig.value.enabled,
    learningRules: learningRules.value.length,
  });
}

function startHotkeyRecording() {
  if (busy.value || recordingSession) return;
  clearLearningEngineIdleTimer();
  hotkeyRecording.value = true;
  armRecordingGuard();
  const session: RecordingSession = {
    id: Date.now(),
    phase: 'starting',
    startedAt: performance.now(),
    startPromise: Promise.resolve(),
    stopRequested: false,
  };
  debugLog('hotkey pressed; starting recording', { sessionId: session.id });
  session.startPromise = beginRecording(session);
  recordingSession = session;
}

async function stopHotkeyRecording(reason = 'hotkey released') {
  const session = recordingSession;
  if (!session) return;
  debugLog(`${reason}; stopping recording`, {
    sessionId: session.id,
    phase: session.phase,
    elapsedMs: Math.round(performance.now() - session.startedAt),
  });
  hotkeyRecording.value = false;
  clearRecordingGuard();
  session.stopRequested = true;
  await finishRecording(session);
}

async function releaseHotkeyRecording() {
  await stopHotkeyRecording();
}

function armRecordingGuard() {
  clearRecordingGuard();
  recordingGuardTimer = window.setTimeout(() => {
    debugLog('hotkey recording exceeded watchdog timeout; forcing stop', { maxMs: MAX_HOTKEY_RECORDING_MS });
    void stopHotkeyRecording('hotkey watchdog timeout');
    void resetNativeHotkeyMonitor('recording watchdog timeout');
  }, MAX_HOTKEY_RECORDING_MS);
}

function clearRecordingGuard() {
  if (!recordingGuardTimer) return;
  window.clearTimeout(recordingGuardTimer);
  recordingGuardTimer = null;
}

async function persistConfig(nextConfig: AppConfig) {
  saving.value = true;
  try {
    debugLog('saving config', { provider: nextConfig.provider, model: nextConfig.model, hotkey: nextConfig.hotkey });
    config.value = await saveConfig(nextConfig);
    await refreshProviderData();
    await registerRuntimeHotkey(config.value.hotkey);
    toast.success('配置已保存');
    debugLog('config saved');
  } catch (error) {
    toast.error(error, '保存配置失败');
  } finally {
    saving.value = false;
  }
}

async function persistProvider(nextProvider: ProviderConfig) {
  saving.value = true;
  try {
    debugLog('saving provider config', { provider: nextProvider.provider, model: nextProvider.model, enabled: nextProvider.enabled });
    providers.value = await saveProviderConfig(nextProvider);
    await refreshProviderData();
    toast.success('供应商配置已保存');
    debugLog('provider config saved');
  } catch (error) {
    toast.error(error, '保存供应商失败');
  } finally {
    saving.value = false;
  }
}

async function activateProvider(id: number) {
  saving.value = true;
  try {
    debugLog('selecting provider config', { id });
    config.value = await selectProviderConfig(id);
    await refreshProviderData();
    await registerRuntimeHotkey(config.value.hotkey);
    toast.success('已切换启用的供应商');
    debugLog('provider config selected');
  } catch (error) {
    toast.error(error, '切换供应商失败');
  } finally {
    saving.value = false;
  }
}

async function removeProvider(id: number) {
  const target = providers.value.find((provider) => provider.id === id);
  const confirmed = await confirm({
    title: `删除供应商「${target?.provider ?? id}」？`,
    description: '删除后无法恢复；如果它是当前启用的供应商，会自动切换到其他配置。',
    confirmLabel: '删除',
    danger: true,
  });
  if (!confirmed) return;
  saving.value = true;
  try {
    debugLog('deleting provider config', { id });
    providers.value = await deleteProviderConfig(id);
    await refreshProviderData();
    toast.success('供应商已删除');
  } catch (error) {
    toast.error(error, '删除供应商失败');
  } finally {
    saving.value = false;
  }
}

async function beginRecording(session: RecordingSession) {
  try {
    const rememberStartedAt = performance.now();
    await rememberInputTarget().catch((error) => console.error('[saynow] failed to remember input target', error));
    debugLog('input target remembered', {
      sessionId: session.id,
      elapsedMs: Math.round(performance.now() - rememberStartedAt),
    });
    const microphoneStartedAt = performance.now();
    debugLog('starting audio recorder', { sessionId: session.id });
    await audioRecorder.start();
    const readyMs = Math.round(performance.now() - session.startedAt);
    debugLog('microphone recording started', {
      sessionId: session.id,
      readyMs,
      microphoneStartMs: Math.round(performance.now() - microphoneStartedAt),
    });
    if (recordingSession !== session || session.stopRequested) {
      debugLog('microphone started after hotkey release; canceling recording start', { sessionId: session.id, readyMs });
      audioRecorder.cancel();
      return;
    }
    session.phase = 'recording';
    await showRecorderOverlay('recording');
  } catch (error) {
    if (recordingSession === session) {
      hotkeyRecording.value = false;
      recordingSession = null;
      clearRecordingGuard();
      audioRecorder.cancel();
      await hideRecorderOverlay();
    }
    toast.error(error, '无法开始录音');
    console.error('[saynow] failed to start recording', error);
    void resetNativeHotkeyMonitor('recording start failed');
  }
}

async function finishRecording(session: RecordingSession) {
  if (session.phase === 'stopping') return;
  session.phase = 'stopping';
  busy.value = true;
  let shouldScheduleLearning = false;
  try {
    await session.startPromise;
    if (recordingSession !== session) return;
    if (!audioRecorder.active) {
      debugLog('recording stopped before microphone became active', { sessionId: session.id });
      return;
    }
    await showRecorderOverlay('processing');
    const stopStartedAt = performance.now();
    const audio = await audioRecorder.stop();
    debugLog('microphone recording stopped', audio ? {
      sessionId: session.id,
      durationSeconds: audio.durationSeconds,
      stopMs: Math.round(performance.now() - stopStartedAt),
      totalSessionMs: Math.round(performance.now() - session.startedAt),
      mimeType: audio.mimeType,
      bytesBase64: audio.audioBase64.length,
    } : { sessionId: session.id, empty: true });
    if (audio) {
      const recognitionStartedAt = performance.now();
      const record = await recognizeAudio(audio);
      debugLog('recognition finished', {
        sessionId: session.id,
        id: record.id,
        status: record.status,
        recognitionMs: Math.round(performance.now() - recognitionStartedAt),
        textLength: record.text.length,
        error: record.errorMessage,
      });
      await refreshRecognitionData();
      if (record.status === 'success' && record.text.trim()) {
        shouldScheduleLearning = true;
        await showCorrectionPrompt(record);
      }
    }
  } catch (error) {
    await showRecorderOverlay('error');
    await refreshRecognitionData();
    toast.error(error, '识别失败（可在浮窗点击重试）');
    console.error('[saynow] failed to finish recognition flow', error);
    scheduleErrorOverlayHide();
    return;
  } finally {
    busy.value = false;
    if (recordingSession === session) recordingSession = null;
  }
  if (shouldScheduleLearning) {
    scheduleLearningEngineRun();
  }
  if (!correctionPromptTimer) {
    await hideRecorderOverlay();
  }
}

async function showRecorderOverlay(state: 'recording' | 'processing' | 'error') {
  if (!isTauriRuntime) return;
  const startedAt = performance.now();
  clearCorrectionPromptTimer();
  clearErrorOverlayHideTimer();
  await resizeRecorderOverlay(RECORDER_OVERLAY_COMPACT_SIZE);
  await emitTo('recorder', 'recorder-state', { state });
  await positionRecorderOverlay(RECORDER_OVERLAY_COMPACT_SIZE);
  await showRecorderOverlayNoActivate();
  debugLog('recorder overlay shown', {
    state,
    elapsedMs: Math.round(performance.now() - startedAt),
  });
}

const ERROR_OVERLAY_HIDE_MS = 8000;

/** 错误浮条自动关闭；用户点重试或再次录音时取消。 */
function scheduleErrorOverlayHide() {
  clearErrorOverlayHideTimer();
  errorOverlayHideTimer = window.setTimeout(() => {
    errorOverlayHideTimer = null;
    void hideRecorderOverlay();
  }, ERROR_OVERLAY_HIDE_MS);
}

function clearErrorOverlayHideTimer() {
  if (!errorOverlayHideTimer) return;
  window.clearTimeout(errorOverlayHideTimer);
  errorOverlayHideTimer = null;
}

async function hideRecorderOverlay() {
  if (!isTauriRuntime) return;
  const startedAt = performance.now();
  clearCorrectionPromptTimer();
  clearErrorOverlayHideTimer();
  await hideRecorderOverlayWindow();
  await resetRecorderOverlay();
  debugLog('recorder overlay hidden', {
    elapsedMs: Math.round(performance.now() - startedAt),
  });
}

async function resetRecorderOverlay() {
  if (!isTauriRuntime) return;
  await emitTo('recorder', 'recorder-reset');
}

async function resizeRecorderOverlay(size: { width: number; height: number }) {
  await setRecorderOverlaySize(size.width, size.height).catch(() => undefined);
}

async function positionRecorderOverlay(size: { width: number; height: number }) {
  const monitor = await resolveRecorderMonitor();
  if (!monitor) return;
  const position = calculateRecorderOverlayPosition({
    workArea: monitor.workArea,
    scaleFactor: monitor.scaleFactor,
    overlaySize: size,
    marginBottom: 18,
  });
  await setRecorderOverlayPosition(position.x, position.y).catch(() => undefined);
}

async function resolveRecorderMonitor(): Promise<Monitor | null> {
  const cursor = await cursorPosition().catch(() => null);
  if (cursor) {
    const monitor = await monitorFromPoint(cursor.x, cursor.y).catch(() => null);
    if (monitor) return monitor;
  }

  return (await currentMonitor().catch(() => null)) ?? (await primaryMonitor().catch(() => null));
}

async function registerRuntimeHotkey(hotkey?: string) {
  if (!isTauriRuntime) return;
  clearHotkeyMonitorRetry();
  await setHotkeyMonitor(null).catch((error) => console.error('[saynow] failed to stop hotkey monitor', error));
  if (!hotkey) {
    debugLog('runtime hotkey disabled', { hotkey });
    return;
  }

  const parts = toHotkeyParts(hotkey);
  debugLog('using native hotkey monitor', { hotkey, parts });
  try {
    await setHotkeyMonitor(parts);
  } catch (error) {
    console.error('[saynow] failed to start hotkey monitor', error);
    scheduleHotkeyMonitorRetry();
  }
}

function scheduleHotkeyMonitorRetry() {
  if (!mounted || hotkeyMonitorRetryTimer || !runtimeHotkeyEnabled.value || !config.value?.hotkey) return;
  hotkeyMonitorRetryTimer = window.setTimeout(() => {
    hotkeyMonitorRetryTimer = null;
    void registerRuntimeHotkey(config.value?.hotkey);
  }, HOTKEY_MONITOR_RETRY_MS);
}

function clearHotkeyMonitorRetry() {
  if (!hotkeyMonitorRetryTimer) return;
  window.clearTimeout(hotkeyMonitorRetryTimer);
  hotkeyMonitorRetryTimer = null;
}

async function resetNativeHotkeyMonitor(reason: string) {
  if (!isTauriRuntime || !runtimeHotkeyEnabled.value || !config.value?.hotkey) return;
  debugLog('resetting native hotkey monitor', { reason });
  await registerRuntimeHotkey(config.value.hotkey);
}

function subscribeHotkeyStateEvents() {
  if (!isTauriRuntime || !mounted) return;
  clearHotkeyStateListenRetry();
  void listen<{ state: 'Pressed' | 'Released' }>('hotkey-state', (event) => {
    debugLog('hotkey state event', event.payload);
    if (event.payload.state === 'Pressed') startHotkeyRecording();
    if (event.payload.state === 'Released') {
      void releaseHotkeyRecording();
    }
  })
    .then((unlisten) => {
      unlistenHotkeyState = unlisten;
    })
    .catch((error) => {
      console.error('[saynow] failed to listen hotkey state', error);
      scheduleHotkeyStateListenRetry();
    });
}

function scheduleHotkeyStateListenRetry() {
  if (!mounted || hotkeyStateListenRetryTimer) return;
  hotkeyStateListenRetryTimer = window.setTimeout(() => {
    hotkeyStateListenRetryTimer = null;
    subscribeHotkeyStateEvents();
  }, HOTKEY_STATE_LISTEN_RETRY_MS);
}

function clearHotkeyStateListenRetry() {
  if (!hotkeyStateListenRetryTimer) return;
  window.clearTimeout(hotkeyStateListenRetryTimer);
  hotkeyStateListenRetryTimer = null;
}

async function showCorrectionPrompt(record: RecognitionRecord) {
  if (!isTauriRuntime) return;
  clearCorrectionPromptTimer();
  await resizeRecorderOverlay(RECORDER_OVERLAY_COMPACT_SIZE);
  await emitTo('recorder', 'recorder-state', {
    state: 'correctionPrompt',
    recordId: record.id,
    text: record.text,
  });
  await positionRecorderOverlay(RECORDER_OVERLAY_COMPACT_SIZE);
  await showRecorderOverlayNoActivate();
  armCorrectionPromptTimer();
}

function armCorrectionPromptTimer() {
  clearCorrectionPromptTimer();
  correctionPromptTimer = window.setTimeout(() => {
    void hideRecorderOverlay();
  }, CORRECTION_PROMPT_TIMEOUT_MS);
}

function clearCorrectionPromptTimer() {
  if (!correctionPromptTimer) return;
  window.clearTimeout(correctionPromptTimer);
  correctionPromptTimer = null;
}

/** 鼠标悬停在纠错浮窗上时暂停自动关闭，移出后重新计时。 */
function handleCorrectionHover(hovering: boolean) {
  if (hovering) {
    clearCorrectionPromptTimer();
  } else {
    armCorrectionPromptTimer();
  }
}

/** 用后端保留的上一段失败音频重试识别。 */
async function retryRecognition() {
  if (busy.value) return;
  busy.value = true;
  try {
    await showRecorderOverlay('processing');
    const record = await retryLastRecognition();
    await refreshRecognitionData();
    if (record.status === 'success' && record.text.trim()) {
      await showCorrectionPrompt(record);
      scheduleLearningEngineRun();
      return;
    }
    await hideRecorderOverlay();
  } catch (error) {
    toast.error(error, '重试识别失败');
    await showRecorderOverlay('error');
    await refreshRecognitionData();
    scheduleErrorOverlayHide();
  } finally {
    busy.value = false;
  }
}

async function openCorrectionEditor() {
  clearCorrectionPromptTimer();
  await resizeRecorderOverlay(RECORDER_OVERLAY_EDITOR_SIZE);
  await positionRecorderOverlay(RECORDER_OVERLAY_EDITOR_SIZE);
  await showRecorderOverlayFocus();
}

async function submitCorrection(payload: {
  recognitionRecordId: number;
  rawText: string;
  correctedText: string;
}) {
  clearCorrectionPromptTimer();
  try {
    const correction = await saveCorrection({
      ...payload,
      source: 'post-insert-overlay',
      applyReplacement: true,
    });
    debugLog('correction saved', {
      id: correction.id,
      recognitionRecordId: correction.recognitionRecordId,
      applied: correction.applied,
      error: correction.errorMessage,
    });
    if (correction.errorMessage) {
      toast.info(`纠错已保存，但替换文本失败：${correction.errorMessage}`);
    } else {
      toast.success('纠错已保存');
    }
    await refreshLearningData();
    scheduleLearningEngineRun();
  } catch (error) {
    toast.error(error, '保存纠错失败');
    console.error('[saynow] failed to save correction', error);
  } finally {
    await hideRecorderOverlay();
  }
}

function subscribeCorrectionEvents() {
  if (!isTauriRuntime || !mounted) return;
  void listen('correction-edit-requested', () => {
    void openCorrectionEditor();
  }).then((unlisten) => {
    unlistenCorrectionEditRequested = unlisten;
  });
  void listen<{
    recognitionRecordId: number;
    rawText: string;
    correctedText: string;
  }>('correction-submit', (event) => {
    void submitCorrection(event.payload);
  }).then((unlisten) => {
    unlistenCorrectionSubmit = unlisten;
  });
  void listen('correction-dismiss', () => {
    void hideRecorderOverlay();
  }).then((unlisten) => {
    unlistenCorrectionDismiss = unlisten;
  });
  void listen('correction-undo', () => {
    clearCorrectionPromptTimer();
    void undoLastInjectedText()
      .catch((error) => {
        toast.error(error, '撤销失败');
        console.error('[saynow] failed to undo last injected text', error);
      })
      .finally(() => {
        void hideRecorderOverlay();
      });
  }).then((unlisten) => {
    unlistenCorrectionUndo = unlisten;
  });
  void listen<{ hovering: boolean }>('correction-hover', (event) => {
    handleCorrectionHover(event.payload.hovering);
  }).then((unlisten) => {
    unlistenCorrectionHover = unlisten;
  });
  void listen('recognition-retry', () => {
    void retryRecognition();
  }).then((unlisten) => {
    unlistenRecognitionRetry = unlisten;
  });
}

function suppressRuntimeHotkeyDomEvent(event: KeyboardEvent) {
  if (!runtimeHotkeyEnabled.value || !isEventPartOfHotkey(event, config.value?.hotkey)) return;
  event.preventDefault();
  event.stopImmediatePropagation();
}

async function createVocabularyTerms(terms: string[]) {
  try {
    vocabulary.value = await addVocabularyTerms(terms);
    toast.success(`已添加 ${terms.length} 个词条`);
  } catch (error) {
    toast.error(error, '添加词条失败');
  }
}

async function removeVocabulary(id: number) {
  try {
    vocabulary.value = await deleteVocabulary(id);
  } catch (error) {
    toast.error(error, '删除词条失败');
  }
}

async function editVocabulary(item: VocabularyItem) {
  try {
    vocabulary.value = await updateVocabulary(item);
    toast.success('词条已更新');
  } catch (error) {
    toast.error(error, '更新词条失败');
  }
}

async function createStyle(item: StylePrompt) {
  try {
    styles.value = await addStylePrompt(item);
    toast.success('提示词已创建');
  } catch (error) {
    toast.error(error, '创建提示词失败');
  }
}

async function saveStyle(item: StylePrompt) {
  try {
    styles.value = await updateStylePrompt(item);
    toast.success('提示词已保存');
  } catch (error) {
    toast.error(error, '保存提示词失败');
  }
}

async function removeStyle(id: number) {
  const target = styles.value.find((style) => style.id === id);
  const confirmed = await confirm({
    title: `删除提示词「${target?.name ?? id}」？`,
    description: '删除后无法恢复。',
    confirmLabel: '删除',
    danger: true,
  });
  if (!confirmed) return;
  try {
    styles.value = await deleteStylePrompt(id);
    toast.success('提示词已删除');
  } catch (error) {
    toast.error(error, '删除提示词失败');
  }
}

async function savePersonalizationPreferenceSettings(preferences: PersonalizationPreferences) {
  try {
    personalizationPreferences.value = await savePersonalizationPreferences(preferences);
  } catch (error) {
    toast.error(error, '保存偏好失败');
  }
}

async function saveLearningEngineSettings(config: LearningEngineConfig) {
  saving.value = true;
  try {
    learningEngineConfig.value = await saveLearningEngineConfig(config);
    toast.success('学习引擎配置已保存');
    scheduleLearningEngineRun();
  } catch (error) {
    toast.error(error, '保存学习引擎失败');
  } finally {
    saving.value = false;
  }
}

async function runLearningEngineNow() {
  saving.value = true;
  clearLearningEngineIdleTimer();
  try {
    learningRules.value = await runLearningEngine(true);
    await refreshLearningData();
    toast.success('学习任务已完成');
  } catch (error) {
    toast.error(error, '学习任务失败');
  } finally {
    saving.value = false;
  }
}

async function changeLearningRuleStatus(payload: { id: number; status: string }) {
  try {
    learningRules.value = await updateLearningRuleStatus(payload.id, payload.status);
  } catch (error) {
    toast.error(error, '更新规则状态失败');
  }
}

async function removeLearningRule(id: number) {
  const confirmed = await confirm({
    title: '删除这条学习规则？',
    description: '删除后规则不再参与识别提示，且无法恢复。',
    confirmLabel: '删除',
    danger: true,
  });
  if (!confirmed) return;
  try {
    learningRules.value = await deleteLearningRule(id);
    toast.success('规则已删除');
  } catch (error) {
    toast.error(error, '删除规则失败');
  }
}

async function clearAllLearningRules() {
  const confirmed = await confirm({
    title: '清空全部学习规则？',
    description: `将删除 ${learningRules.value.length} 条规则，无法恢复。`,
    confirmLabel: '清空',
    danger: true,
  });
  if (!confirmed) return;
  try {
    learningRules.value = await clearLearningRules();
    toast.success('学习规则已清空');
  } catch (error) {
    toast.error(error, '清空规则失败');
  }
}

async function clearAllCorrections() {
  const confirmed = await confirm({
    title: '清空全部纠错记录？',
    description: `将删除 ${correctionRecords.value.length} 条纠错记录，无法恢复。`,
    confirmLabel: '清空',
    danger: true,
  });
  if (!confirmed) return;
  try {
    correctionRecords.value = await clearCorrectionRecords();
    toast.success('纠错记录已清空');
  } catch (error) {
    toast.error(error, '清空纠错记录失败');
  }
}

async function removeRecord(id: number) {
  const confirmed = await confirm({
    title: '删除这条识别记录？',
    description: '删除后无法恢复。',
    confirmLabel: '删除',
    danger: true,
  });
  if (!confirmed) return;
  try {
    records.value = await deleteRecord(id);
    dashboard.value = await getDashboard();
    toast.success('记录已删除');
  } catch (error) {
    toast.error(error, '删除记录失败');
  }
}

async function clearAllRecords() {
  const confirmed = await confirm({
    title: '清空全部识别记录？',
    description: `将删除 ${records.value.length} 条识别记录并重置统计，无法恢复。`,
    confirmLabel: '清空',
    danger: true,
  });
  if (!confirmed) return;
  try {
    records.value = await clearRecords();
    dashboard.value = await getDashboard();
    toast.success('识别记录已清空');
  } catch (error) {
    toast.error(error, '清空记录失败');
  }
}

function scheduleLearningEngineRun() {
  clearLearningEngineIdleTimer();
  const learningConfig = learningEngineConfig.value;
  if (!isTauriRuntime || !learningConfig.enabled || hotkeyRecording.value || busy.value) {
    debugLog('learning engine schedule skipped', {
      enabled: learningConfig.enabled,
      hotkeyRecording: hotkeyRecording.value,
      busy: busy.value,
    });
    return;
  }
  const delayMs = Math.max(5, learningConfig.idleSeconds || 30) * 1000;
  debugLog('learning engine scheduled', {
    delayMs,
    minNewSamples: learningConfig.minNewCorrections,
    mode: learningConfig.runMode,
  });
  learningEngineIdleTimer = window.setTimeout(() => {
    learningEngineIdleTimer = null;
    debugLog('learning engine idle timer fired');
    void runLearningEngine(false)
      .then((rules) => {
        learningRules.value = rules;
        return refreshLearningData();
      })
      .catch((error) => console.error('[saynow] failed to run scheduled learning engine', error));
  }, delayMs);
}

function clearLearningEngineIdleTimer() {
  if (!learningEngineIdleTimer) return;
  window.clearTimeout(learningEngineIdleTimer);
  learningEngineIdleTimer = null;
}

watch(
  () => config.value?.hotkey,
  (hotkey) => {
    void registerRuntimeHotkey(hotkey);
  },
);

onMounted(() => {
  mounted = true;
  window.addEventListener('keydown', suppressRuntimeHotkeyDomEvent, true);
  window.addEventListener('keyup', suppressRuntimeHotkeyDomEvent, true);
  subscribeHotkeyStateEvents();
  subscribeCorrectionEvents();
  void refreshAll()
    .then(() => registerRuntimeHotkey(config.value?.hotkey))
    .catch((error) => console.error('[saynow] failed to initialize app runtime', error));
});

onBeforeUnmount(() => {
  mounted = false;
  clearRecordingGuard();
  clearHotkeyMonitorRetry();
  clearHotkeyStateListenRetry();
  clearCorrectionPromptTimer();
  clearErrorOverlayHideTimer();
  clearLearningEngineIdleTimer();
  recordingSession = null;
  hotkeyRecording.value = false;
  audioRecorder.cancel();
  window.removeEventListener('keydown', suppressRuntimeHotkeyDomEvent, true);
  window.removeEventListener('keyup', suppressRuntimeHotkeyDomEvent, true);
  unlistenHotkeyState?.();
  unlistenCorrectionEditRequested?.();
  unlistenCorrectionSubmit?.();
  unlistenCorrectionDismiss?.();
  unlistenCorrectionUndo?.();
  unlistenCorrectionHover?.();
  unlistenRecognitionRetry?.();
  void setHotkeyMonitor(null).catch(() => undefined);
});
</script>

<template>
  <AppShell :active-page="activePage" @navigate="activePage = $event">
    <HomePage v-if="currentPage === 'home'" :dashboard="dashboard" />
    <ConfigPage
      v-else-if="currentPage === 'config'"
      :config="config"
      :providers="providers"
      :saving="saving"
      @save="persistConfig"
      @save-provider="persistProvider"
      @select-provider="activateProvider"
      @delete-provider="removeProvider"
    />
    <DataPage
      v-else-if="currentPage === 'data'"
      :records="records"
      @delete-record="removeRecord"
      @clear-records="clearAllRecords"
    />
    <PersonalizationPage
      v-else-if="currentPage === 'personalization'"
      :vocabulary="vocabulary"
      :styles="styles"
      :preferences="personalizationPreferences"
      :learning-engine-config="learningEngineConfig"
      :learning-rules="learningRules"
      :correction-records="correctionRecords"
      :saving="saving"
      @add-vocabulary-terms="createVocabularyTerms"
      @update-vocabulary="editVocabulary"
      @delete-vocabulary="removeVocabulary"
      @add-style="createStyle"
      @update-style="saveStyle"
      @delete-style="removeStyle"
      @update-preferences="savePersonalizationPreferenceSettings"
      @update-learning-engine="saveLearningEngineSettings"
      @run-learning-engine="runLearningEngineNow"
      @update-rule-status="changeLearningRuleStatus"
      @delete-rule="removeLearningRule"
      @clear-rules="clearAllLearningRules"
      @clear-corrections="clearAllCorrections"
    />
    <FeedbackPage v-else />
  </AppShell>
  <ToastHost />
  <ConfirmDialog />
</template>
