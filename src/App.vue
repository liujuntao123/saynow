<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
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
  hideRecorderOverlayWindow,
  listProviderConfigs,
  listRecords,
  listStylePrompts,
  listVocabulary,
  recognizeAudio,
  saveConfig,
  saveProviderConfig,
  deleteProviderConfig,
  selectProviderConfig,
  setHotkeyMonitor,
  setRecorderOverlayPosition,
  showRecorderOverlayNoActivate,
  updateStylePrompt,
} from './api/tauri';
import { createAudioRecorder } from './domain/audioRecorder';
import { createHoldHotkeyController, toHotkeyParts } from './domain/hotkeyRecorder';
import { calculateRecorderOverlayPosition } from './domain/recorderOverlayPosition';
import ConfigPage from './pages/ConfigPage.vue';
import DataPage from './pages/DataPage.vue';
import FeedbackPage from './pages/FeedbackPage.vue';
import HomePage from './pages/HomePage.vue';
import PersonalizationPage from './pages/PersonalizationPage.vue';
import type { AppConfig, DashboardData, ProviderConfig, RecognitionRecord, StylePrompt, VocabularyItem } from './types';

const activePage = ref('home');
const dashboard = ref<DashboardData | null>(null);
const config = ref<AppConfig | null>(null);
const providers = ref<ProviderConfig[]>([]);
const records = ref<RecognitionRecord[]>([]);
const vocabulary = ref<VocabularyItem[]>([]);
const styles = ref<StylePrompt[]>([]);
const busy = ref(false);
const saving = ref(false);
const hotkeyRecording = ref(false);
const configuringHotkey = ref(false);
const holdHotkeyController = shallowRef<ReturnType<typeof createHoldHotkeyController> | null>(null);
const audioRecorder = createAudioRecorder();
let recordingStartPromise: Promise<void> | null = null;
let unlistenModifierHotkey: (() => void) | null = null;
let recordingGuardTimer: ReturnType<typeof window.setTimeout> | null = null;
let hotkeyMonitorRetryTimer: ReturnType<typeof window.setTimeout> | null = null;
let modifierHotkeyListenRetryTimer: ReturnType<typeof window.setTimeout> | null = null;
let mounted = false;
const RECORDER_OVERLAY_SIZE = { width: 760, height: 52 };
const MAX_HOTKEY_RECORDING_MS = 120_000;
const HOTKEY_MONITOR_RETRY_MS = 1500;
const MODIFIER_EVENT_LISTEN_RETRY_MS = 1500;

const currentPage = computed(() => activePage.value);
const runtimeHotkeyEnabled = computed(() => Boolean(config.value?.hotkey) && !configuringHotkey.value && !saving.value);
const isTauriRuntime = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

function debugLog(message: string, details?: unknown) {
  if (details === undefined) {
    console.info(`[saynow] ${message}`);
  } else {
    console.info(`[saynow] ${message}`, details);
  }
}

async function refreshAll() {
  debugLog('refreshing app data');
  dashboard.value = await getDashboard();
  config.value = await getConfig();
  providers.value = await listProviderConfigs();
  records.value = await listRecords();
  vocabulary.value = await listVocabulary();
  styles.value = await listStylePrompts();
  debugLog('app data refreshed', {
    hotkey: config.value.hotkey,
    providers: providers.value.length,
    records: records.value.length,
    vocabulary: vocabulary.value.length,
    styles: styles.value.length,
  });
}

function startHotkeyRecording() {
  if (busy.value || hotkeyRecording.value) return;
  debugLog('hotkey pressed; starting recording');
  hotkeyRecording.value = true;
  armRecordingGuard();
  recordingStartPromise = beginRecording();
}

async function stopHotkeyRecording(reason = 'hotkey released') {
  if (!hotkeyRecording.value) return;
  debugLog(`${reason}; stopping recording`);
  hotkeyRecording.value = false;
  clearRecordingGuard();
  await finishRecording();
}

function handleRuntimeKeyDown(event: KeyboardEvent) {
  if (isTauriRuntime) return;
  if (!runtimeHotkeyEnabled.value) return;
  holdHotkeyController.value?.handleKeyDown(event);
}

function handleRuntimeKeyUp(event: KeyboardEvent) {
  if (isTauriRuntime) return;
  if (!runtimeHotkeyEnabled.value) return;
  holdHotkeyController.value?.handleKeyUp(event);
}

