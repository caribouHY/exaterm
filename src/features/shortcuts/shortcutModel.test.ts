import { describe, expect, it } from "vitest";
import {
  DEFAULT_SHORTCUT_CONFIG,
  SHORTCUT_ACTIONS,
  captureShortcut,
  findShortcutAction,
  findShortcutConflict,
  formatShortcut,
  matchesShortcut,
  normalizeShortcutConfig,
  normalizeShortcutKey,
} from "./shortcutModel";

function keyEvent(
  key: string,
  modifiers: Partial<{
    ctrlKey: boolean;
    altKey: boolean;
    shiftKey: boolean;
    metaKey: boolean;
    isComposing: boolean;
  }> = {}
) {
  return {
    key,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...modifiers,
  };
}

describe("shortcutModel", () => {
  it("normalizes keys and formats modifiers in a stable order", () => {
    expect(normalizeShortcutKey("N")).toBe("n");
    expect(normalizeShortcutKey(" ")).toBe("Space");
    expect(normalizeShortcutKey("f12")).toBe("F12");
    expect(formatShortcut({ key: "n", ctrl: true, alt: true, shift: true })).toBe(
      "Ctrl+Alt+Shift+N"
    );
  });

  it("requires an exact modifier match", () => {
    const binding = DEFAULT_SHORTCUT_CONFIG.new_connection;
    expect(matchesShortcut(binding, keyEvent("N", { ctrlKey: true }))).toBe(true);
    expect(matchesShortcut(binding, keyEvent("N", { ctrlKey: true, shiftKey: true }))).toBe(false);
  });

  it("keeps new connection and new window shortcuts distinct", () => {
    expect(
      findShortcutAction(DEFAULT_SHORTCUT_CONFIG, keyEvent("n", { ctrlKey: true }), "application")
    ).toBe("new_connection");
    expect(
      findShortcutAction(
        DEFAULT_SHORTCUT_CONFIG,
        keyEvent("N", { ctrlKey: true, shiftKey: true }),
        "application"
      )
    ).toBe("new_window");
  });

  it("defines exit as an unassigned application shortcut", () => {
    expect(DEFAULT_SHORTCUT_CONFIG.exit).toBeNull();
    expect(SHORTCUT_ACTIONS.find(({ id }) => id === "exit")?.scope).toBe("application");

    const shortcuts = {
      ...DEFAULT_SHORTCUT_CONFIG,
      exit: { key: "q", ctrl: true, alt: false, shift: true },
    };
    const exitEvent = keyEvent("Q", { ctrlKey: true, shiftKey: true });
    expect(findShortcutAction(shortcuts, exitEvent, "application")).toBe("exit");
    expect(findShortcutAction(shortcuts, exitEvent, "terminal")).toBeNull();
    expect(
      findShortcutConflict(shortcuts, "new_connection", {
        key: "Q",
        ctrl: true,
        alt: false,
        shift: true,
      })
    ).toBe("exit");
  });

  it("keeps terminal shortcuts scoped to the terminal", () => {
    const copyEvent = keyEvent("C", { ctrlKey: true, shiftKey: true });
    const logStartEvent = keyEvent("F9", { ctrlKey: true, shiftKey: true });

    expect(findShortcutAction(DEFAULT_SHORTCUT_CONFIG, copyEvent, "terminal")).toBe(
      "terminal_copy"
    );
    expect(findShortcutAction(DEFAULT_SHORTCUT_CONFIG, copyEvent, "application")).toBeNull();
    expect(findShortcutAction(DEFAULT_SHORTCUT_CONFIG, logStartEvent, "terminal")).toBe(
      "terminal_log_start_overwrite"
    );
    expect(findShortcutAction(DEFAULT_SHORTCUT_CONFIG, logStartEvent, "application")).toBeNull();
    expect(
      findShortcutAction(DEFAULT_SHORTCUT_CONFIG, keyEvent("c", { ctrlKey: true }), "terminal")
    ).toBeNull();

    const shortcuts = {
      ...DEFAULT_SHORTCUT_CONFIG,
      terminal_clear_viewport: { key: "k", ctrl: true, alt: false, shift: true },
      terminal_clear_buffer: { key: "F8", ctrl: true, alt: false, shift: true },
    };
    expect(
      findShortcutAction(shortcuts, keyEvent("K", { ctrlKey: true, shiftKey: true }), "terminal")
    ).toBe("terminal_clear_viewport");
    expect(
      findShortcutAction(shortcuts, keyEvent("F8", { ctrlKey: true, shiftKey: true }), "terminal")
    ).toBe("terminal_clear_buffer");
    expect(
      findShortcutAction(shortcuts, keyEvent("K", { ctrlKey: true, shiftKey: true }), "application")
    ).toBeNull();
  });

  it("defines terminal clear actions as unassigned by default", () => {
    expect(DEFAULT_SHORTCUT_CONFIG.terminal_clear_viewport).toBeNull();
    expect(DEFAULT_SHORTCUT_CONFIG.terminal_clear_buffer).toBeNull();
    expect(
      SHORTCUT_ACTIONS.filter(({ id }) => id.startsWith("terminal_clear_")).map(
        ({ id, scope }) => ({ id, scope })
      )
    ).toEqual([
      { id: "terminal_clear_viewport", scope: "terminal" },
      { id: "terminal_clear_buffer", scope: "terminal" },
    ]);

    expect(
      findShortcutConflict(DEFAULT_SHORTCUT_CONFIG, "terminal_clear_viewport", {
        key: "n",
        ctrl: true,
        alt: false,
        shift: false,
      })
    ).toBe("new_connection");
  });

  it("defines all log actions in terminal scope with only start and stop assigned by default", () => {
    expect(
      SHORTCUT_ACTIONS.filter(({ id }) => id.startsWith("terminal_log_")).map(({ id, scope }) => ({
        id,
        scope,
      }))
    ).toEqual([
      { id: "terminal_log_start_overwrite", scope: "terminal" },
      { id: "terminal_log_start_append", scope: "terminal" },
      { id: "terminal_log_stop", scope: "terminal" },
      { id: "terminal_log_pause", scope: "terminal" },
      { id: "terminal_log_resume", scope: "terminal" },
    ]);
    expect(DEFAULT_SHORTCUT_CONFIG.terminal_log_start_overwrite).toEqual({
      key: "F9",
      ctrl: true,
      alt: false,
      shift: true,
    });
    expect(DEFAULT_SHORTCUT_CONFIG.terminal_log_start_append).toBeNull();
    expect(DEFAULT_SHORTCUT_CONFIG.terminal_log_stop).toEqual({
      key: "F10",
      ctrl: true,
      alt: false,
      shift: true,
    });
    expect(DEFAULT_SHORTCUT_CONFIG.terminal_log_pause).toBeNull();
    expect(DEFAULT_SHORTCUT_CONFIG.terminal_log_resume).toBeNull();
  });

  it("captures standalone function keys and rejects standalone characters", () => {
    expect(captureShortcut(keyEvent("F1"))).toEqual({
      kind: "binding",
      binding: { key: "F1", ctrl: false, alt: false, shift: false },
    });
    expect(captureShortcut(keyEvent("n"))).toEqual({ kind: "invalid" });
  });

  it("uses Escape to cancel and Backspace to clear", () => {
    expect(captureShortcut(keyEvent("Escape"))).toEqual({ kind: "cancel" });
    expect(captureShortcut(keyEvent("Backspace"))).toEqual({ kind: "clear" });
  });

  it("rejects Meta shortcuts, composition events, and Alt+F4", () => {
    expect(captureShortcut(keyEvent("n", { metaKey: true }))).toEqual({ kind: "invalid" });
    expect(captureShortcut(keyEvent("n", { ctrlKey: true, isComposing: true }))).toEqual({
      kind: "invalid",
    });
    expect(captureShortcut(keyEvent("F4", { altKey: true }))).toEqual({ kind: "invalid" });
  });

  it("detects conflicts while ignoring the edited action", () => {
    expect(
      findShortcutConflict(DEFAULT_SHORTCUT_CONFIG, "open_settings", {
        key: "N",
        ctrl: true,
        alt: false,
        shift: false,
      })
    ).toBe("new_connection");
    expect(
      findShortcutConflict(DEFAULT_SHORTCUT_CONFIG, "new_connection", {
        key: "N",
        ctrl: true,
        alt: false,
        shift: false,
      })
    ).toBeNull();
    expect(
      findShortcutConflict(DEFAULT_SHORTCUT_CONFIG, "new_connection", {
        key: "C",
        ctrl: true,
        alt: false,
        shift: true,
      })
    ).toBe("terminal_copy");
  });

  it("fills missing actions without replacing explicit null assignments", () => {
    const normalized = normalizeShortcutConfig({
      new_connection: null,
      new_window: { key: "F2", ctrl: false, alt: false, shift: false },
      terminal_copy: null,
    });
    expect(normalized.new_connection).toBeNull();
    expect(normalized.new_window?.key).toBe("F2");
    expect(normalized.open_settings).toEqual(DEFAULT_SHORTCUT_CONFIG.open_settings);
    expect(normalized.exit).toBeNull();
    expect(normalized.terminal_select_all).toEqual(DEFAULT_SHORTCUT_CONFIG.terminal_select_all);
    expect(normalized.terminal_copy).toBeNull();
    expect(normalized.terminal_paste).toEqual(DEFAULT_SHORTCUT_CONFIG.terminal_paste);
    expect(normalized.terminal_clear_viewport).toBeNull();
    expect(normalized.terminal_clear_buffer).toBeNull();
    expect(normalized.terminal_log_start_overwrite).toEqual(
      DEFAULT_SHORTCUT_CONFIG.terminal_log_start_overwrite
    );
    expect(normalized.terminal_log_start_append).toBeNull();
    expect(normalized.terminal_log_stop).toEqual(DEFAULT_SHORTCUT_CONFIG.terminal_log_stop);
    expect(normalized.terminal_log_pause).toBeNull();
    expect(normalized.terminal_log_resume).toBeNull();
  });

  it("preserves explicit null terminal clear shortcuts", () => {
    const normalized = normalizeShortcutConfig({
      terminal_clear_viewport: null,
      terminal_clear_buffer: null,
    });

    expect(normalized.terminal_clear_viewport).toBeNull();
    expect(normalized.terminal_clear_buffer).toBeNull();
  });

  it("preserves explicit null log shortcuts and detects conflicts with their defaults", () => {
    const normalized = normalizeShortcutConfig({
      terminal_log_start_overwrite: null,
      terminal_log_stop: null,
    });
    expect(normalized.terminal_log_start_overwrite).toBeNull();
    expect(normalized.terminal_log_stop).toBeNull();

    expect(
      findShortcutConflict(DEFAULT_SHORTCUT_CONFIG, "terminal_log_start_append", {
        key: "f9",
        ctrl: true,
        alt: false,
        shift: true,
      })
    ).toBe("terminal_log_start_overwrite");
  });

  it("preserves an explicit null exit shortcut", () => {
    expect(normalizeShortcutConfig({ exit: null }).exit).toBeNull();
  });
});
