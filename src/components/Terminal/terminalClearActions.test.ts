import { describe, expect, it, vi } from "vitest";
import {
  CLEAR_TERMINAL_VIEWPORT_SEQUENCE,
  clearTerminalBuffer,
  clearTerminalViewport,
} from "./terminalClearActions";

function createTerminal() {
  let writeCallback: (() => void) | undefined;
  const terminal = {
    write: vi.fn((_data: string, callback?: () => void) => {
      writeCallback = callback;
    }),
    clear: vi.fn(),
    clearSelection: vi.fn(),
    focus: vi.fn(),
  };

  return {
    terminal,
    completeWrite: () => writeCallback?.(),
  };
}

describe("terminalClearActions", () => {
  it("clears only the local viewport with terminal control sequences", () => {
    const { terminal, completeWrite } = createTerminal();
    const onCleared = vi.fn();

    clearTerminalViewport(terminal, onCleared);

    expect(terminal.write).toHaveBeenCalledWith(
      CLEAR_TERMINAL_VIEWPORT_SEQUENCE,
      expect.any(Function)
    );
    expect(terminal.clear).not.toHaveBeenCalled();
    expect(terminal.clearSelection).not.toHaveBeenCalled();

    completeWrite();

    expect(terminal.clearSelection).toHaveBeenCalledOnce();
    expect(onCleared).toHaveBeenCalledOnce();
    expect(terminal.focus).toHaveBeenCalledOnce();
  });

  it("clears the xterm buffer and the retained current line", () => {
    const { terminal, completeWrite } = createTerminal();
    const onCleared = vi.fn();

    clearTerminalBuffer(terminal, onCleared);

    expect(terminal.clear).toHaveBeenCalledOnce();
    expect(terminal.write).toHaveBeenCalledWith(
      CLEAR_TERMINAL_VIEWPORT_SEQUENCE,
      expect.any(Function)
    );
    expect(terminal.clear.mock.invocationCallOrder[0]).toBeLessThan(
      terminal.write.mock.invocationCallOrder[0]
    );
    expect(terminal.clearSelection).not.toHaveBeenCalled();

    completeWrite();

    expect(terminal.clearSelection).toHaveBeenCalledOnce();
    expect(onCleared).toHaveBeenCalledOnce();
    expect(terminal.focus).toHaveBeenCalledOnce();
  });
});
