import { describe, expect, it } from 'vitest';
import { buildPromptPreview } from '../promptContext';

describe('buildPromptPreview', () => {
  it('includes enabled vocabulary, active style, and recent successful history', () => {
    const prompt = buildPromptPreview({
      vocabulary: [
        { term: 'Kunlun', alias: '昆仑', enabled: true },
        { term: 'unused', alias: '', enabled: false },
      ],
      styles: [{ name: '书面语', prompt: '整理为简洁书面语。', enabled: true }],
      records: [
        { text: '昨天讨论 Kunlun 模型导出。', status: 'success' },
        { text: '失败记录', status: 'failed' },
      ],
    });

    expect(prompt).toContain('Kunlun');
    expect(prompt).toContain('昆仑');
    expect(prompt).toContain('整理为简洁书面语');
    expect(prompt).toContain('昨天讨论 Kunlun 模型导出');
    expect(prompt).not.toContain('unused');
    expect(prompt).not.toContain('失败记录');
  });
});
