import { describe, expect, it } from "vitest";
import {
  DEFAULT_TERMINAL_MODE,
  getTerminalModeOptions,
  isTerminalMode,
  normalizeTerminalMode,
  TERMINAL_MODE_CATALOG,
} from "./terminalModes";

describe("terminal modes", () => {
  it("keeps general first and sorts device modes by their stable English names", () => {
    const labelsByKey = new Map(
      TERMINAL_MODE_CATALOG.map((definition) => [definition.labelKey, definition.sortName])
    );

    expect(getTerminalModeOptions((labelKey) => labelsByKey.get(labelKey) ?? labelKey)).toEqual([
      { label: "General", value: "general" },
      { label: "Allied Telesis AW+", value: "allied_telesis_awplus" },
      { label: "Arista EOS", value: "arista_eos" },
      { label: "Cisco IOS", value: "cisco_ios" },
      { label: "Fujitsu Si-R", value: "fujitsu_sir" },
      { label: "Furukawa FITELnet", value: "furukawa_fitelnet" },
      { label: "VyOS", value: "vyos" },
    ]);
  });

  it("derives valid modes and normalization from the catalog", () => {
    TERMINAL_MODE_CATALOG.forEach(({ value }) => {
      expect(isTerminalMode(value)).toBe(true);
      expect(normalizeTerminalMode(value)).toBe(value);
    });

    expect(isTerminalMode("unknown")).toBe(false);
    expect(normalizeTerminalMode("unknown")).toBe(DEFAULT_TERMINAL_MODE);
    expect(normalizeTerminalMode(null)).toBe(DEFAULT_TERMINAL_MODE);
    expect(normalizeTerminalMode(undefined)).toBe(DEFAULT_TERMINAL_MODE);
  });
});
