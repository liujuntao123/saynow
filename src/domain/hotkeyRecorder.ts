const modifierAliases: Record<string, string> = {
  Control: 'Ctrl',
  Shift: 'Shift',
  Alt: 'Alt',
  Meta: 'Meta',
};

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
  return parts.length > 0 && parts.every((part) => ['Ctrl', 'Alt', 'Shift', 'Meta'].includes(part));
}

export function toGlobalShortcut(hotkey: string): string {
  return hotkeyParts(hotkey)
    .map((part) => {
      if (part === 'Ctrl') return 'CommandOrControl';
      if (part === 'Meta') return 'Command';
      return part;
    })
    .join('+');
}

export interface HoldHotkeyHandlers {
  onStart: () => void;
  onStop: () => void;
}

function hotkeyParts(hotkey: string): string[] {
  return hotkey
    .split('+')
    .map((part) => part.trim())
    .filter(Boolean);
}

function eventMatchesHotkey(event: KeyboardEvent, hotkey: string): boolean {
  const parts = hotkeyParts(hotkey);
  if (!parts.length) return false;

  const triggerKey = canonicalHotkeyKey(event.key);
  const hasCtrl = parts.includes('Ctrl');
  const hasAlt = parts.includes('Alt');
  const hasShift = parts.includes('Shift');
  const hasMeta = parts.includes('Meta');
  const nonModifierParts = parts.filter((part) => !['Ctrl', 'Alt', 'Shift', 'Meta'].includes(part));

  if (Boolean(event.ctrlKey) !== hasCtrl || Boolean(event.altKey) !== hasAlt || Boolean(event.shiftKey) !== hasShift || Boolean(event.metaKey) !== hasMeta) {
    return false;
  }

  if (!nonModifierParts.length) {
    return formatHotkey(event) === hotkey;
  }

  return nonModifierParts.length === 1 && nonModifierParts[0] === triggerKey;
}

function eventReleasesHotkey(event: KeyboardEvent, hotkey: string): boolean {
  const releasedKey = canonicalHotkeyKey(event.key);
  return hotkeyParts(hotkey).includes(releasedKey);
}

export function createHoldHotkeyController(hotkey: string, handlers: HoldHotkeyHandlers) {
  let active = false;

  return {
    handleKeyDown(event: KeyboardEvent) {
      if (active || event.repeat || !eventMatchesHotkey(event, hotkey)) return;
      active = true;
      handlers.onStart();
    },
    handleKeyUp(event: KeyboardEvent) {
      if (!active || !eventReleasesHotkey(event, hotkey)) return;
      active = false;
      handlers.onStop();
    },
  };
}
