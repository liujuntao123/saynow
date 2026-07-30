import { describe, expect, it } from 'vitest';
import {
  isDoubaoProvider,
  isProviderFormComplete,
  parseDoubaoExtra,
  providerSubtitle,
  serializeDoubaoExtra,
} from '../providerKinds';

describe('providerKinds', () => {
  it('detects doubao by kind or name', () => {
    expect(isDoubaoProvider('doubao-ws', 'x')).toBe(true);
    expect(isDoubaoProvider('', 'Doubao ASR')).toBe(true);
    expect(isDoubaoProvider('openai-compatible', 'Qwen')).toBe(false);
  });

  it('roundtrips doubao extra json', () => {
    const raw = serializeDoubaoExtra({ authMode: 'legacy', accessKey: 'token' });
    expect(parseDoubaoExtra(raw)).toEqual({ authMode: 'legacy', accessKey: 'token' });
  });

  it('validates doubao form completeness', () => {
    expect(
      isProviderFormComplete({
        id: 0,
        provider: 'Doubao ASR',
        baseUrl: '',
        model: '',
        apiKeyRef: 'app',
        enabled: true,
        kind: 'doubao-ws',
        extraJson: serializeDoubaoExtra({ authMode: 'legacy', accessKey: '' }),
      }),
    ).toBe(false);

    expect(
      isProviderFormComplete({
        id: 0,
        provider: 'Doubao ASR',
        baseUrl: '',
        model: '',
        apiKeyRef: 'key',
        enabled: true,
        kind: 'doubao-ws',
        extraJson: serializeDoubaoExtra({ authMode: 'apiKey', accessKey: '' }),
      }),
    ).toBe(true);
  });

  it('shows resource id for doubao subtitle', () => {
    expect(
      providerSubtitle({
        kind: 'doubao-ws',
        provider: 'Doubao ASR',
        model: 'volc.seedasr.sauc.duration',
        baseUrl: '',
      }),
    ).toBe('seedasr.sauc.duration');
  });
});
