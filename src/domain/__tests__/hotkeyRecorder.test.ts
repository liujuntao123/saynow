import { describe, expect, it } from 'vitest';
import { formatHotkey, isEventPartOfHotkey, isModifierOnlyHotkey, toHotkeyParts } from '../hotkeyRecorder';

function keyEvent(input: Partial<KeyboardEvent>): KeyboardEvent {
  return input as KeyboardEvent;
}

describe('hotkey recorder', () => {
  it('formats modifier combinations with a trigger key', () => {
    expect(formatHotkey(keyEvent({ key: ' ', ctrlKey: true }))).toBe('Ctrl+Space');
    expect(formatHotkey(keyEvent({ key: 'k', ctrlKey: true, shiftKey: true }))).toBe('Ctrl+Shift+K');
    expect(formatHotkey(keyEvent({ key: 'ArrowLeft', altKey: true }))).toBe('Alt+Left');
  });

  it('formats modifier-only input as a valid hotkey', () => {
    expect(formatHotkey(keyEvent({ key: 'Control', ctrlKey: true }))).toBe('Ctrl');
    expect(formatHotkey(keyEvent({ key: 'Meta', metaKey: true }))).toBe('Meta');
  });

  it('detects modifier-only hotkeys', () => {
    expect(isModifierOnlyHotkey('Ctrl')).toBe(true);
    expect(isModifierOnlyHotkey('Meta')).toBe(true);
    expect(isModifierOnlyHotkey('Ctrl+Space')).toBe(false);
  });

  it('treats Escape as cancellation instead of a hotkey', () => {
    expect(formatHotkey(keyEvent({ key: 'Escape' }))).toBeNull();
  });

  it('extracts native hotkey parts', () => {
    expect(toHotkeyParts('Ctrl+Space')).toEqual(['Ctrl', 'Space']);
    expect(toHotkeyParts('Alt+Shift+K')).toEqual(['Alt', 'Shift', 'K']);
  });

  it('detects DOM events that belong to the configured hotkey', () => {
    expect(isEventPartOfHotkey(keyEvent({ key: 'Alt', altKey: true }), 'Alt')).toBe(true);
    expect(isEventPartOfHotkey(keyEvent({ key: ' ', ctrlKey: true }), 'Ctrl+Space')).toBe(true);
    expect(isEventPartOfHotkey(keyEvent({ key: 'Control', ctrlKey: true }), 'Ctrl+Space')).toBe(true);
    expect(isEventPartOfHotkey(keyEvent({ key: ' ', altKey: true }), 'Ctrl+Space')).toBe(false);
    expect(isEventPartOfHotkey(keyEvent({ key: 'k', ctrlKey: true, shiftKey: true }), 'Ctrl+K')).toBe(false);
  });
});
