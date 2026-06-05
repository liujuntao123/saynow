import { afterEach, describe, expect, it, vi } from 'vitest';
import { createHoldHotkeyController, formatHotkey, isModifierOnlyHotkey, toHotkeyParts } from '../hotkeyRecorder';

function keyEvent(input: Partial<KeyboardEvent>): KeyboardEvent {
  return input as KeyboardEvent;
}

describe('hotkey recorder', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

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

  it('starts recording after a standalone modifier is held and stops when released', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true, repeat: true }));
    expect(actions).toEqual([]);

    vi.advanceTimersByTime(300);
    controller.handleKeyUp(keyEvent({ key: 'Control' }));

    expect(actions).toEqual(['start', 'stop']);
  });

  it('does not start recording for a short standalone modifier press', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Alt', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Alt', altKey: true }));
    vi.advanceTimersByTime(100);
    controller.handleKeyUp(keyEvent({ key: 'Alt' }));
    vi.advanceTimersByTime(300);

    expect(actions).toEqual([]);
  });

  it('does not start recording when a standalone modifier is used with another key', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Alt', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Alt', altKey: true }));
    controller.handleKeyDown(keyEvent({ key: 'Tab', altKey: true }));
    vi.advanceTimersByTime(300);
    controller.handleKeyUp(keyEvent({ key: 'Tab', altKey: true }));
    controller.handleKeyUp(keyEvent({ key: 'Alt' }));

    expect(actions).toEqual([]);
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
