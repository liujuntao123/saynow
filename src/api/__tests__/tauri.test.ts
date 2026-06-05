import { describe, expect, it } from 'vitest';
import { addStylePrompt, updateStylePrompt } from '../tauri';

describe('local preview API', () => {
  it('keeps at most one style prompt enabled', async () => {
    await addStylePrompt({
      id: 0,
      name: '口语整理',
      prompt: '整理为自然口语。',
      enabled: true,
    });
    const styles = await addStylePrompt({
      id: 0,
      name: '会议纪要',
      prompt: '整理为会议纪要。',
      enabled: true,
    });

    const enabled = styles.filter((style) => style.enabled);
    expect(enabled).toHaveLength(1);
    expect(enabled[0].name).toBe('会议纪要');

    const disabled = await updateStylePrompt({ ...enabled[0], enabled: false });
    expect(disabled.every((style) => !style.enabled)).toBe(true);
  });
});
