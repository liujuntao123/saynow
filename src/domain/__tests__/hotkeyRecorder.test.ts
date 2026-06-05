import { describe, expect, it } from 'vitest';
import { createHoldHotkeyController, formatHotkey, isModifierOnlyHotkey, toHotkeyParts } from '../hotkeyRecorder';

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

  it('starts recording on hotkey down and stops recording when the hotkey is released', () => {
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true, repeat: true }));
    controller.handleKeyUp(keyEvent({ key: 'Control' }));

    expect(actions).toEqual(['start', 'stop']);
  });

  it('keeps recording while a combination hotkey is held', () => {
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl+Space', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: ' ', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: ' ', ctrlKey: true, repeat: true }));
    controller.handleKeyUp(keyEvent({ key: ' ' }));

    expect(actions).toEqual(['start', 'stop']);
  });

  it('supports holding a modifier-only combination hotkey', () => {
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl+Shift', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: 'Shift', ctrlKey: true, shiftKey: true }));
    controller.handleKeyUp(keyEvent({ key: 'Shift' }));

    expect(actions).toEqual(['start', 'stop']);
  });
});
