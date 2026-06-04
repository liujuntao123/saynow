<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import { emitTo, listen } from '@tauri-apps/api/event';
import { register, unregister } from '@tauri-apps/plugin-global-shortcut';
import { LogicalPosition } from '@tauri-apps/api/dpi';
import { Window, primaryMonitor } from '@tauri-apps/api/window';
import AppShell from './components/AppShell.vue';
import {
  addStylePrompt,
  addVocabularyTerms,
  deleteStylePrompt,
  deleteVocabulary,
  getConfig,
  getDashboard,
  listRecords,
  listStylePrompts,
  listVocabulary,
  recognizeAudio,
  saveConfig,
  setModifierHotkeyMonitor,
  simulateRecognition,
  updateStylePrompt,
} from './api/tauri';
import { createAudioRecorder } from './domain/audioRecorder';
import { createHoldHotkeyController, isModifierOnlyHotkey, toGlobalShortcut, toModifierHotkeyParts } from './domain/hotkeyRecorder';
import ConfigPage from './pages/ConfigPage.vue';
import DataPage from './pages/DataPage.vue';
import FeedbackPage from './pages/FeedbackPage.vue';
import HomePage from './pages/HomePage.vue';
import type { AppConfig, DashboardData, RecognitionRecord, StylePrompt, VocabularyItem } from './types';

const activePage = ref('home');
const dashboard = ref<DashboardData | null>(null);
const config = ref<AppConfig | null>(null);
const records = ref<RecognitionRecord[]>([]);
const vocabulary = ref<VocabularyItem[]>([]);
const styles = ref<StylePrompt[]>([]);
const busy = ref(false);
const saving = ref(false);
const hotkeyRecording = ref(false);
const configuringHotkey = ref(false);
const holdHotkeyController = shallowRef<ReturnType<typeof createHoldHotkeyController> | null>(null);
const audioRecorder = createAudioRecorder();
const registeredGlobalShortcut = ref<string | null>(null);
const recorderWindow = shallowRef<Window | null>(null);
let recordingStartPromise: Promise<void> | null = null;
let unlistenModifierHotkey: (() => void) | null = null;

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
  records.value = await listRecords();
  vocabulary.value = await listVocabulary();
  styles.value = await listStylePrompts();
  debugLog('app data refreshed', {
    hotkey: config.value.hotkey,
    records: records.value.length,
    vocabulary: vocabulary.value.length,
    styles: styles.value.length,
  });
}

async function runSimulation() {
  busy.value = true;
  try {
    await simulateRecognition();
    await refreshAll();
  } finally {
    busy.value = false;
  }
}

function startHotkeyRecording() {
  if (busy.value || hotkeyRecording.value) return;
  debugLog('hotkey pressed; starting recording');
  hotkeyRecording.value = true;
  recordingStartPromise = beginRecording();
}

async function stopHotkeyRecording() {
  if (!hotkeyRecording.value) return;
  debugLog('hotkey released; stopping recording');
  hotkeyRecording.value = false;
  await finishRecording();
}

function handleRuntimeKeyDown(event: KeyboardEvent) {
  if (!runtimeHotkeyEnabled.value) return;
  holdHotkeyController.value?.handleKeyDown(event);
}

