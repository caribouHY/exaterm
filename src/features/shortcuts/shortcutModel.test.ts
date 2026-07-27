import { describe, expect, it } from "vitest";
import {
  DEFAULT_SHORTCUT_CONFIG,
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

  it("keeps terminal shortcuts scoped to the terminal", () => {
    const copyEvent = keyEvent("C", { ctrlKey: true, shiftKey: true });

    expect(findShortcutAction(DEFAULT_SHORTCUT_CONFIG, copyEvent, "terminal")).toBe(
      "terminal_copy"
    );
    expect(findShortcutAction(DEFAULT_SHORTCUT_CONFIG, copyEvent, "application")).toBeNull();
    expect(
      findShortcutAction(DEFAULT_SHORTCUT_CONFIG, keyEvent("c", { ctrlKey: true }), "terminal")
    ).toBeNull();
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
    expect(normalized.terminal_select_all).toEqual(DEFAULT_SHORTCUT_CONFIG.terminal_select_all);
    expect(normalized.terminal_copy).toBeNull();
    expect(normalized.terminal_paste).toEqual(DEFAULT_SHORTCUT_CONFIG.terminal_paste);
  });
});
