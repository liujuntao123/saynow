import { describe, expect, it } from 'vitest';
import { formatHotkey } from '../hotkeyRecorder';

function keyEvent(input: Partial<KeyboardEvent>): KeyboardEvent {
  return input as KeyboardEvent;
}

describe('hotkey recorder', () => {
  it('formats modifier combinations with a trigger key', () => {
    expect(formatHotkey(keyEvent({ key: ' ', ctrlKey: true }))).toBe('Ctrl+Space');
    expect(formatHotkey(keyEvent({ key: 'k', ctrlKey: true, shiftKey: true }))).toBe('Ctrl+Shift+K');
    expect(formatHotkey(keyEvent({ key: 'ArrowLeft', altKey: true }))).toBe('Alt+Left');
  });

  it('ignores modifier-only input until a trigger key is pressed', () => {
    expect(formatHotkey(keyEvent({ key: 'Control', ctrlKey: true }))).toBeNull();
    expect(formatHotkey(keyEvent({ key: 'Shift', shiftKey: true }))).toBeNull();
  });

  it('treats Escape as cancellation instead of a hotkey', () => {
    expect(formatHotkey(keyEvent({ key: 'Escape' }))).toBeNull();
  });
});
