import { invoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  DashboardData,
  PersonalizationPreferences,
  ProviderConfig,
  RecognitionRecord,
  StylePrompt,
  VocabularyItem,
} from '../types';

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const localPreviewRecords: RecognitionRecord[] = [];

let localPreviewConfig: AppConfig = {
  provider: '',
  baseUrl: '',
  model: '',
  apiKeyRef: '',
  hotkey: 'Alt',
};

let localPreviewProviders: ProviderConfig[] = [];
let nextLocalPreviewProviderId = 1;

let localPreviewVocabulary: VocabularyItem[] = [];

let localPreviewStyles: StylePrompt[] = [];
let nextLocalPreviewStyleId = Date.now();

let localPreviewPersonalizationPreferences: PersonalizationPreferences = {
  removeTrailingPeriod: false,
};

function localPreviewStats() {
  const successful = localPreviewRecords.filter((record) => record.status === 'success');
  return {
    totalDurationSeconds: successful.reduce((sum, record) => sum + record.durationSeconds, 0),
    totalRecords: successful.length,
    totalCharacters: successful.reduce((sum, record) => sum + Array.from(record.text).length, 0),
  };
}

export async function getDashboard(): Promise<DashboardData> {
  if (isTauri) return invoke('get_dashboard');
  return {
    stats: localPreviewStats(),
    records: localPreviewRecords,
    platform: {
      supported: false,
      message: '浏览器/WSL 预览模式：托盘、全局快捷键、录音和文本注入已跳过。',
    },
  };
}

export async function getConfig(): Promise<AppConfig> {
  if (isTauri) return invoke('get_config');
  return localPreviewConfig;
}

export async function saveConfig(config: AppConfig): Promise<AppConfig> {
  if (isTauri) return invoke('save_config', { config });
  localPreviewConfig = config;
  return localPreviewConfig;
}

export async function listProviderConfigs(): Promise<ProviderConfig[]> {
  if (isTauri) return invoke('list_provider_configs');
  return localPreviewProviders;
}

export async function saveProviderConfig(provider: ProviderConfig): Promise<ProviderConfig[]> {
  if (isTauri) return invoke('save_provider_config', { provider });
  const saved = { ...provider, id: provider.id || nextLocalPreviewProviderId++ };
  const hasEnabled = saved.enabled || !localPreviewProviders.length;
  localPreviewProviders = localPreviewProviders.filter((item) => item.id !== saved.id);
  localPreviewProviders = [{ ...saved, enabled: hasEnabled }, ...localPreviewProviders].map((item) => ({
    ...item,
    enabled: hasEnabled ? item.id === saved.id : item.enabled,
  }));
  if (hasEnabled) {
    localPreviewConfig = { provider: saved.provider, baseUrl: saved.baseUrl, model: saved.model, apiKeyRef: saved.apiKeyRef, hotkey: localPreviewConfig.hotkey };
  }
  return localPreviewProviders;
}

export async function selectProviderConfig(id: number): Promise<AppConfig> {
  if (isTauri) return invoke('select_provider_config', { id });
  const provider = localPreviewProviders.find((item) => item.id === id);
  if (!provider) return localPreviewConfig;
  localPreviewProviders = localPreviewProviders.map((item) => ({ ...item, enabled: item.id === id }));
  localPreviewConfig = { provider: provider.provider, baseUrl: provider.baseUrl, model: provider.model, apiKeyRef: provider.apiKeyRef, hotkey: localPreviewConfig.hotkey };
  return localPreviewConfig;
}

export async function deleteProviderConfig(id: number): Promise<ProviderConfig[]> {
  if (isTauri) return invoke('delete_provider_config', { id });
  const wasEnabled = localPreviewProviders.some((item) => item.id === id && item.enabled);
  localPreviewProviders = localPreviewProviders.filter((item) => item.id !== id);
  if (wasEnabled && localPreviewProviders.length) {
    localPreviewProviders = localPreviewProviders.map((item, index) => ({ ...item, enabled: index === 0 }));
    const provider = localPreviewProviders[0];
    localPreviewConfig = { provider: provider.provider, baseUrl: provider.baseUrl, model: provider.model, apiKeyRef: provider.apiKeyRef, hotkey: localPreviewConfig.hotkey };
  } else if (wasEnabled) {
    localPreviewConfig = { provider: '', baseUrl: '', model: '', apiKeyRef: '', hotkey: localPreviewConfig.hotkey };
  }
  return localPreviewProviders;
}

