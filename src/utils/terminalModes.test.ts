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
  });

  it("normalizes device modes and preserves the existing fallback", () => {
    expect(normalizeTerminalMode("arista_eos")).toBe("arista_eos");
    expect(normalizeTerminalMode("vyos")).toBe("vyos");
    expect(normalizeTerminalMode("unknown")).toBe(DEFAULT_TERMINAL_MODE);
  });
});
