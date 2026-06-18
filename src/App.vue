<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { emitTo, listen } from '@tauri-apps/api/event';
import { currentMonitor, cursorPosition, monitorFromPoint, primaryMonitor, type Monitor } from '@tauri-apps/api/window';
import AppShell from './components/AppShell.vue';
import {
  addStylePrompt,
  addVocabularyTerms,
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
  updateStylePrompt,
  writeRuntimeLog,
} from './api/tauri';
import { createAudioRecorder } from './domain/audioRecorder';
import { isEventPartOfHotkey, toHotkeyParts } from './domain/hotkeyRecorder';
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
const configuringHotkey = ref(false);
const audioRecorder = createAudioRecorder();
let recordingSession: RecordingSession | null = null;
let unlistenHotkeyState: (() => void) | null = null;
let recordingGuardTimer: ReturnType<typeof window.setTimeout> | null = null;
let hotkeyMonitorRetryTimer: ReturnType<typeof window.setTimeout> | null = null;
let hotkeyStateListenRetryTimer: ReturnType<typeof window.setTimeout> | null = null;
let correctionPromptTimer: ReturnType<typeof window.setTimeout> | null = null;
let learningEngineIdleTimer: ReturnType<typeof window.setTimeout> | null = null;
let unlistenCorrectionEditRequested: (() => void) | null = null;
let unlistenCorrectionSubmit: (() => void) | null = null;
let unlistenCorrectionDismiss: (() => void) | null = null;
let unlistenCorrectionUndo: (() => void) | null = null;
let mounted = false;
const RECORDER_OVERLAY_COMPACT_SIZE = { width: 660, height: 54 };
const RECORDER_OVERLAY_EDITOR_SIZE = { width: 640, height: 152 };
const MAX_HOTKEY_RECORDING_MS = 120_000;
const HOTKEY_MONITOR_RETRY_MS = 1500;
const HOTKEY_STATE_LISTEN_RETRY_MS = 1500;
const CORRECTION_PROMPT_TIMEOUT_MS = 4000;

type RecordingPhase = 'starting' | 'recording' | 'stopping';

interface RecordingSession {
  id: number;
  phase: RecordingPhase;
  startedAt: number;
  startPromise: Promise<void>;
  stopRequested: boolean;
}

const currentPage = computed(() => activePage.value);
const runtimeHotkeyEnabled = computed(() => Boolean(config.value?.hotkey) && !configuringHotkey.value && !saving.value);
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

async function refreshAll() {
  debugLog('refreshing app data');
  dashboard.value = await getDashboard();
  config.value = await getConfig();
  providers.value = await listProviderConfigs();
  records.value = await listRecords();
  correctionRecords.value = await listCorrectionRecords();
  vocabulary.value = await listVocabulary();
  styles.value = await listStylePrompts();
  personalizationPreferences.value = await getPersonalizationPreferences();
  learningEngineConfig.value = await getLearningEngineConfig();
  learningRules.value = await listLearningRules();
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
    await refreshAll();
    await registerRuntimeHotkey(config.value.hotkey);
    debugLog('config saved');
  } finally {
    saving.value = false;
  }
}

async function persistProvider(nextProvider: ProviderConfig) {
  saving.value = true;
  try {
    debugLog('saving provider config', { provider: nextProvider.provider, model: nextProvider.model, enabled: nextProvider.enabled });
    await saveProviderConfig(nextProvider);
    await refreshAll();
    debugLog('provider config saved');
  } finally {
    saving.value = false;
  }
}

async function activateProvider(id: number) {
  saving.value = true;
  try {
    debugLog('selecting provider config', { id });
    config.value = await selectProviderConfig(id);
    await refreshAll();
    await registerRuntimeHotkey(config.value.hotkey);
    debugLog('provider config selected');
  } finally {
    saving.value = false;
  }
}