export async function listRecords(): Promise<RecognitionRecord[]> {
  if (isTauri) return invoke('list_records');
  return localPreviewRecords;
}

export async function listVocabulary(): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('list_vocabulary');
  return localPreviewVocabulary;
}

export async function addVocabulary(item: VocabularyItem): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('add_vocabulary', { item });
  localPreviewVocabulary = [{ ...item, id: Date.now() }, ...localPreviewVocabulary];
  return localPreviewVocabulary;
}

export async function addVocabularyTerms(terms: string[]): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('add_vocabulary_terms', { terms });
  const items = terms
    .map((term) => term.trim())
    .filter(Boolean)
    .map((term, index) => ({ id: Date.now() + index, term, alias: '', category: '', note: '', enabled: true }));
  localPreviewVocabulary = [...items, ...localPreviewVocabulary];
  return localPreviewVocabulary;
}

export async function deleteVocabulary(id: number): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('delete_vocabulary', { id });
  localPreviewVocabulary = localPreviewVocabulary.filter((item) => item.id !== id);
  return localPreviewVocabulary;
}

export async function listStylePrompts(): Promise<StylePrompt[]> {
  if (isTauri) return invoke('list_style_prompts');
  return localPreviewStyles;
}

export async function addStylePrompt(item: StylePrompt): Promise<StylePrompt[]> {
  if (isTauri) return invoke('add_style_prompt', { item });
  const created = { ...item, id: nextLocalPreviewStyleId++ };
  localPreviewStyles = normalizeStylePrompts([created, ...localPreviewStyles], created.enabled ? created.id : undefined);
  return localPreviewStyles;
}

export async function updateStylePrompt(item: StylePrompt): Promise<StylePrompt[]> {
  if (isTauri) return invoke('update_style_prompt', { item });
  localPreviewStyles = localPreviewStyles.map((style) => (style.id === item.id ? item : style));
  localPreviewStyles = normalizeStylePrompts(localPreviewStyles, item.enabled ? item.id : undefined);
  return localPreviewStyles;
}

export async function deleteStylePrompt(id: number): Promise<StylePrompt[]> {
  if (isTauri) return invoke('delete_style_prompt', { id });
  localPreviewStyles = localPreviewStyles.filter((item) => item.id !== id);
  return localPreviewStyles;
}

export async function getPersonalizationPreferences(): Promise<PersonalizationPreferences> {
  if (isTauri) return invoke('get_personalization_preferences');
  return localPreviewPersonalizationPreferences;
}

export async function savePersonalizationPreferences(
  preferences: PersonalizationPreferences,
): Promise<PersonalizationPreferences> {
  if (isTauri) return invoke('save_personalization_preferences', { preferences });
  localPreviewPersonalizationPreferences = preferences;
  return localPreviewPersonalizationPreferences;
}

function normalizeStylePrompts(items: StylePrompt[], activeId?: number): StylePrompt[] {
  const fallbackActiveId = activeId ?? items.find((item) => item.enabled)?.id;
  return items.map((item) => ({
    ...item,
    enabled: fallbackActiveId !== undefined && item.id === fallbackActiveId,
  }));
}

export async function recognizeAudio(input: {
  audioBase64: string;
  durationSeconds: number;
  mimeType: string;
}): Promise<RecognitionRecord> {
  if (isTauri) return invoke('recognize_audio', { input });
  const errorMessage = '浏览器预览模式不支持真实语音识别，请在桌面端运行。';
  const record: RecognitionRecord = {
    id: Date.now(),
    createdAt: new Date().toISOString(),
    durationSeconds: input.durationSeconds,
    text: '',
    provider: localPreviewConfig.provider,
    model: localPreviewConfig.model,
    status: 'failed',
    errorMessage,
  };
  localPreviewRecords.unshift(record);
  throw new Error(errorMessage);
}

export async function showRecorderOverlayNoActivate(): Promise<void> {
  if (isTauri) return invoke('show_recorder_overlay_no_activate');
}

export async function hideRecorderOverlayWindow(): Promise<void> {
  if (isTauri) return invoke('hide_recorder_overlay');
}

export async function setRecorderOverlayPosition(x: number, y: number): Promise<void> {
  if (isTauri) return invoke('set_recorder_overlay_position', { x, y });
}

export async function setHotkeyMonitor(parts: string[] | null): Promise<void> {
  if (isTauri) return invoke('set_hotkey_monitor', { parts });
}
