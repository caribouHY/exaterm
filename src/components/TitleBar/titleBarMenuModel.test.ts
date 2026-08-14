import { describe, expect, it, vi } from "vitest";
import { DEFAULT_SHORTCUT_CONFIG } from "../../features/shortcuts/shortcutModel";
import type { PopoverMenuItem } from "../Common";
import { createTitleBarMenus } from "./titleBarMenuModel";

type ActionMenuItem = Exclude<PopoverMenuItem, { key: string; separator: true }>;

const labels = {
  newConnection: "New Connection",
  newWindow: "New Window",
  sessionLogs: "Session Logs",
  settings: "Settings",
  checkUpdates: "Check for Updates",
  exit: "Exit",
  selectAll: "Select All",
  copy: "Copy",
  paste: "Paste",
  clearViewport: "Clear Display",
  clearBuffer: "Clear Buffer",
};

function createActions() {
  return {
    openConnection: vi.fn(),
    openWindow: vi.fn(),
    openSessionLogs: vi.fn(),
    openSettings: vi.fn(),
    checkUpdates: vi.fn(),
    exit: vi.fn(),
    selectAll: vi.fn(),
    copy: vi.fn(),
    paste: vi.fn(),
    clearViewport: vi.fn(),
    clearBuffer: vi.fn(),
  };
}

function actionItems(items: PopoverMenuItem[]) {
  return items.filter((item): item is ActionMenuItem => !("separator" in item));
}

function findAction(items: PopoverMenuItem[], key: string) {
  const item = items.find((candidate) => candidate.key === key);
  if (!item || "separator" in item) {
    throw new Error(`Missing menu action: ${key}`);
  }
  return item;
}

describe("createTitleBarMenus", () => {
  it("separates file navigation from terminal editing", () => {
    const menus = createTitleBarMenus({
      activeView: "logs",
      shortcuts: DEFAULT_SHORTCUT_CONFIG,
      canAccessTerminal: true,
      canCopyTerminal: true,
      canPasteTerminal: true,
      labels,
      actions: createActions(),
    });

    expect(actionItems(menus.file).map((item) => item.key)).toEqual([
      "new_connection",
      "new_window",
      "session_logs",
      "settings",
      "check_updates",
      "exit",
    ]);
    expect(actionItems(menus.edit).map((item) => item.key)).toEqual([
      "terminal_select_all",
      "terminal_copy",
      "terminal_paste",
      "terminal_clear_viewport",
      "terminal_clear_buffer",
    ]);
    expect(findAction(menus.file, "session_logs").active).toBe(true);
    expect(findAction(menus.file, "settings").active).toBe(false);
    expect(findAction(menus.edit, "terminal_select_all").shortcut).toBe("Ctrl+Shift+A");
  });

  it("disables terminal actions when there is no active terminal", () => {
    const menus = createTitleBarMenus({
      activeView: "settings",
      shortcuts: DEFAULT_SHORTCUT_CONFIG,
      canAccessTerminal: false,
      canCopyTerminal: false,
      canPasteTerminal: false,
      labels,
      actions: createActions(),
    });

    expect(actionItems(menus.edit).every((item) => item.disabled)).toBe(true);
  });

  it("tracks selection and connection capabilities independently", () => {
    const menus = createTitleBarMenus({
      activeView: "terminal",
      shortcuts: DEFAULT_SHORTCUT_CONFIG,
      canAccessTerminal: true,
      canCopyTerminal: false,
      canPasteTerminal: false,
      labels,
      actions: createActions(),
    });

    expect(findAction(menus.edit, "terminal_select_all").disabled).toBe(false);
    expect(findAction(menus.edit, "terminal_copy").disabled).toBe(true);
    expect(findAction(menus.edit, "terminal_paste").disabled).toBe(true);
    expect(findAction(menus.edit, "terminal_clear_viewport").disabled).toBe(false);
    expect(findAction(menus.edit, "terminal_clear_buffer").disabled).toBe(false);
  });
});
