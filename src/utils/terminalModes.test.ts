import { describe, expect, it } from "vitest";
import {
  DEFAULT_TERMINAL_MODE,
  normalizeTerminalMode,
  TERMINAL_MODE_OPTIONS,
} from "./terminalModes";

describe("terminal modes", () => {
  it("lists network operating system modes", () => {
    expect(TERMINAL_MODE_OPTIONS).toContainEqual({
      labelKey: "terminal_mode.arista_eos",
      value: "arista_eos",
    });
    expect(TERMINAL_MODE_OPTIONS).toContainEqual({
      labelKey: "terminal_mode.vyos",
      value: "vyos",
    });
    expect(TERMINAL_MODE_OPTIONS).toContainEqual({
      labelKey: "terminal_mode.fujitsu_sir",
      value: "fujitsu_sir",
    });
    expect(TERMINAL_MODE_OPTIONS).toContainEqual({
      labelKey: "terminal_mode.allied_telesis_awplus",
      value: "allied_telesis_awplus",
    });
  });

  it("normalizes device modes and preserves the existing fallback", () => {
    expect(normalizeTerminalMode("arista_eos")).toBe("arista_eos");
    expect(normalizeTerminalMode("vyos")).toBe("vyos");
    expect(normalizeTerminalMode("fujitsu_sir")).toBe("fujitsu_sir");
    expect(normalizeTerminalMode("allied_telesis_awplus")).toBe("allied_telesis_awplus");
    expect(normalizeTerminalMode("unknown")).toBe(DEFAULT_TERMINAL_MODE);
  });
});
