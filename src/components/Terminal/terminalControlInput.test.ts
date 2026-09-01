import { describe, expect, it } from "vitest";
import { getTerminalControlInput } from "./terminalControlInput";

const defaultEvent = {
  altKey: false,
  code: "Digit6",
  ctrlKey: true,
  metaKey: false,
  shiftKey: true,
};

describe("getTerminalControlInput", () => {
  it("maps Ctrl+Shift+Digit6 to the record separator control byte", () => {
    expect(getTerminalControlInput(defaultEvent)).toBe("\x1e");
  });

  it.each([
    { ...defaultEvent, code: "Digit5" },
    { ...defaultEvent, ctrlKey: false },
    { ...defaultEvent, shiftKey: false },
    { ...defaultEvent, altKey: true },
    { ...defaultEvent, metaKey: true },
  ])("does not map other key combinations", (event) => {
    expect(getTerminalControlInput(event)).toBeNull();
  });
});
