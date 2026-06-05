const modifierAliases: Record<string, string> = {
  Control: 'Ctrl',
  Shift: 'Shift',
  Alt: 'Alt',
  Meta: 'Meta',
};
const standaloneModifierHoldDelayMs = 300;
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

export interface HoldHotkeyHandlers {
  onStart: () => void;
  onStop: () => void;
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

function eventMatchesHotkey(event: KeyboardEvent, hotkey: string): boolean {
  const parts = hotkeyParts(hotkey);
  if (!parts.length) return false;

  const triggerKey = canonicalHotkeyKey(event.key);
  const hasCtrl = parts.includes('Ctrl');
  const hasAlt = parts.includes('Alt');
  const hasShift = parts.includes('Shift');
  const hasMeta = parts.includes('Meta');
  const nonModifierParts = parts.filter((part) => !modifierKeys.includes(part));

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

function isStandaloneModifierHotkey(hotkey: string): boolean {
  const parts = hotkeyParts(hotkey);
  return parts.length === 1 && modifierKeys.includes(parts[0]);
}

export function createHoldHotkeyController(hotkey: string, handlers: HoldHotkeyHandlers) {
  let active = false;
  let pendingTimer: ReturnType<typeof globalThis.setTimeout> | null = null;

  function clearPending() {
    if (!pendingTimer) return;
    globalThis.clearTimeout(pendingTimer);
    pendingTimer = null;
  }

  function start() {
    active = true;
    handlers.onStart();
  }

  return {
    get active() {
      return active;
    },
    handleKeyDown(event: KeyboardEvent) {
      if (active && isStandaloneModifierHotkey(hotkey) && !eventMatchesHotkey(event, hotkey)) {
        active = false;
        handlers.onStop();
        return;
      }

      if (pendingTimer && !eventMatchesHotkey(event, hotkey)) {
        clearPending();
        return;
      }

      if (active || pendingTimer || event.repeat || !eventMatchesHotkey(event, hotkey)) return;
      if (isStandaloneModifierHotkey(hotkey)) {
        pendingTimer = globalThis.setTimeout(() => {
          pendingTimer = null;
          start();
        }, standaloneModifierHoldDelayMs);
        return;
      }

      start();
    },
    handleKeyUp(event: KeyboardEvent) {
      if (pendingTimer && eventReleasesHotkey(event, hotkey)) {
        clearPending();
        return;
      }
      if (!active || !eventReleasesHotkey(event, hotkey)) return;
      active = false;
      handlers.onStop();
    },
    cancel() {
      clearPending();
      if (!active) return;
      active = false;
      handlers.onStop();
    },
  };
}