function handleRuntimeKeyUp(event: KeyboardEvent) {
  if (!runtimeHotkeyEnabled.value) return;
  holdHotkeyController.value?.handleKeyUp(event);
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

async function beginRecording() {
  try {
    debugLog('showing recorder overlay');
    await showRecorderOverlay('recording');
    debugLog('requesting microphone stream');
    await audioRecorder.start();
    debugLog('microphone recording started');
  } catch (error) {
    hotkeyRecording.value = false;
    await hideRecorderOverlay();
    await refreshAll();
    console.error('[saynow] failed to start recording', error);
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
  }
  await hideRecorderOverlay();
}

async function showRecorderOverlay(state: 'recording' | 'processing' | 'error') {
  if (!isTauriRuntime) return;
  const overlay = await getRecorderWindow();
  await emitTo('recorder', 'recorder-state', { state });
  await positionRecorderOverlay(overlay);
  await overlay.show();
}

async function hideRecorderOverlay() {
  if (!isTauriRuntime) return;
  const overlay = await getRecorderWindow();
  await overlay.hide();
}

async function getRecorderWindow() {
  if (!recorderWindow.value) {
    recorderWindow.value = (await Window.getByLabel('recorder')) ?? new Window('recorder');
  }
  return recorderWindow.value;
}

async function positionRecorderOverlay(overlay: Window) {
  const monitor = await primaryMonitor().catch(() => null);
  if (!monitor) return;
  const width = 360;
  const height = 72;
  const workArea = monitor.workArea;
  const x = workArea.position.x / monitor.scaleFactor + (workArea.size.width / monitor.scaleFactor - width) / 2;
  const y = workArea.position.y / monitor.scaleFactor + workArea.size.height / monitor.scaleFactor - height - 24;
  await overlay.setPosition(new LogicalPosition(Math.round(x), Math.round(y))).catch(() => undefined);
}

async function registerRuntimeHotkey(hotkey?: string) {
  if (!isTauriRuntime) return;
  if (registeredGlobalShortcut.value) {
    debugLog('unregistering global shortcut', { shortcut: registeredGlobalShortcut.value });
    await unregister(registeredGlobalShortcut.value).catch(() => undefined);
    registeredGlobalShortcut.value = null;
  }
  await setModifierHotkeyMonitor(null).catch((error) => console.error('[saynow] failed to stop modifier hotkey monitor', error));
  if (!hotkey || configuringHotkey.value) {
    debugLog('runtime hotkey disabled', { hotkey, configuringHotkey: configuringHotkey.value });
    return;
  }

  if (isModifierOnlyHotkey(hotkey)) {
    const parts = toModifierHotkeyParts(hotkey);
    debugLog('using native modifier hotkey monitor', { hotkey, parts });
    await setModifierHotkeyMonitor(parts);
    return;
  }

  const shortcut = toGlobalShortcut(hotkey);
  debugLog('registering global shortcut', { hotkey, shortcut });
  await register(shortcut, (event) => {
    debugLog('global shortcut event', event);
    if (event.state === 'Pressed') startHotkeyRecording();
    if (event.state === 'Released') {
      void stopHotkeyRecording();
    }
  });
  registeredGlobalShortcut.value = shortcut;
  debugLog('global shortcut registered', { shortcut });
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
    void registerRuntimeHotkey(undefined);
  } else {
    void registerRuntimeHotkey(config.value?.hotkey);
  }
});

onMounted(() => {
  window.addEventListener('keydown', handleRuntimeKeyDown, true);
  window.addEventListener('keyup', handleRuntimeKeyUp, true);
  if (isTauriRuntime) {
    void listen<{ state: 'Pressed' | 'Released' }>('modifier-hotkey-state', (event) => {
      debugLog('modifier hotkey event', event.payload);
      if (event.payload.state === 'Pressed') startHotkeyRecording();
      if (event.payload.state === 'Released') {
        void stopHotkeyRecording();
      }
    }).then((unlisten) => {
      unlistenModifierHotkey = unlisten;
    });
  }
  void refreshAll()
    .then(() => registerRuntimeHotkey(config.value?.hotkey))
    .catch((error) => console.error('[saynow] failed to initialize app runtime', error));
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleRuntimeKeyDown, true);
  window.removeEventListener('keyup', handleRuntimeKeyUp, true);
  unlistenModifierHotkey?.();
  void setModifierHotkeyMonitor(null).catch(() => undefined);
  if (registeredGlobalShortcut.value) {
    void unregister(registeredGlobalShortcut.value);
  }
});
</script>

<template>
  <AppShell :active-page="activePage" @navigate="activePage = $event">
    <HomePage v-if="currentPage === 'home'" :dashboard="dashboard" :busy="busy" :recording="hotkeyRecording" @simulate="runSimulation" />
    <ConfigPage
      v-else-if="currentPage === 'config'"
      :config="config"
      :saving="saving"
      @save="persistConfig"
      @hotkey-recording-change="configuringHotkey = $event"
    />
    <DataPage
      v-else-if="currentPage === 'data'"
      :records="records"
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
