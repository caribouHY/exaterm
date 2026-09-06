import { describe, expect, it } from "vitest";
import { finalizeBoundedInteger, parseBoundedInteger } from "./terminalSettingsModel";

describe("terminal settings numeric input", () => {
  it("accepts an in-range integer while it is being edited", () => {
    expect(parseBoundedInteger("16", 8, 32)).toBe(16);
  });

  it.each(["", "1", "33", "15.5", "abc"])(
    "does not update the setting from an incomplete or out-of-range value: %s",
    (value) => {
      expect(parseBoundedInteger(value, 8, 32)).toBeNull();
    }
  );

  it.each([
    ["1", 15, 8, 32, 8],
    ["33", 15, 8, 32, 32],
    ["", 15, 8, 32, 15],
    ["15.5", 15, 8, 32, 15],
    ["99", 10_000, 100, 100_000, 100],
    ["100001", 10_000, 100, 100_000, 100_000],
  ])("finalizes %s as %d", (value, currentValue, min, max, expected) => {
    expect(finalizeBoundedInteger(value, currentValue, min, max)).toBe(expected);
  });
});
