import type { ShortcutConfig, ViewMode } from "../../types";
import { formatShortcut } from "../../features/shortcuts/shortcutModel";
import type { PopoverMenuItem } from "../Common";

export const TITLE_BAR_MENU_KEYS = ["file", "edit", "help"] as const;
export type TitleBarMenuKey = (typeof TITLE_BAR_MENU_KEYS)[number];

export function adjacentTitleBarMenu(menu: TitleBarMenuKey, direction: "previous" | "next") {
  const offset = direction === "next" ? 1 : -1;
  return TITLE_BAR_MENU_KEYS[
    (TITLE_BAR_MENU_KEYS.indexOf(menu) + offset + TITLE_BAR_MENU_KEYS.length) %
      TITLE_BAR_MENU_KEYS.length
  ];
}

interface TitleBarMenuLabels {
  newConnection: string;
  newWindow: string;
  sessionLogs: string;
  settings: string;
  checkUpdates: string;
  documentation: string;
  about: string;
  exit: string;
  selectAll: string;
  copy: string;
  paste: string;
  clearViewport: string;
  clearBuffer: string;
}

interface TitleBarMenuActions {
  openConnection: () => void;
  openWindow: () => void;
  openSessionLogs: () => void;
  openSettings: () => void;
  checkUpdates: () => void;
  openDocumentation: () => void;
  openAbout: () => void;
  exit: () => void;
  selectAll: () => void;
  copy: () => void;
  paste: () => void;
  clearViewport: () => void;
  clearBuffer: () => void;
}

interface CreateTitleBarMenusOptions {
  activeView: ViewMode;
  shortcuts: ShortcutConfig;
  canAccessTerminal: boolean;
  canCopyTerminal: boolean;
  canPasteTerminal: boolean;
  labels: TitleBarMenuLabels;
  actions: TitleBarMenuActions;
}

function shortcut(value: ShortcutConfig[keyof ShortcutConfig]) {
  return formatShortcut(value) || undefined;
}

export function createTitleBarMenus({
  activeView,
  shortcuts,
  canAccessTerminal,
  canCopyTerminal,
  canPasteTerminal,
  labels,
  actions,
}: CreateTitleBarMenusOptions): Record<TitleBarMenuKey, PopoverMenuItem[]> {
  return {
    file: [
      {
        key: "new_connection",
        label: labels.newConnection,
        shortcut: shortcut(shortcuts.new_connection),
        action: actions.openConnection,
      },
      {
        key: "new_window",
        label: labels.newWindow,
        shortcut: shortcut(shortcuts.new_window),
        action: actions.openWindow,
      },
      { key: "separator-new", separator: true },
      {
        key: "session_logs",
        label: labels.sessionLogs,
        active: activeView === "logs",
        action: actions.openSessionLogs,
      },
      {
        key: "settings",
        label: labels.settings,
        shortcut: shortcut(shortcuts.open_settings),
        active: activeView === "settings",
        action: actions.openSettings,
      },
      { key: "separator-exit", separator: true },
      {
        key: "exit",
        label: labels.exit,
        shortcut: shortcut(shortcuts.exit),
        action: actions.exit,
      },
    ],
    help: [
      { key: "documentation", label: labels.documentation, action: actions.openDocumentation },
      { key: "check_updates", label: labels.checkUpdates, action: actions.checkUpdates },
      { key: "separator-about", separator: true },
      { key: "about", label: labels.about, action: actions.openAbout },
    ],
    edit: [
      {
        key: "terminal_select_all",
        label: labels.selectAll,
        shortcut: shortcut(shortcuts.terminal_select_all),
        disabled: !canAccessTerminal,
        action: actions.selectAll,
      },
      {
        key: "terminal_copy",
        label: labels.copy,
        shortcut: shortcut(shortcuts.terminal_copy),
        disabled: !canCopyTerminal,
        action: actions.copy,
      },
      {
        key: "terminal_paste",
        label: labels.paste,
        shortcut: shortcut(shortcuts.terminal_paste),
        disabled: !canPasteTerminal,
        action: actions.paste,
      },
      { key: "separator-clear", separator: true },
      {
        key: "terminal_clear_viewport",
        label: labels.clearViewport,
        shortcut: shortcut(shortcuts.terminal_clear_viewport),
        disabled: !canAccessTerminal,
        action: actions.clearViewport,
      },
      {
        key: "terminal_clear_buffer",
        label: labels.clearBuffer,
        shortcut: shortcut(shortcuts.terminal_clear_buffer),
        disabled: !canAccessTerminal,
        action: actions.clearBuffer,
      },
    ],
  };
}
