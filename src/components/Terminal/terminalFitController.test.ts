import { Terminal } from "@xterm/xterm";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createTerminalFitController } from "./terminalFitController";

afterEach(() => {
  vi.useRealTimers();
});

function createHarness(resize?: (cols: number, rows: number) => void) {
  const state = { active: true, cols: 80, rows: 24 };
  const container = { clientWidth: 800, clientHeight: 480 };
  const fitAddon = { fit: vi.fn(() => resize?.(state.cols, state.rows)) };
  const onFit = vi.fn();
  const controller = createTerminalFitController({
    container,
    fitAddon,
    isActive: () => state.active,
    onFit,
  });
  return { state, container, fitAddon, onFit, controller };
}

function readTerminal(terminal: Terminal) {
  const buffer = terminal.buffer.active;
  return {
    lines: Array.from({ length: buffer.length }, (_, index) =>
      buffer.getLine(index)?.translateToString(true)
    ),
    cursorX: buffer.cursorX,
    cursorY: buffer.cursorY,
    baseY: buffer.baseY,
    viewportY: buffer.viewportY,
    cols: terminal.cols,
    rows: terminal.rows,
  };
}

const write = (terminal: Terminal, text: string) =>
  new Promise<void>((resolve) => terminal.write(text, resolve));

describe("terminalFitController", () => {
  it("preserves the actual xterm cursor line through repeated hidden fits", async () => {
    const terminal = new Terminal({ cols: 80, rows: 24, allowProposedApi: true });
    const { state, controller, fitAddon } = createHarness((cols, rows) =>
      terminal.resize(cols, rows)
    );
    try {
      await write(terminal, "SYNTHETIC history\r\nSYNTHETIC> pending-command");
      controller.fit();
      const before = readTerminal(terminal);

      for (let index = 0; index < 10; index += 1) {
        state.active = false;
        state.cols = 2;
        state.rows = 1;
        expect(controller.fit()).toBe(false);
        expect(readTerminal(terminal)).toEqual(before);

        state.active = true;
        state.cols = 80;
        state.rows = 24;
        expect(controller.fit()).toBe(true);
        expect(readTerminal(terminal)).toEqual(before);
      }
      expect(fitAddon.fit).toHaveBeenCalledTimes(11);
    } finally {
      controller.dispose();
      terminal.dispose();
    }
  });

  it("retains output received while hidden and fits normally after returning", async () => {
    const terminal = new Terminal({ cols: 80, rows: 24, allowProposedApi: true });
    const { state, container, controller } = createHarness((cols, rows) =>
      terminal.resize(cols, rows)
    );
    try {
      await write(terminal, "SYNTHETIC> partial");
      state.active = false;
      container.clientWidth = 0;
      container.clientHeight = 0;
      state.cols = 2;
      state.rows = 1;
      controller.fit();
      await write(terminal, "-received-while-hidden");
      const hidden = readTerminal(terminal);
      expect(hidden.lines[0]).toBe("SYNTHETIC> partial-received-while-hidden");
      expect(hidden.cursorX).toBe(hidden.lines[0]?.length);
      expect([terminal.cols, terminal.rows]).toEqual([80, 24]);

      state.active = true;
      container.clientWidth = 1000;
      container.clientHeight = 600;
      state.cols = 100;
      state.rows = 30;
      expect(controller.fit()).toBe(true);
      expect(readTerminal(terminal).lines[0]).toBe(hidden.lines[0]);
      expect(terminal.buffer.active.cursorX).toBe(hidden.cursorX);
      expect(terminal.buffer.active.cursorY).toBe(hidden.cursorY);
      expect([terminal.cols, terminal.rows]).toEqual([100, 30]);
    } finally {
      controller.dispose();
      terminal.dispose();
    }
  });

  it.each([
    { active: false, width: 800, height: 480 },
    { active: true, width: 0, height: 480 },
    { active: true, width: 800, height: 0 },
  ])("skips fitting and resize side effects for $active / $width / $height", (size) => {
    const { state, container, fitAddon, onFit, controller } = createHarness();
    state.active = size.active;
    container.clientWidth = size.width;
    container.clientHeight = size.height;
    expect(controller.fit()).toBe(false);
    expect(fitAddon.fit).not.toHaveBeenCalled();
    expect(onFit).not.toHaveBeenCalled();
  });

  it.each(["inactive", "zero-width", "zero-height"])(
    "checks current visibility when a delayed fit runs: %s",
    (hiddenState) => {
      vi.useFakeTimers();
      const { state, container, fitAddon, onFit, controller } = createHarness();
      const focus = vi.fn();
      controller.schedule(focus);
      if (hiddenState === "inactive") state.active = false;
      if (hiddenState === "zero-width") container.clientWidth = 0;
      if (hiddenState === "zero-height") container.clientHeight = 0;
      vi.advanceTimersByTime(50);
      expect(fitAddon.fit).not.toHaveBeenCalled();
      expect(onFit).not.toHaveBeenCalled();
      expect(focus).not.toHaveBeenCalled();
    }
  );

  it("cancels activation and config timers when hidden and refits after returning", () => {
    vi.useFakeTimers();
    const { state, fitAddon, onFit, controller } = createHarness();
    const focus = vi.fn();
    controller.schedule(focus);
    controller.schedule();
    state.active = false;
    controller.cancelPending();
    expect(vi.getTimerCount()).toBe(0);
    controller.schedule();
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(50);
    expect(fitAddon.fit).not.toHaveBeenCalled();
    expect(focus).not.toHaveBeenCalled();

    state.active = true;
    controller.schedule(focus);
    vi.advanceTimersByTime(50);
    expect(fitAddon.fit).toHaveBeenCalledOnce();
    expect(onFit).toHaveBeenCalledOnce();
    expect(focus).toHaveBeenCalledOnce();
  });

  it("cleans up one superseded config timer without cancelling another fit", () => {
    vi.useFakeTimers();
    const { fitAddon, controller } = createHarness();
    const cancelConfigFit = controller.schedule();
    const focus = vi.fn();
    controller.schedule(focus);
    cancelConfigFit();
    vi.advanceTimersByTime(50);
    expect(fitAddon.fit).toHaveBeenCalledOnce();
    expect(focus).toHaveBeenCalledOnce();
  });

  it("rejects pending and future fits after disposal", () => {
    vi.useFakeTimers();
    const { fitAddon, onFit, controller } = createHarness();
    const focus = vi.fn();
    controller.schedule(focus);
    controller.schedule();
    controller.dispose();
    controller.dispose();
    expect(vi.getTimerCount()).toBe(0);
    controller.schedule(focus);
    expect(controller.fit()).toBe(false);
    vi.advanceTimersByTime(50);
    expect(fitAddon.fit).not.toHaveBeenCalled();
    expect(onFit).not.toHaveBeenCalled();
    expect(focus).not.toHaveBeenCalled();
  });
});
