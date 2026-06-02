import { describe, expect, it } from 'vitest';
import { aggregateUsageStats } from '../stats';

describe('aggregateUsageStats', () => {
  it('aggregates successful recognition records only', () => {
    const stats = aggregateUsageStats([
      { durationSeconds: 12, text: '你好世界', status: 'success', createdAt: '2026-06-02T09:00:00Z' },
      { durationSeconds: 30, text: 'custom vocabulary', status: 'success', createdAt: '2026-06-02T10:00:00Z' },
      { durationSeconds: 8, text: '', status: 'failed', createdAt: '2026-06-02T11:00:00Z' },
    ]);

    expect(stats.totalDurationSeconds).toBe(42);
    expect(stats.totalRecords).toBe(2);
    expect(stats.totalCharacters).toBe(21);
  });
});