function handleRuntimeReset() {
  if (isTauriRuntime) return;
  holdHotkeyController.value?.cancel();
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

async function beginRecording() {
  try {
    debugLog('showing recorder overlay');
    await showRecorderOverlay('recording');
    debugLog('requesting microphone stream');
    await audioRecorder.start();
    debugLog('microphone recording started');
  } catch (error) {
    hotkeyRecording.value = false;
    clearRecordingGuard();
    await hideRecorderOverlay();
    await refreshAll();
    console.error('[saynow] failed to start recording', error);
    void resetNativeHotkeyMonitor('recording start failed');
  }
}

async function finishRecording() {
  busy.value = true;
  try {
    await recordingStartPromise;
    recordingStartPromise = null;
    await showRecorderOverlay('processing');
    const audio = await audioRecorder.stop();
    debugLog('microphone recording stopped', audio ? { durationSeconds: audio.durationSeconds, mimeType: audio.mimeType, bytesBase64: audio.audioBase64.length } : { empty: true });
    if (audio) {
      const record = await recognizeAudio(audio);
      debugLog('recognition finished', { id: record.id, status: record.status, textLength: record.text.length, error: record.errorMessage });
      await refreshAll();
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
    recordingStartPromise = null;
  }
  await hideRecorderOverlay();
}

async function showRecorderOverlay(state: 'recording' | 'processing' | 'error') {
  if (!isTauriRuntime) return;
  await emitTo('recorder', 'recorder-state', { state });
  await positionRecorderOverlay();
  await showRecorderOverlayNoActivate();
}

async function hideRecorderOverlay() {
  if (!isTauriRuntime) return;
  await hideRecorderOverlayWindow();
}

async function positionRecorderOverlay() {
  const monitor = await resolveRecorderMonitor();
  if (!monitor) return;
  const position = calculateRecorderOverlayPosition({
    workArea: monitor.workArea,
    scaleFactor: monitor.scaleFactor,
    overlaySize: RECORDER_OVERLAY_SIZE,
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

function subscribeModifierHotkeyEvents() {
  if (!isTauriRuntime || !mounted) return;
  clearModifierHotkeyListenRetry();
  void listen<{ state: 'Pressed' | 'Released' }>('modifier-hotkey-state', (event) => {
    debugLog('modifier hotkey event', event.payload);
    if (event.payload.state === 'Pressed') startHotkeyRecording();
    if (event.payload.state === 'Released') {
      void stopHotkeyRecording();
    }
  })
    .then((unlisten) => {
      unlistenModifierHotkey = unlisten;
    })
    .catch((error) => {
      console.error('[saynow] failed to listen modifier hotkey state', error);
      scheduleModifierHotkeyListenRetry();
    });
}

function scheduleModifierHotkeyListenRetry() {
  if (!mounted || modifierHotkeyListenRetryTimer) return;
  modifierHotkeyListenRetryTimer = window.setTimeout(() => {
    modifierHotkeyListenRetryTimer = null;
    subscribeModifierHotkeyEvents();
  }, MODIFIER_EVENT_LISTEN_RETRY_MS);
}

function clearModifierHotkeyListenRetry() {
  if (!modifierHotkeyListenRetryTimer) return;
  window.clearTimeout(modifierHotkeyListenRetryTimer);
  modifierHotkeyListenRetryTimer = null;
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

watch(
  () => config.value?.hotkey,
  (hotkey) => {
    holdHotkeyController.value = hotkey
      ? createHoldHotkeyController(hotkey, {
          onStart: startHotkeyRecording,
          onStop: () => {
            void stopHotkeyRecording();
          },
        })
      : null;
    void registerRuntimeHotkey(hotkey);
  },
);

watch(configuringHotkey, (recording) => {
  if (recording) {
    handleRuntimeReset();
    void registerRuntimeHotkey(undefined);
  } else {
    void registerRuntimeHotkey(config.value?.hotkey);
  }
});

onMounted(() => {
  mounted = true;
  window.addEventListener('keydown', handleRuntimeKeyDown, true);
  window.addEventListener('keyup', handleRuntimeKeyUp, true);
  window.addEventListener('blur', handleRuntimeReset);
  document.addEventListener('visibilitychange', handleRuntimeReset);
  subscribeModifierHotkeyEvents();
  void refreshAll()
    .then(() => registerRuntimeHotkey(config.value?.hotkey))
    .catch((error) => console.error('[saynow] failed to initialize app runtime', error));
});

onBeforeUnmount(() => {
  mounted = false;
  window.removeEventListener('keydown', handleRuntimeKeyDown, true);
  window.removeEventListener('keyup', handleRuntimeKeyUp, true);
  window.removeEventListener('blur', handleRuntimeReset);
  document.removeEventListener('visibilitychange', handleRuntimeReset);
  clearRecordingGuard();
  clearHotkeyMonitorRetry();
  clearModifierHotkeyListenRetry();
  handleRuntimeReset();
  unlistenModifierHotkey?.();
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
      @add-vocabulary-terms="createVocabularyTerms"
      @delete-vocabulary="removeVocabulary"
      @add-style="createStyle"
      @update-style="saveStyle"
      @delete-style="removeStyle"
    />
    <FeedbackPage v-else />
  </AppShell>
</template>
