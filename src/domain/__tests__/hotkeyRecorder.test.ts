import { afterEach, describe, expect, it, vi } from 'vitest';
import { createHoldHotkeyController, formatHotkey, isModifierOnlyHotkey, toHotkeyParts } from '../hotkeyRecorder';

function keyEvent(input: Partial<KeyboardEvent>): KeyboardEvent {
  return input as KeyboardEvent;
}

const standaloneModifierSamples = [
  { hotkey: 'Ctrl', key: 'Control', state: { ctrlKey: true } },
  { hotkey: 'Alt', key: 'Alt', state: { altKey: true } },
  { hotkey: 'Shift', key: 'Shift', state: { shiftKey: true } },
  { hotkey: 'Meta', key: 'Meta', state: { metaKey: true } },
] as const;

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

  it.each(standaloneModifierSamples)('starts recording after standalone $hotkey is held and stops when released', ({ hotkey, key, state }) => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController(hotkey, {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key, ...state }));
    controller.handleKeyDown(keyEvent({ key, ...state, repeat: true }));
    expect(actions).toEqual([]);

    vi.advanceTimersByTime(500);
    controller.handleKeyUp(keyEvent({ key }));

    expect(actions).toEqual(['start', 'stop']);
  });

  it.each(standaloneModifierSamples)('does not start recording for a short standalone $hotkey press', ({ hotkey, key, state }) => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController(hotkey, {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key, ...state }));
    vi.advanceTimersByTime(100);
    controller.handleKeyUp(keyEvent({ key }));
    vi.advanceTimersByTime(500);

    expect(actions).toEqual([]);
  });

  it.each(standaloneModifierSamples)('does not start recording when standalone $hotkey is used with another key', ({ hotkey, key, state }) => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController(hotkey, {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key, ...state }));
    controller.handleKeyDown(keyEvent({ key: 'A', ...state }));
    vi.advanceTimersByTime(500);
    controller.handleKeyUp(keyEvent({ key: 'A', ...state }));
    controller.handleKeyUp(keyEvent({ key }));

    expect(actions).toEqual([]);
  });

  it('starts recording after a combination hotkey is held', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl+Space', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: ' ', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: ' ', ctrlKey: true, repeat: true }));
    expect(actions).toEqual([]);

    vi.advanceTimersByTime(500);
    controller.handleKeyUp(keyEvent({ key: ' ' }));

    expect(actions).toEqual(['start', 'stop']);
  });

  it('does not start recording for a short combination hotkey press', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl+Space', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: ' ', ctrlKey: true }));
    vi.advanceTimersByTime(100);
    controller.handleKeyUp(keyEvent({ key: ' ' }));
    vi.advanceTimersByTime(500);

    expect(actions).toEqual([]);
  });

  it('does not start recording when a combination hotkey is used with another key', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl+Space', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: ' ', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: 'A', ctrlKey: true }));
    vi.advanceTimersByTime(500);
    controller.handleKeyUp(keyEvent({ key: 'A', ctrlKey: true }));
    controller.handleKeyUp(keyEvent({ key: ' ' }));

    expect(actions).toEqual([]);
  });

  it('does not start recording when another key is already held before a combination hotkey', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl+Space', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'A' }));
    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: ' ', ctrlKey: true }));
    vi.advanceTimersByTime(500);
    controller.handleKeyUp(keyEvent({ key: ' ' }));
    controller.handleKeyUp(keyEvent({ key: 'Control' }));
    controller.handleKeyUp(keyEvent({ key: 'A' }));

    expect(actions).toEqual([]);
  });

  it('supports holding a modifier-only combination hotkey', () => {
    vi.useFakeTimers();
    const actions: string[] = [];
    const controller = createHoldHotkeyController('Ctrl+Shift', {
      onStart: () => actions.push('start'),
      onStop: () => actions.push('stop'),
    });

    controller.handleKeyDown(keyEvent({ key: 'Control', ctrlKey: true }));
    controller.handleKeyDown(keyEvent({ key: 'Shift', ctrlKey: true, shiftKey: true }));
    expect(actions).toEqual([]);

    vi.advanceTimersByTime(500);
    controller.handleKeyUp(keyEvent({ key: 'Shift' }));

    expect(actions).toEqual(['start', 'stop']);
  });
});
