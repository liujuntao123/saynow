const modifierAliases: Record<string, string> = {
  Control: 'Ctrl',
  Shift: 'Shift',
  Alt: 'Alt',
  Meta: 'Meta',
};
const modifierKeys = ['Ctrl', 'Alt', 'Shift', 'Meta'];
const browserReservedModifierKeys = ['Alt'];
const unstableDefaultHotkeys = ['Ctrl+Shift+Space'];

export function normalizeHotkeyKey(key: string): string {
  if (key === ' ') return 'Space';
  if (key.length === 1) return key.toUpperCase();
  return key.replace(/^Arrow/, '');
}

function canonicalHotkeyKey(key: string): string {
  return modifierAliases[key] ?? normalizeHotkeyKey(key);
}

export function formatHotkey(event: KeyboardEvent): string | null {
  if (event.key === 'Escape') return null;

  const key = canonicalHotkeyKey(event.key);
  const parts = Array.from(new Set(hotkeyParts(
    [
      event.ctrlKey ? 'Ctrl' : '',
      event.altKey ? 'Alt' : '',
      event.shiftKey ? 'Shift' : '',
      event.metaKey ? 'Meta' : '',
      event.key in modifierAliases ? key : '',
    ].join('+'),
  )));

  if (!(event.key in modifierAliases)) parts.push(key);

  return parts.join('+');
}

export function isModifierOnlyHotkey(hotkey: string): boolean {
  const parts = hotkeyParts(hotkey);
  return parts.length > 0 && parts.every((part) => modifierKeys.includes(part));
}

export function usesBrowserReservedHotkey(hotkey: string): boolean {
  const parts = hotkeyParts(hotkey);
  return parts.some((part) => browserReservedModifierKeys.includes(part))
    || unstableDefaultHotkeys.includes(parts.join('+'));
}

export function toHotkeyParts(hotkey: string): string[] {
  return hotkey
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean);
}

function hotkeyParts(hotkey: string): string[] {
  return toHotkeyParts(hotkey);
}

export function isEventPartOfHotkey(event: KeyboardEvent, hotkey?: string): boolean {
  const parts = toHotkeyParts(hotkey ?? '');
  if (!parts.length) return false;

  const key = canonicalHotkeyKey(event.key);
  if (!parts.includes(key)) return false;

  const requiredModifiers = new Set(parts.filter((part) => modifierKeys.includes(part)));
  const activeModifiers = new Set(
    [
      event.ctrlKey ? 'Ctrl' : '',
      event.altKey ? 'Alt' : '',
      event.shiftKey ? 'Shift' : '',
      event.metaKey ? 'Meta' : '',
      event.key in modifierAliases ? key : '',
    ].filter(Boolean),
  );

  for (const modifier of activeModifiers) {
    if (!requiredModifiers.has(modifier)) return false;
  }

  if (modifierKeys.includes(key)) {
    return true;
  }

  for (const modifier of requiredModifiers) {
    if (!activeModifiers.has(modifier)) return false;
  }

  return true;
}
