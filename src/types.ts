export type RecognitionStatus = 'success' | 'failed' | 'processing';

export interface UsageStats {
  totalDurationSeconds: number;
  totalRecords: number;
  totalCharacters: number;
  todayDurationSeconds: number;
  todayRecords: number;
  todayCharacters: number;
  successRatePercent?: number | null;
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
  learningProcessedAt?: string | null;
}

export interface RecordedAudio {
  audioBase64: string;
  durationSeconds: number;
  mimeType: string;
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
  evidenceRecognitionIds: string;
  risk: string;
}

/** openai-compatible chat ASR / DashScope multimodal ASR / doubao websocket ASR */
export type ProviderKind = 'openai-compatible' | 'dashscope-multimodal' | 'doubao-ws';

/** 豆包鉴权：旧控制台 AppKey+AccessKey，新控制台仅 API Key */
export type DoubaoAuthMode = 'legacy' | 'apiKey';

export interface AppConfig {
  provider: string;
  baseUrl: string;
  model: string;
  apiKeyRef: string;
  hotkey: string;
  kind?: ProviderKind | string;
  extraJson?: string;
}

export interface ProviderConfig {
  id: number;
  provider: string;
  baseUrl: string;
  model: string;
  apiKeyRef: string;
  enabled: boolean;
  kind?: ProviderKind | string;
  extraJson?: string;
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
  /** 识别后是否显示“已输入”文字编辑浮条，默认关闭。 */
  showCorrectionStrip: boolean;
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
