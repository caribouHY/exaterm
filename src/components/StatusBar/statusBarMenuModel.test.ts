import { describe, expect, it } from "vitest";
import {
  filterStatusBarPaletteItems,
  getStatusBarPaletteLabelSegments,
  moveStatusBarPaletteSelection,
  resolveStatusBarPaletteSelection,
  type StatusBarPaletteItem,
} from "./statusBarMenuModel";

const action = () => {};

function item(
  key: string,
  label: string,
  options: Pick<StatusBarPaletteItem, "active" | "disabled" | "searchLabel"> = {}
): StatusBarPaletteItem {
  return { key, label, action, ...options };
}

describe("filterStatusBarPaletteItems", () => {
  const items = [item("utf8", "UTF-8"), item("shift_jis", "Shift-JIS"), item("euc_jp", "EUC-JP")];

  it("returns the original order for an empty query", () => {
    expect(filterStatusBarPaletteItems(items, "  ")).toEqual(items);
  });

  it("filters labels with a case-insensitive partial match", () => {
    expect(filterStatusBarPaletteItems(items, "jis").map((entry) => entry.key)).toEqual([
      "shift_jis",
    ]);
  });

  it("filters terminal modes by their appended CLI value", () => {
    const terminalModes = [
      item("cisco_ios", "Cisco IOS (cisco-ios)"),
      item("arista_eos", "Arista EOS (arista-eos)"),
    ];
    expect(
      filterStatusBarPaletteItems(terminalModes, "arista-eos").map((entry) => entry.key)
    ).toEqual(["arista_eos"]);
  });

  it("filters bilingual log actions by their English search label only", () => {
    const logActions = [
      item("start", "手動ログを開始", { searchLabel: "Start manual log" }),
      item("stop", "手動ログを停止", { searchLabel: "Stop manual log" }),
    ];

    expect(filterStatusBarPaletteItems(logActions, "start").map((entry) => entry.key)).toEqual([
      "start",
    ]);
    expect(filterStatusBarPaletteItems(logActions, "開始")).toEqual([]);
  });

  it("returns an empty list when no labels match", () => {
    expect(filterStatusBarPaletteItems(items, "ascii")).toEqual([]);
  });

  it("does not include the current-state marker in search", () => {
    const itemsWithCurrent = [item("general", "General", { active: true })];
    expect(filterStatusBarPaletteItems(itemsWithCurrent, "current")).toEqual([]);
  });
});

describe("getStatusBarPaletteLabelSegments", () => {
  it("returns an unmarked label for an empty query", () => {
    expect(getStatusBarPaletteLabelSegments("Shift-JIS", "  ")).toEqual([
      { text: "Shift-JIS", matched: false },
    ]);
  });

  it("marks every case-insensitive match while preserving the original text", () => {
    expect(getStatusBarPaletteLabelSegments("Shift-JIS Shift", "shift")).toEqual([
      { text: "Shift", matched: true },
      { text: "-JIS ", matched: false },
      { text: "Shift", matched: true },
    ]);
  });

  it("marks matches in Japanese labels", () => {
    expect(getStatusBarPaletteLabelSegments("手動ログ操作を開始", "ログ")).toEqual([
      { text: "手動", matched: false },
      { text: "ログ", matched: true },
      { text: "操作を開始", matched: false },
    ]);
  });

  it("marks matches in an appended CLI value", () => {
    expect(getStatusBarPaletteLabelSegments("Cisco IOS (cisco-ios)", "cisco-ios")).toEqual([
      { text: "Cisco IOS (", matched: false },
      { text: "cisco-ios", matched: true },
      { text: ")", matched: false },
    ]);
  });
});

describe("resolveStatusBarPaletteSelection", () => {
  it("keeps an enabled preferred item", () => {
    const items = [item("first", "First", { active: true }), item("second", "Second")];
    expect(resolveStatusBarPaletteSelection(items, "second")).toBe("second");
  });

  it("selects the active item when the preferred item is unavailable", () => {
    const items = [
      item("disabled", "Disabled", { disabled: true }),
      item("active", "Active", { active: true }),
      item("other", "Other"),
    ];
    expect(resolveStatusBarPaletteSelection(items, "disabled")).toBe("active");
  });

  it("falls back to the first enabled item", () => {
    const items = [item("disabled", "Disabled", { disabled: true }), item("enabled", "Enabled")];
    expect(resolveStatusBarPaletteSelection(items, null)).toBe("enabled");
  });

  it("returns null when no item is enabled", () => {
    const items = [item("disabled", "Disabled", { disabled: true })];
    expect(resolveStatusBarPaletteSelection(items, null)).toBeNull();
  });
});

describe("moveStatusBarPaletteSelection", () => {
  const items = [
    item("first", "First"),
    item("disabled", "Disabled", { disabled: true }),
    item("last", "Last"),
  ];

  it("skips disabled items and wraps forward", () => {
    expect(moveStatusBarPaletteSelection(items, "first", "next")).toBe("last");
    expect(moveStatusBarPaletteSelection(items, "last", "next")).toBe("first");
  });

  it("skips disabled items and wraps backward", () => {
    expect(moveStatusBarPaletteSelection(items, "last", "previous")).toBe("first");
    expect(moveStatusBarPaletteSelection(items, "first", "previous")).toBe("last");
  });
});
