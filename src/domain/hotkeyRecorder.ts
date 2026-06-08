const modifierAliases: Record<string, string> = {
  Control: 'Ctrl',
  Shift: 'Shift',
  Alt: 'Alt',
  Meta: 'Meta',
};
const modifierKeys = ['Ctrl', 'Alt', 'Shift', 'Meta'];

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

export function toHotkeyParts(hotkey: string): string[] {
  return hotkey
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean);
}

function hotkeyParts(hotkey: string): string[] {
  return toHotkeyParts(hotkey);
}
