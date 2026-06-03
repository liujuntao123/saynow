<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
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
  saveConfig,
  simulateRecognition,
  updateStylePrompt,
} from './api/tauri';
import { createHoldHotkeyController } from './domain/hotkeyRecorder';
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

const currentPage = computed(() => activePage.value);
const runtimeHotkeyEnabled = computed(() => Boolean(config.value?.hotkey) && !configuringHotkey.value && !saving.value);

async function refreshAll() {
  dashboard.value = await getDashboard();
  config.value = await getConfig();
  records.value = await listRecords();
  vocabulary.value = await listVocabulary();
  styles.value = await listStylePrompts();
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
  hotkeyRecording.value = true;
}

async function stopHotkeyRecording() {
  if (!hotkeyRecording.value) return;
  hotkeyRecording.value = false;
  await runSimulation();
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
    config.value = await saveConfig(nextConfig);
    await refreshAll();
  } finally {
    saving.value = false;
  }
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
  },
);

onMounted(() => {
  window.addEventListener('keydown', handleRuntimeKeyDown, true);
  window.addEventListener('keyup', handleRuntimeKeyUp, true);
  void refreshAll();
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleRuntimeKeyDown, true);
  window.removeEventListener('keyup', handleRuntimeKeyUp, true);
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
