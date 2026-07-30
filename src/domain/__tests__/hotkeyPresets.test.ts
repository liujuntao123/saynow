import { describe, expect, it } from 'vitest';
import {
  DEFAULT_HOTKEY,
  hotkeyPresetLabel,
  isEventPartOfHotkey,
  normalizeHotkeyPreset,
  toHotkeyParts,
} from '../hotkeyPresets';

function keyEvent(input: Partial<KeyboardEvent>): KeyboardEvent {
  return input as KeyboardEvent;
}

describe('hotkey presets', () => {
  it('normalizes legacy values onto the two supported presets', () => {
    expect(normalizeHotkeyPreset('LCtrl+Meta')).toBe('LCtrl+Meta');
    expect(normalizeHotkeyPreset('RAlt')).toBe('RAlt');
    expect(normalizeHotkeyPreset('Alt')).toBe(DEFAULT_HOTKEY);
    expect(normalizeHotkeyPreset('LAlt')).toBe(DEFAULT_HOTKEY);
    expect(normalizeHotkeyPreset('F8')).toBe(DEFAULT_HOTKEY);
    expect(normalizeHotkeyPreset('AltRight')).toBe('RAlt');
    expect(normalizeHotkeyPreset('')).toBe(DEFAULT_HOTKEY);
  });

  it('exposes human labels and native parts', () => {
    expect(hotkeyPresetLabel('RAlt')).toBe('右 Alt');
    expect(hotkeyPresetLabel('LCtrl+Meta')).toBe('左 Ctrl + Win');
    expect(toHotkeyParts('RAlt')).toEqual(['RAlt']);
    expect(toHotkeyParts('LCtrl+Meta')).toEqual(['LCtrl', 'Meta']);
    expect(toHotkeyParts('Alt')).toEqual(['LCtrl', 'Meta']);
  });

  it('detects DOM events that belong to each preset', () => {
    expect(
      isEventPartOfHotkey(keyEvent({ key: 'Alt', code: 'AltRight', location: 2 }), 'RAlt'),
    ).toBe(true);
    expect(
      isEventPartOfHotkey(keyEvent({ key: 'Alt', code: 'AltLeft', location: 1 }), 'RAlt'),
    ).toBe(false);
    expect(
      isEventPartOfHotkey(keyEvent({ key: 'Control', code: 'ControlLeft', location: 1 }), 'LCtrl+Meta'),
    ).toBe(true);
    expect(
      isEventPartOfHotkey(keyEvent({ key: 'Meta', code: 'MetaLeft' }), 'LCtrl+Meta'),
    ).toBe(true);
    expect(
      isEventPartOfHotkey(keyEvent({ key: 'Control', code: 'ControlRight', location: 2 }), 'LCtrl+Meta'),
    ).toBe(false);
  });
});
