import type { ShortcutBinding, ShortcutConfig } from "../../types";

export type ShortcutAction = keyof ShortcutConfig;
export type ShortcutScope = "application" | "terminal";

export interface ShortcutKeyboardEvent {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  isComposing?: boolean;
}

export type ShortcutCaptureResult =
  | { kind: "binding"; binding: ShortcutBinding }
  | { kind: "cancel" }
  | { kind: "clear" }
  | { kind: "invalid" };

export const SHORTCUT_ACTIONS: Array<{
  id: ShortcutAction;
  labelKey: string;
  scope: ShortcutScope;
}> = [
  {
    id: "new_connection",
    labelKey: "settings.shortcuts.action.new_connection",
    scope: "application",
  },
  {
    id: "new_window",
    labelKey: "settings.shortcuts.action.new_window",
    scope: "application",
  },
  {
    id: "open_settings",
    labelKey: "settings.shortcuts.action.open_settings",
    scope: "application",
  },
  {
    id: "terminal_select_all",
    labelKey: "settings.shortcuts.action.terminal_select_all",
    scope: "terminal",
  },
  {
    id: "terminal_copy",
    labelKey: "settings.shortcuts.action.terminal_copy",
    scope: "terminal",
  },
  {
    id: "terminal_paste",
    labelKey: "settings.shortcuts.action.terminal_paste",
    scope: "terminal",
  },
];

const DEFAULT_NEW_CONNECTION_SHORTCUT: ShortcutBinding = {
  key: "n",
  ctrl: true,
  alt: false,
  shift: false,
};
const DEFAULT_NEW_WINDOW_SHORTCUT: ShortcutBinding = {
  key: "n",
  ctrl: true,
  alt: false,
  shift: true,
};
const DEFAULT_OPEN_SETTINGS_SHORTCUT: ShortcutBinding = {
  key: ",",
  ctrl: true,
  alt: false,
  shift: false,
};
const DEFAULT_TERMINAL_SELECT_ALL_SHORTCUT: ShortcutBinding = {
  key: "a",
  ctrl: true,
  alt: false,
  shift: true,
};
const DEFAULT_TERMINAL_COPY_SHORTCUT: ShortcutBinding = {
  key: "c",
  ctrl: true,
  alt: false,
  shift: true,
};
const DEFAULT_TERMINAL_PASTE_SHORTCUT: ShortcutBinding = {
  key: "v",
  ctrl: true,
  alt: false,
  shift: true,
};

export const DEFAULT_SHORTCUT_CONFIG: ShortcutConfig = {
  new_connection: DEFAULT_NEW_CONNECTION_SHORTCUT,
  new_window: DEFAULT_NEW_WINDOW_SHORTCUT,
  open_settings: DEFAULT_OPEN_SETTINGS_SHORTCUT,
  terminal_select_all: DEFAULT_TERMINAL_SELECT_ALL_SHORTCUT,
  terminal_copy: DEFAULT_TERMINAL_COPY_SHORTCUT,
  terminal_paste: DEFAULT_TERMINAL_PASTE_SHORTCUT,
};

export function createDefaultShortcutConfig(): ShortcutConfig {
  return {
    new_connection: { ...DEFAULT_NEW_CONNECTION_SHORTCUT },
    new_window: { ...DEFAULT_NEW_WINDOW_SHORTCUT },
    open_settings: { ...DEFAULT_OPEN_SETTINGS_SHORTCUT },
    terminal_select_all: { ...DEFAULT_TERMINAL_SELECT_ALL_SHORTCUT },
    terminal_copy: { ...DEFAULT_TERMINAL_COPY_SHORTCUT },
    terminal_paste: { ...DEFAULT_TERMINAL_PASTE_SHORTCUT },
  };
}

export function normalizeShortcutKey(key: string): string | null {
  if (key === " " || key === "Spacebar" || key.toLowerCase() === "space") {
    return "Space";
  }

  if (/^f(?:[1-9]|1[0-2])$/i.test(key)) {
    return key.toUpperCase();
  }

  if (Array.from(key).length !== 1 || /\s/u.test(key)) {
    return null;
  }

  return /^[A-Z]$/.test(key) ? key.toLowerCase() : key;
}

export function normalizeShortcutBinding(binding: ShortcutBinding): ShortcutBinding | null {
  const key = normalizeShortcutKey(binding.key);
  if (!key) return null;
  return { ...binding, key };
}

