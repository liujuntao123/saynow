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

export interface AppConfig {
  provider: string;
  baseUrl: string;
  model: string;
  apiKeyRef: string;
  hotkey: string;
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

export interface DashboardData {
  stats: UsageStats;
  records: RecognitionRecord[];
  platform: PlatformStatus;
}
