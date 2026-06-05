import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, DashboardData, ProviderConfig, RecognitionRecord, StylePrompt, VocabularyItem } from '../types';

const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const demoRecords: RecognitionRecord[] = [
  {
    id: 3,
    createdAt: '2026-06-02T10:10:00Z',
    durationSeconds: 18,
    text: '请把 Qwen3.5-Omni 的供应商配置放到默认模板里。',
    provider: 'Qwen',
    model: 'qwen3.5-omni-plus',
    status: 'success',
  },
  {
    id: 2,
    createdAt: '2026-06-02T09:42:00Z',
    durationSeconds: 11,
    text: '这是一次模拟语音识别结果，已按照书面语风格整理。',
    provider: 'MiMo',
    model: 'mimo-v2.5',
    status: 'success',
  },
];

let demoConfig: AppConfig = {
  provider: 'MiMo',
  baseUrl: 'https://api.xiaomimimo.com/v1',
  model: 'mimo-v2.5',
  apiKeyRef: 'credential-manager:mimo',
  hotkey: 'Ctrl+Space',
};

let demoProviders: ProviderConfig[] = [
  {
    id: 1,
    provider: 'MiMo',
    baseUrl: 'https://api.xiaomimimo.com/v1',
    model: 'mimo-v2.5',
    apiKeyRef: 'credential-manager:mimo',
    enabled: true,
  },
  {
    id: 2,
    provider: 'Qwen',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    model: 'qwen3.5-omni-plus',
    apiKeyRef: 'credential-manager:qwen',
    enabled: false,
  },
];
let nextDemoProviderId = 3;

let demoVocabulary: VocabularyItem[] = [
  { id: 1, term: 'Kunlun', alias: '昆仑', category: '项目', note: '内部项目名', enabled: true },
  { id: 2, term: 'Qwen3.5-Omni', alias: '通义千问 Omni', category: '模型', note: '', enabled: true },
];

let demoStyles: StylePrompt[] = [
  { id: 1, name: '书面语', prompt: '将口语整理为简洁书面语，保留原意。', enabled: true },
];
let nextDemoStyleId = Date.now();

function demoStats() {
  const successful = demoRecords.filter((record) => record.status === 'success');
  return {
    totalDurationSeconds: successful.reduce((sum, record) => sum + record.durationSeconds, 0),
    totalRecords: successful.length,
    totalCharacters: successful.reduce((sum, record) => sum + Array.from(record.text).length, 0),
  };
}

export async function getDashboard(): Promise<DashboardData> {
  if (isTauri) return invoke('get_dashboard');
  return {
    stats: demoStats(),
    records: demoRecords,
    platform: {
      supported: false,
      message: '浏览器/WSL 预览模式：托盘、全局快捷键、录音和文本注入已跳过。',
    },
  };
}

export async function getConfig(): Promise<AppConfig> {
  if (isTauri) return invoke('get_config');
  return demoConfig;
}

export async function saveConfig(config: AppConfig): Promise<AppConfig> {
  if (isTauri) return invoke('save_config', { config });
  demoConfig = config;
  return demoConfig;
}

export async function listProviderConfigs(): Promise<ProviderConfig[]> {
  if (isTauri) return invoke('list_provider_configs');
  return demoProviders;
}

export async function saveProviderConfig(provider: ProviderConfig): Promise<ProviderConfig[]> {
  if (isTauri) return invoke('save_provider_config', { provider });
  const saved = { ...provider, id: provider.id || nextDemoProviderId++ };
  const hasEnabled = saved.enabled || !demoProviders.length;
  demoProviders = demoProviders.filter((item) => item.id !== saved.id);
  demoProviders = [{ ...saved, enabled: hasEnabled }, ...demoProviders].map((item) => ({
    ...item,
    enabled: hasEnabled ? item.id === saved.id : item.enabled,
  }));
  if (hasEnabled) {
    demoConfig = { provider: saved.provider, baseUrl: saved.baseUrl, model: saved.model, apiKeyRef: saved.apiKeyRef, hotkey: demoConfig.hotkey };
  }
  return demoProviders;
}

export async function selectProviderConfig(id: number): Promise<AppConfig> {
  if (isTauri) return invoke('select_provider_config', { id });
  const provider = demoProviders.find((item) => item.id === id);
  if (!provider) return demoConfig;
  demoProviders = demoProviders.map((item) => ({ ...item, enabled: item.id === id }));
  demoConfig = { provider: provider.provider, baseUrl: provider.baseUrl, model: provider.model, apiKeyRef: provider.apiKeyRef, hotkey: demoConfig.hotkey };
  return demoConfig;
}

