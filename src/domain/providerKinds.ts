import type { DoubaoAuthMode, ProviderConfig, ProviderKind } from '../types';

export const KIND_OPENAI_COMPATIBLE: ProviderKind = 'openai-compatible';
export const KIND_DASHSCOPE_MULTIMODAL: ProviderKind = 'dashscope-multimodal';
export const KIND_DOUBAO_WS: ProviderKind = 'doubao-ws';

export const QWEN_AUDIO_ASR_MODEL = 'qwen-audio-3.0-asr-flash';

export const DEFAULT_DOUBAO_ENDPOINT =
  'wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async';

export const DEFAULT_DOUBAO_RESOURCE_ID = 'volc.seedasr.sauc.duration';

export const DOUBAO_RESOURCE_OPTIONS = [
  {
    id: 'seed-duration',
    label: '2.0 小时版',
    value: 'volc.seedasr.sauc.duration',
  },
  {
    id: 'seed-concurrent',
    label: '2.0 并发版',
    value: 'volc.seedasr.sauc.concurrent',
  },
  {
    id: 'big-duration',
    label: '1.0 小时版',
    value: 'volc.bigasr.sauc.duration',
  },
  {
    id: 'big-concurrent',
    label: '1.0 并发版',
    value: 'volc.bigasr.sauc.concurrent',
  },
] as const;

export interface DoubaoExtra {
  authMode: DoubaoAuthMode;
  accessKey: string;
}

export function isDoubaoProvider(
  kind?: string | null,
  provider?: string | null,
): boolean {
  const normalizedKind = (kind ?? '').trim().toLowerCase();
  if (normalizedKind === KIND_DOUBAO_WS || normalizedKind === 'doubao') {
    return true;
  }
  const name = (provider ?? '').toLowerCase();
  return name.includes('doubao') || name.includes('豆包');
}

export function isDashScopeMultimodalProvider(
  kind?: string | null,
  model?: string | null,
): boolean {
  if ((kind ?? '').trim().toLowerCase() === KIND_DASHSCOPE_MULTIMODAL) {
    return true;
  }
  return (model ?? '').trim().toLowerCase() === QWEN_AUDIO_ASR_MODEL;
}

export function parseDoubaoExtra(raw?: string | null): DoubaoExtra {
  const fallback: DoubaoExtra = { authMode: 'legacy', accessKey: '' };
  if (!raw?.trim()) return fallback;
  try {
    const parsed = JSON.parse(raw) as {
      authMode?: string;
      accessKey?: string;
    };
    const authMode: DoubaoAuthMode =
      parsed.authMode === 'apiKey' || parsed.authMode === 'api_key' || parsed.authMode === 'new'
        ? 'apiKey'
        : 'legacy';
    return {
      authMode,
      accessKey: typeof parsed.accessKey === 'string' ? parsed.accessKey : '',
    };
  } catch {
    return fallback;
  }
}

export function serializeDoubaoExtra(extra: DoubaoExtra): string {
  return JSON.stringify({
    authMode: extra.authMode,
    accessKey: extra.accessKey.trim(),
  });
}

export function providerSubtitle(provider: Pick<ProviderConfig, 'kind' | 'provider' | 'model' | 'baseUrl'>): string {
  if (isDoubaoProvider(provider.kind, provider.provider)) {
    const resource = provider.model?.trim() || DEFAULT_DOUBAO_RESOURCE_ID;
    const short = resource.replace(/^volc\./, '');
    return short;
  }
  return provider.model || provider.baseUrl || '';
}

export function isProviderFormComplete(provider: ProviderConfig): boolean {
  if (!provider.provider.trim()) return false;
  if (isDoubaoProvider(provider.kind, provider.provider)) {
    if (!provider.apiKeyRef.trim()) return false;
    const extra = parseDoubaoExtra(provider.extraJson);
    if (extra.authMode === 'legacy') {
      return Boolean(extra.accessKey.trim());
    }
    return true;
  }
  return Boolean(
    provider.baseUrl.trim() &&
      provider.model.trim() &&
      provider.apiKeyRef.trim(),
  );
}
