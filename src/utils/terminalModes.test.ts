import { describe, expect, it } from "vitest";
import {
  DEFAULT_TERMINAL_MODE,
  normalizeTerminalMode,
  TERMINAL_MODE_OPTIONS,
} from "./terminalModes";

describe("terminal modes", () => {
  it("lists Arista EOS as a supported mode", () => {
    expect(TERMINAL_MODE_OPTIONS).toContainEqual({
      labelKey: "terminal_mode.arista_eos",
      value: "arista_eos",
    });
  });

  it("normalizes Arista EOS and preserves the existing fallback", () => {
    expect(normalizeTerminalMode("arista_eos")).toBe("arista_eos");
    expect(normalizeTerminalMode("unknown")).toBe(DEFAULT_TERMINAL_MODE);
  });
});
