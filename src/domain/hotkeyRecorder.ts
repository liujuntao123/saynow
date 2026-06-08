const modifierAliases: Record<string, string> = {
  Control: 'Ctrl',
  Shift: 'Shift',
  Alt: 'Alt',
  Meta: 'Meta',
};
const hotkeyHoldDelayMs = 500;
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

export function createHoldHotkeyController(hotkey: string, handlers: HoldHotkeyHandlers) {
  const requiredKeys = new Set(hotkeyParts(hotkey));
  const pressedKeys = new Set<string>();
  let active = false;
  let pendingTimer: ReturnType<typeof globalThis.setTimeout> | null = null;

  function hotkeyExactlyPressed() {
    if (!requiredKeys.size || pressedKeys.size !== requiredKeys.size) return false;
    return Array.from(requiredKeys).every((key) => pressedKeys.has(key));
  }

  function eventReleasesHotkey(event: KeyboardEvent) {
    return requiredKeys.has(canonicalHotkeyKey(event.key));
  }

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
      const key = canonicalHotkeyKey(event.key);
      if (event.repeat && pressedKeys.has(key)) return;

      pressedKeys.add(key);

      if (active && !hotkeyExactlyPressed()) {
        active = false;
        handlers.onStop();
        return;
      }

      if (pendingTimer && !hotkeyExactlyPressed()) {
        clearPending();
        return;
      }

      if (active || pendingTimer || !hotkeyExactlyPressed()) return;
      pendingTimer = globalThis.setTimeout(() => {
        pendingTimer = null;
        if (!hotkeyExactlyPressed()) return;
        start();
      }, hotkeyHoldDelayMs);
    },
    handleKeyUp(event: KeyboardEvent) {
      const releasesHotkey = eventReleasesHotkey(event);
      pressedKeys.delete(canonicalHotkeyKey(event.key));

      if (pendingTimer && releasesHotkey) {
        clearPending();
        return;
      }
      if (!active || !releasesHotkey) return;
      active = false;
      handlers.onStop();
    },
    cancel() {
      clearPending();
      pressedKeys.clear();
      if (!active) return;
      active = false;
      handlers.onStop();
    },
  };
}