export function shortcutBindingsEqual(
  left: ShortcutBinding | null,
  right: ShortcutBinding | null
): boolean {
  if (!left || !right) return left === right;
  const normalizedLeft = normalizeShortcutBinding(left);
  const normalizedRight = normalizeShortcutBinding(right);
  if (!normalizedLeft || !normalizedRight) return false;
  return (
    normalizedLeft.key === normalizedRight.key &&
    normalizedLeft.ctrl === normalizedRight.ctrl &&
    normalizedLeft.alt === normalizedRight.alt &&
    normalizedLeft.shift === normalizedRight.shift
  );
}

export function isShortcutBindingValid(binding: ShortcutBinding): boolean {
  const normalized = normalizeShortcutBinding(binding);
  if (!normalized) return false;

  const isFunctionKey = /^F(?:[1-9]|1[0-2])$/.test(normalized.key);
  if (!isFunctionKey && !normalized.ctrl && !normalized.alt) return false;
  if (normalized.alt && normalized.key === "F4") return false;
  return true;
}

export function captureShortcut(event: ShortcutKeyboardEvent): ShortcutCaptureResult {
  if (event.isComposing || event.metaKey) return { kind: "invalid" };
  if (event.key === "Escape") return { kind: "cancel" };
  if (event.key === "Backspace" || event.key === "Delete") return { kind: "clear" };

  const binding = normalizeShortcutBinding({
    key: event.key,
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
  });
  if (!binding || !isShortcutBindingValid(binding)) return { kind: "invalid" };
  return { kind: "binding", binding };
}

export function formatShortcut(binding: ShortcutBinding | null): string {
  if (!binding) return "";
  const normalized = normalizeShortcutBinding(binding);
  if (!normalized) return "";

  const parts: string[] = [];
  if (normalized.ctrl) parts.push("Ctrl");
  if (normalized.alt) parts.push("Alt");
  if (normalized.shift) parts.push("Shift");
  parts.push(/^[a-z]$/.test(normalized.key) ? normalized.key.toUpperCase() : normalized.key);
  return parts.join("+");
}

export function matchesShortcut(
  binding: ShortcutBinding | null,
  event: ShortcutKeyboardEvent
): boolean {
  if (!binding || event.isComposing || event.metaKey) return false;
  const normalized = normalizeShortcutBinding(binding);
  const eventKey = normalizeShortcutKey(event.key);
  if (!normalized || !eventKey) return false;
  return (
    normalized.key === eventKey &&
    normalized.ctrl === event.ctrlKey &&
    normalized.alt === event.altKey &&
    normalized.shift === event.shiftKey
  );
}

export function findShortcutAction(
  shortcuts: ShortcutConfig,
  event: ShortcutKeyboardEvent,
  scope: ShortcutScope
): ShortcutAction | null {
  return (
    SHORTCUT_ACTIONS.find(
      ({ id, scope: actionScope }) => actionScope === scope && matchesShortcut(shortcuts[id], event)
    )?.id ?? null
  );
}

export function findShortcutConflict(
  shortcuts: ShortcutConfig,
  action: ShortcutAction,
  binding: ShortcutBinding
): ShortcutAction | null {
  return (
    SHORTCUT_ACTIONS.find(
      ({ id }) => id !== action && shortcutBindingsEqual(shortcuts[id], binding)
    )?.id ?? null
  );
}

export function normalizeShortcutConfig(shortcuts?: Partial<ShortcutConfig>): ShortcutConfig {
  const defaults = createDefaultShortcutConfig();
  if (!shortcuts) return defaults;
  return {
    new_connection:
      shortcuts.new_connection === undefined ? defaults.new_connection : shortcuts.new_connection,
    new_window: shortcuts.new_window === undefined ? defaults.new_window : shortcuts.new_window,
    open_settings:
      shortcuts.open_settings === undefined ? defaults.open_settings : shortcuts.open_settings,
    terminal_select_all:
      shortcuts.terminal_select_all === undefined
        ? defaults.terminal_select_all
        : shortcuts.terminal_select_all,
    terminal_copy:
      shortcuts.terminal_copy === undefined ? defaults.terminal_copy : shortcuts.terminal_copy,
    terminal_paste:
      shortcuts.terminal_paste === undefined ? defaults.terminal_paste : shortcuts.terminal_paste,
  };
}
