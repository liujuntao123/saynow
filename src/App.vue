<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
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

const currentPage = computed(() => activePage.value);

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

onMounted(refreshAll);
</script>

<template>
  <AppShell :active-page="activePage" @navigate="activePage = $event">
    <HomePage v-if="currentPage === 'home'" :dashboard="dashboard" :busy="busy" @simulate="runSimulation" />
    <ConfigPage v-else-if="currentPage === 'config'" :config="config" :saving="saving" @save="persistConfig" />
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
