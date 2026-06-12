export type RecognitionStatus = 'success' | 'failed' | 'processing';

export interface UsageStats {
  totalDurationSeconds: number;
  totalRecords: number;
  totalCharacters: number;
}

export interface PlatformStatus {
  supported: boolean;
  message: string;
}

export interface RecognitionRecord {
  id: number;
  createdAt: string;
  durationSeconds: number;
  text: string;
  provider: string;
  model: string;
  status: RecognitionStatus;
  errorMessage?: string | null;
}

export interface CorrectionRecord {
  id: number;
  createdAt: string;
  recognitionRecordId: number;
  rawText: string;
  correctedText: string;
  source: string;
  applied: boolean;
  errorMessage?: string | null;
  learningProcessedAt?: string | null;
}

export interface SaveCorrectionInput {
  recognitionRecordId: number;
  rawText: string;
  correctedText: string;
  source: string;
  applyReplacement: boolean;
}

export interface LearningRule {
  id: number;
  createdAt: string;
  updatedAt: string;
  ruleType: string;
  description: string;
  matchHints: string;
  fromText: string;
  toText: string;
  confidence: number;
  status: string;
  evidenceCorrectionIds: string;
  risk: string;
}

export interface AppConfig {
  provider: string;
  baseUrl: string;
  model: string;
  apiKeyRef: string;
  hotkey: string;
}

export interface ProviderConfig {
  id: number;
  provider: string;
  baseUrl: string;
  model: string;
  apiKeyRef: string;
  enabled: boolean;
}

export interface VocabularyItem {
  id: number;
  term: string;
  alias: string;
  category: string;
  note: string;
  enabled: boolean;
}

export interface StylePrompt {
  id: number;
  name: string;
  prompt: string;
  enabled: boolean;
}

export interface PersonalizationPreferences {
  removeTrailingPeriod: boolean;
}

export interface LearningEngineConfig {
  enabled: boolean;
  provider: string;
  baseUrl: string;
  model: string;
  apiKeyRef: string;
  runMode: 'localOnly' | 'llmAssist';
  minNewCorrections: number;
  idleSeconds: number;
}

export interface DashboardData {
  stats: UsageStats;
  records: RecognitionRecord[];
  platform: PlatformStatus;
}
