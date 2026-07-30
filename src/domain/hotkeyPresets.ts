/** Built-in wake hotkeys. Free-form recording is not supported. */
export const HOTKEY_PRESETS = [
  {
    id: 'lctrl-meta',
    value: 'LCtrl+Meta',
    label: '左 Ctrl + Win',
    description: '同时按住左侧 Ctrl 与 Win 键',
  },
  {
    id: 'ralt',
    value: 'RAlt',
    label: '右 Alt',
    description: '按住右侧 Alt 键',
  },
] as const;

export type HotkeyPresetValue = (typeof HOTKEY_PRESETS)[number]['value'];

export const DEFAULT_HOTKEY: HotkeyPresetValue = 'LCtrl+Meta';

const PRESET_VALUES = new Set<string>(HOTKEY_PRESETS.map((preset) => preset.value));

export function isHotkeyPreset(hotkey: string): hotkey is HotkeyPresetValue {
  return PRESET_VALUES.has(hotkey);
}

/** Map any stored/legacy value onto one of the two supported presets. */
export function normalizeHotkeyPreset(hotkey?: string | null): HotkeyPresetValue {
  const value = (hotkey ?? '').trim();
  if (isHotkeyPreset(value)) return value;
  if (value === 'AltRight' || value === 'AltGr') return 'RAlt';
  return DEFAULT_HOTKEY;
}

export function hotkeyPresetLabel(hotkey?: string | null): string {
  const normalized = normalizeHotkeyPreset(hotkey);
  return HOTKEY_PRESETS.find((preset) => preset.value === normalized)?.label ?? normalized;
}

export function toHotkeyParts(hotkey: string): string[] {
  return normalizeHotkeyPreset(hotkey)
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean);
}

/**
 * True when this DOM event is part of the active preset (used to suppress browser defaults).
 * Native hook is the real trigger; this only prevents the app UI from eating the keys.
 */
export function isEventPartOfHotkey(event: KeyboardEvent, hotkey?: string): boolean {
  const preset = normalizeHotkeyPreset(hotkey);

  if (preset === 'RAlt') {
    return event.code === 'AltRight' || (event.key === 'Alt' && event.location === 2);
  }

  // LCtrl+Meta
  if (event.code === 'ControlLeft' || (event.key === 'Control' && event.location === 1)) {
    return true;
  }
  if (event.code === 'MetaLeft' || event.code === 'MetaRight' || event.key === 'Meta') {
    return true;
  }
  // While the chord is held, suppress related events that still carry both modifiers.
  return Boolean(event.ctrlKey && event.metaKey);
}
