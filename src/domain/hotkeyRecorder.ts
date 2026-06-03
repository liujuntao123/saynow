const modifierKeys = new Set(['Control', 'Shift', 'Alt', 'Meta']);

export function normalizeHotkeyKey(key: string): string {
  if (key === ' ') return 'Space';
  if (key.length === 1) return key.toUpperCase();
  return key.replace(/^Arrow/, '');
}

export function formatHotkey(event: KeyboardEvent): string | null {
  if (event.key === 'Escape') return null;

  const key = normalizeHotkeyKey(event.key);
  if (modifierKeys.has(key)) return null;

  const parts = [
    event.ctrlKey ? 'Ctrl' : '',
    event.altKey ? 'Alt' : '',
    event.shiftKey ? 'Shift' : '',
    event.metaKey ? 'Meta' : '',
    key,
  ].filter(Boolean);

  return parts.join('+');
}
