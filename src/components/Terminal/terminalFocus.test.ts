import { afterEach, expect, it, vi } from "vitest";
import { focusTerminalUnlessModal } from "./terminalFocus";

afterEach(() => vi.unstubAllGlobals());

it("keeps focus in an open dialog while allowing the terminal without an overlay", () => {
  const focus = vi.fn();
  const querySelector = vi.fn((): object | null => ({}));
  vi.stubGlobal("document", { querySelector });
  focusTerminalUnlessModal(focus);
  expect(focus).not.toHaveBeenCalled();
  querySelector.mockReturnValue(null);
  focusTerminalUnlessModal(focus);
  expect(focus).toHaveBeenCalledTimes(1);
});