async function removeProvider(id: number) {
  saving.value = true;
  try {
    debugLog('deleting provider config', { id });
    providers.value = await deleteProviderConfig(id);
    await refreshAll();
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
    debugLog('requesting microphone stream', { sessionId: session.id });
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
      await hideRecorderOverlay();
      await refreshAll();
    }
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
      await refreshAll();
      if (record.status === 'success' && record.text.trim()) {
        shouldScheduleLearning = true;
        await showCorrectionPrompt(record);
      }
    }
  } catch (error) {
    await showRecorderOverlay('error');
    await refreshAll();
    console.error('[saynow] failed to finish recognition flow', error);
    window.setTimeout(() => {
      void hideRecorderOverlay();
    }, 1200);
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
  await resizeRecorderOverlay(RECORDER_OVERLAY_COMPACT_SIZE);
  await emitTo('recorder', 'recorder-state', { state });
  await positionRecorderOverlay(RECORDER_OVERLAY_COMPACT_SIZE);
  await showRecorderOverlayNoActivate();
  debugLog('recorder overlay shown', {
    state,
    elapsedMs: Math.round(performance.now() - startedAt),
  });
}

async function hideRecorderOverlay() {
  if (!isTauriRuntime) return;
  const startedAt = performance.now();
  clearCorrectionPromptTimer();
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
  if (!hotkey || configuringHotkey.value) {
    debugLog('runtime hotkey disabled', { hotkey, configuringHotkey: configuringHotkey.value });
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
  correctionPromptTimer = window.setTimeout(() => {
    void hideRecorderOverlay();
  }, CORRECTION_PROMPT_TIMEOUT_MS);
}

function clearCorrectionPromptTimer() {
  if (!correctionPromptTimer) return;
  window.clearTimeout(correctionPromptTimer);
  correctionPromptTimer = null;
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
    await refreshAll();
    scheduleLearningEngineRun();
  } catch (error) {
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
      .catch((error) => console.error('[saynow] failed to undo last injected text', error))
      .finally(() => {
        void hideRecorderOverlay();
      });
  }).then((unlisten) => {
    unlistenCorrectionUndo = unlisten;
  });
}

function suppressRuntimeHotkeyDomEvent(event: KeyboardEvent) {
  if (!runtimeHotkeyEnabled.value || !isEventPartOfHotkey(event, config.value?.hotkey)) return;
  event.preventDefault();
  event.stopImmediatePropagation();
}

async function createVocabularyTerms(terms: string[]) {
  vocabulary.value = await addVocabularyTerms(terms);
  await refreshAll();
}

async function removeVocabulary(id: number) {
  vocabulary.value = await deleteVocabulary(id);
  await refreshAll();
}

async function createStyle(item: StylePrompt) {
  styles.value = await addStylePrompt(item);
  await refreshAll();
}

async function saveStyle(item: StylePrompt) {
  styles.value = await updateStylePrompt(item);
  await refreshAll();
}

async function removeStyle(id: number) {
  styles.value = await deleteStylePrompt(id);
  await refreshAll();
}

async function savePersonalizationPreferenceSettings(preferences: PersonalizationPreferences) {
  personalizationPreferences.value = await savePersonalizationPreferences(preferences);
  await refreshAll();
}

async function saveLearningEngineSettings(config: LearningEngineConfig) {
  saving.value = true;
  try {
    learningEngineConfig.value = await saveLearningEngineConfig(config);
    await refreshAll();
    scheduleLearningEngineRun();
  } finally {
    saving.value = false;
  }
}

async function runLearningEngineNow() {
  saving.value = true;
  clearLearningEngineIdleTimer();
  try {
    await runLearningEngine(true);
    await refreshAll();
  } finally {
    saving.value = false;
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
      .then(() => refreshAll())
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

watch(configuringHotkey, (recording) => {
  if (recording) {
    void registerRuntimeHotkey(undefined);
  } else {
    void registerRuntimeHotkey(config.value?.hotkey);
  }
});

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
      @hotkey-recording-change="configuringHotkey = $event"
    />
    <DataPage
      v-else-if="currentPage === 'data'"
      :records="records"
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
      @delete-vocabulary="removeVocabulary"
      @add-style="createStyle"
      @update-style="saveStyle"
      @delete-style="removeStyle"
      @update-preferences="savePersonalizationPreferenceSettings"
      @update-learning-engine="saveLearningEngineSettings"
      @run-learning-engine="runLearningEngineNow"
    />
    <FeedbackPage v-else />
  </AppShell>
</template>