export async function deleteProviderConfig(id: number): Promise<ProviderConfig[]> {
  if (isTauri) return invoke('delete_provider_config', { id });
  const wasEnabled = demoProviders.some((item) => item.id === id && item.enabled);
  demoProviders = demoProviders.filter((item) => item.id !== id);
  if (wasEnabled && demoProviders.length) {
    demoProviders = demoProviders.map((item, index) => ({ ...item, enabled: index === 0 }));
    const provider = demoProviders[0];
    demoConfig = { provider: provider.provider, baseUrl: provider.baseUrl, model: provider.model, apiKeyRef: provider.apiKeyRef, hotkey: demoConfig.hotkey };
  }
  return demoProviders;
}

export async function listRecords(): Promise<RecognitionRecord[]> {
  if (isTauri) return invoke('list_records');
  return demoRecords;
}

export async function listVocabulary(): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('list_vocabulary');
  return demoVocabulary;
}

export async function addVocabulary(item: VocabularyItem): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('add_vocabulary', { item });
  demoVocabulary = [{ ...item, id: Date.now() }, ...demoVocabulary];
  return demoVocabulary;
}

export async function addVocabularyTerms(terms: string[]): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('add_vocabulary_terms', { terms });
  const items = terms
    .map((term) => term.trim())
    .filter(Boolean)
    .map((term, index) => ({ id: Date.now() + index, term, alias: '', category: '', note: '', enabled: true }));
  demoVocabulary = [...items, ...demoVocabulary];
  return demoVocabulary;
}

export async function deleteVocabulary(id: number): Promise<VocabularyItem[]> {
  if (isTauri) return invoke('delete_vocabulary', { id });
  demoVocabulary = demoVocabulary.filter((item) => item.id !== id);
  return demoVocabulary;
}

export async function listStylePrompts(): Promise<StylePrompt[]> {
  if (isTauri) return invoke('list_style_prompts');
  return demoStyles;
}

export async function addStylePrompt(item: StylePrompt): Promise<StylePrompt[]> {
  if (isTauri) return invoke('add_style_prompt', { item });
  const created = { ...item, id: nextDemoStyleId++ };
  demoStyles = normalizeStylePrompts([created, ...demoStyles], created.enabled ? created.id : undefined);
  return demoStyles;
}

export async function updateStylePrompt(item: StylePrompt): Promise<StylePrompt[]> {
  if (isTauri) return invoke('update_style_prompt', { item });
  demoStyles = demoStyles.map((style) => (style.id === item.id ? item : style));
  demoStyles = normalizeStylePrompts(demoStyles, item.enabled ? item.id : undefined);
  return demoStyles;
}

export async function deleteStylePrompt(id: number): Promise<StylePrompt[]> {
  if (isTauri) return invoke('delete_style_prompt', { id });
  demoStyles = demoStyles.filter((item) => item.id !== id);
  return demoStyles;
}

function normalizeStylePrompts(items: StylePrompt[], activeId?: number): StylePrompt[] {
  const fallbackActiveId = activeId ?? items.find((item) => item.enabled)?.id;
  return items.map((item) => ({
    ...item,
    enabled: fallbackActiveId !== undefined && item.id === fallbackActiveId,
  }));
}

export async function simulateRecognition(): Promise<RecognitionRecord> {
  if (isTauri) return invoke('simulate_recognition');
  const record: RecognitionRecord = {
    id: Date.now(),
    createdAt: new Date().toISOString(),
    durationSeconds: 6,
    text: '这是一次模拟语音识别结果，已自动填充到当前输入窗口。',
    provider: demoConfig.provider,
    model: demoConfig.model,
    status: 'success',
  };
  demoRecords.unshift(record);
  return record;
}

export async function recognizeAudio(input: {
  audioBase64: string;
  durationSeconds: number;
  mimeType: string;
}): Promise<RecognitionRecord> {
  if (isTauri) return invoke('recognize_audio', { input });
  const record: RecognitionRecord = {
    id: Date.now(),
    createdAt: new Date().toISOString(),
    durationSeconds: input.durationSeconds,
    text: '这是一次本地预览识别结果。桌面端会发送真实录音到模型 API。',
    provider: demoConfig.provider,
    model: demoConfig.model,
    status: 'success',
  };
  demoRecords.unshift(record);
  return record;
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
