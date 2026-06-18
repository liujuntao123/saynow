import { invoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  CorrectionRecord,
  DashboardData,
  LearningRule,
  LearningEngineConfig,
  PersonalizationPreferences,
  ProviderConfig,
  RecordedAudio,
  RecognitionRecord,
  SaveCorrectionInput,
  StylePrompt,
  VocabularyItem,
} from '../types';

export async function getDashboard(): Promise<DashboardData> {
  return invoke('get_dashboard');
}

export async function getConfig(): Promise<AppConfig> {
  return invoke('get_config');
}

export async function saveConfig(config: AppConfig): Promise<AppConfig> {
  return invoke('save_config', { config });
}

export async function listProviderConfigs(): Promise<ProviderConfig[]> {
  return invoke('list_provider_configs');
}

export async function saveProviderConfig(provider: ProviderConfig): Promise<ProviderConfig[]> {
  return invoke('save_provider_config', { provider });
}

export async function selectProviderConfig(id: number): Promise<AppConfig> {
  return invoke('select_provider_config', { id });
}

export async function deleteProviderConfig(id: number): Promise<ProviderConfig[]> {
  return invoke('delete_provider_config', { id });
}

export async function listRecords(): Promise<RecognitionRecord[]> {
  return invoke('list_records');
}

export async function listCorrectionRecords(): Promise<CorrectionRecord[]> {
  return invoke('list_correction_records');
}

export async function saveCorrection(input: SaveCorrectionInput): Promise<CorrectionRecord> {
  return invoke('save_correction', { input });
}

export async function listLearningRules(): Promise<LearningRule[]> {
  return invoke('list_learning_rules');
}

export async function refreshLearningRules(): Promise<LearningRule[]> {
  return invoke('refresh_learning_rules');
}

export async function runLearningEngine(force: boolean): Promise<LearningRule[]> {
  return invoke('run_learning_engine', { force });
}

export async function listVocabulary(): Promise<VocabularyItem[]> {
  return invoke('list_vocabulary');
}

export async function addVocabulary(item: VocabularyItem): Promise<VocabularyItem[]> {
  return invoke('add_vocabulary', { item });
}

export async function addVocabularyTerms(terms: string[]): Promise<VocabularyItem[]> {
  return invoke('add_vocabulary_terms', { terms });
}

export async function deleteVocabulary(id: number): Promise<VocabularyItem[]> {
  return invoke('delete_vocabulary', { id });
}

export async function listStylePrompts(): Promise<StylePrompt[]> {
  return invoke('list_style_prompts');
}

export async function addStylePrompt(item: StylePrompt): Promise<StylePrompt[]> {
  return invoke('add_style_prompt', { item });
}

export async function updateStylePrompt(item: StylePrompt): Promise<StylePrompt[]> {
  return invoke('update_style_prompt', { item });
}

export async function deleteStylePrompt(id: number): Promise<StylePrompt[]> {
  return invoke('delete_style_prompt', { id });
}

export async function getPersonalizationPreferences(): Promise<PersonalizationPreferences> {
  return invoke('get_personalization_preferences');
}

export async function savePersonalizationPreferences(
  preferences: PersonalizationPreferences,
): Promise<PersonalizationPreferences> {
  return invoke('save_personalization_preferences', { preferences });
}

export async function getLearningEngineConfig(): Promise<LearningEngineConfig> {
  return invoke('get_learning_engine_config');
}

export async function saveLearningEngineConfig(config: LearningEngineConfig): Promise<LearningEngineConfig> {
  return invoke('save_learning_engine_config', { config });
}

export async function recognizeAudio(input: RecordedAudio): Promise<RecognitionRecord> {
  return invoke('recognize_audio', { input });
}

export async function startNativeRecording(): Promise<void> {
  return invoke('start_native_recording');
}

export async function stopNativeRecording(): Promise<RecordedAudio | null> {
  return invoke('stop_native_recording');
}

export async function cancelNativeRecording(): Promise<void> {
  return invoke('cancel_native_recording');
}

export async function showRecorderOverlayNoActivate(): Promise<void> {
  return invoke('show_recorder_overlay_no_activate');
}

export async function showRecorderOverlayFocus(): Promise<void> {
  return invoke('show_recorder_overlay_focus');
}

export async function hideRecorderOverlayWindow(): Promise<void> {
  return invoke('hide_recorder_overlay');
}

export async function setRecorderOverlayPosition(x: number, y: number): Promise<void> {
  return invoke('set_recorder_overlay_position', { x, y });
}

export async function setRecorderOverlaySize(width: number, height: number): Promise<void> {
  return invoke('set_recorder_overlay_size', { width, height });
}

export async function setHotkeyMonitor(parts: string[] | null): Promise<void> {
  return invoke('set_hotkey_monitor', { parts });
}

export async function rememberInputTarget(): Promise<void> {
  return invoke('remember_input_target');
}

export async function restoreInputTarget(): Promise<void> {
  return invoke('restore_input_target');
}

export async function undoLastInjectedText(): Promise<void> {
  return invoke('undo_last_injected_text');
}

export async function writeRuntimeLog(message: string): Promise<void> {
  return invoke('write_runtime_log', { message });
}

export async function getRuntimeLogPath(): Promise<string> {
  return invoke('get_runtime_log_path');
}

export async function openExternalUrl(url: string): Promise<void> {
  return invoke('open_external_url', { url });
}
