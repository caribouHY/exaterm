import { Terminal } from "@xterm/xterm";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createTerminalFitController } from "./terminalFitController";
import { createTerminalResizeController } from "./terminalResizeController";

const cleanup: (() => void)[] = [];

afterEach(() => {
  cleanup.splice(0).forEach((dispose) => {
    dispose();
  });
  vi.useRealTimers();
});

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const settle = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

function createHarness(protocol: "ssh" | "telnet" | "serial" = "ssh", connected = true) {
  const terminal = new Terminal({ cols: 80, rows: 24, allowProposedApi: true });
  const state = { active: true, cols: 80, rows: 24 };
  const container = { clientWidth: 800, clientHeight: 480 };
  const invoke = vi.fn<(command: string, args: object) => Promise<unknown>>(() =>
    Promise.resolve()
  );
  const resize = createTerminalResizeController({
    terminal,
    container,
    isActive: () => state.active,
    isConnected: connected,
    sendResize:
      protocol === "serial"
        ? null
        : (size) => invoke(`${protocol}_resize`, { sessionId: "synthetic-session", ...size }),
  });
  const fit = createTerminalFitController({
    container,
    // The Node test runner has no renderer; only cell measurement is substituted.
    fitAddon: {
      fit: () => {
        terminal.resize(state.cols, state.rows);
      },
    },
    isActive: () => state.active,
    onFit: resize.synchronize,
  });
  cleanup.push(() => {
    fit.dispose();
    resize.dispose();
    terminal.dispose();
  });
  return { terminal, state, container, invoke, resize, fit };
}

describe("terminalResizeController", () => {
  it.each(["ssh", "telnet"] as const)(
    "notifies %s after a fixed-container font fit without a DOM resize",
    async (protocol) => {
      vi.useFakeTimers();
      const { state, terminal, container, invoke, fit } = createHarness(protocol);
      fit.fit();
      await settle();
      invoke.mockClear();

      state.cols = 64;
      state.rows = 19;
      fit.schedule();
      vi.advanceTimersByTime(50);
      await settle();

      expect(container).toEqual({ clientWidth: 800, clientHeight: 480 });
      expect([terminal.cols, terminal.rows]).toEqual([64, 19]);
      expect(invoke).toHaveBeenCalledExactlyOnceWith(`${protocol}_resize`, {
        sessionId: "synthetic-session",
        cols: 64,
        rows: 19,
      });
      fit.fit();
      await settle();
      expect(invoke).toHaveBeenCalledTimes(1);
    }
  );

  it("synchronizes the initial size even when fit does not resize xterm", async () => {
    const { fit, invoke } = createHarness();
    fit.fit();
    fit.fit();
    await settle();
    fit.fit();
    expect(invoke).toHaveBeenCalledExactlyOnceWith("ssh_resize", {
      sessionId: "synthetic-session",
      cols: 80,
      rows: 24,
    });
  });

  it("sends the fitted size when a connection becomes ready", async () => {
    const { state, fit, invoke, resize } = createHarness("ssh", false);
    state.cols = 100;
    state.rows = 30;
    fit.fit();
    expect(invoke).not.toHaveBeenCalled();
    resize.setConnected(true);
    await settle();
    expect(invoke).toHaveBeenCalledExactlyOnceWith("ssh_resize", {
      sessionId: "synthetic-session",
      cols: 100,
      rows: 30,
    });
  });

  it("retains hidden output and synchronizes the latest fit on return", async () => {
    const { state, terminal, fit, invoke } = createHarness();
    fit.fit();
    await settle();
    invoke.mockClear();
    state.active = false;
    state.cols = 100;
    state.rows = 30;
    fit.schedule();
    expect(fit.fit()).toBe(false);
    await new Promise<void>((resolve) => {
      terminal.write("SYNTHETIC retained output", resolve);
    });
    expect(invoke).not.toHaveBeenCalled();
    expect([terminal.cols, terminal.rows]).toEqual([80, 24]);
    vi.useFakeTimers();
    state.active = true;
    fit.schedule();
    vi.advanceTimersByTime(50);
    await settle();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0][1]).toMatchObject({ cols: 100, rows: 30 });
    expect(terminal.buffer.active.getLine(0)?.translateToString(true)).toBe(
      "SYNTHETIC retained output"
    );
  });

  it.each(["inactive", "zero-width", "zero-height", "disconnected"])(
    "does not send xterm events while %s",
    (condition) => {
      const { state, container, terminal, resize, invoke } = createHarness();
      if (condition === "inactive") state.active = false;
      if (condition === "zero-width") container.clientWidth = 0;
      if (condition === "zero-height") container.clientHeight = 0;
      if (condition === "disconnected") resize.setConnected(false);
      terminal.resize(100, 30);
      resize.synchronize();
      expect(invoke).not.toHaveBeenCalled();
    }
  );

  it("never sends protocol data for Serial", async () => {
    const { state, fit, resize, invoke } = createHarness("serial");
    fit.fit();
    state.cols = 100;
    fit.fit();
    resize.setConnected(false);
    resize.setConnected(true);
    await settle();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("serializes requests and coalesces pending changes to the latest size", async () => {
    const { terminal, resize, invoke } = createHarness();
    const first = deferred();
    invoke.mockReturnValueOnce(first.promise);
    resize.synchronize();
    terminal.resize(100, 30);
    terminal.resize(120, 40);
    resize.synchronize();
    expect(invoke).toHaveBeenCalledTimes(1);
    first.resolve();
    await settle();
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke.mock.calls[1][1]).toMatchObject({ cols: 120, rows: 40 });
  });

  it("suppresses a pending size that returns to the successfully sent size", async () => {
    const { terminal, resize, invoke } = createHarness();
    const first = deferred();
    invoke.mockReturnValueOnce(first.promise);
    resize.synchronize();
    terminal.resize(100, 30);
    terminal.resize(80, 24);
    first.resolve();
    await settle();
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("retries failures only on another trigger, including an unchanged return fit", async () => {
    const { state, fit, invoke } = createHarness();
    const first = deferred();
    invoke.mockReturnValueOnce(first.promise);
    fit.fit();
    fit.fit();
    first.reject(new Error("synthetic failure"));
    await settle();
    expect(invoke).toHaveBeenCalledTimes(1);
    state.active = false;
    fit.fit();
    state.active = true;
    fit.fit();
    await settle();
    expect(invoke).toHaveBeenCalledTimes(2);
    fit.fit();
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("sends a newer pending size after a failed older request", async () => {
    const { terminal, resize, invoke } = createHarness();
    const first = deferred();
    invoke.mockReturnValueOnce(first.promise);
    resize.synchronize();
    terminal.resize(100, 30);
    first.reject(new Error("synthetic failure"));
    await settle();
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke.mock.calls[1][1]).toMatchObject({ cols: 100, rows: 30 });
  });

  it.each(["success", "failure"])(
    "ignores old-generation %s and resynchronizes after reconnect",
    async (result) => {
      const { resize, invoke } = createHarness();
      const first = deferred();
      invoke.mockReturnValueOnce(first.promise);
      resize.synchronize();
      resize.setConnected(false);
      resize.setConnected(true);
      expect(invoke).toHaveBeenCalledTimes(1);
      if (result === "success") first.resolve();
      else first.reject(new Error("synthetic old failure"));
      await settle();
      expect(invoke).toHaveBeenCalledTimes(2);
      resize.synchronize();
      await settle();
      expect(invoke).toHaveBeenCalledTimes(2);
    }
  );

  it.each(["hidden", "disconnected", "disposed"])(
    "does not drain pending requests after becoming %s",
    async (condition) => {
      const { terminal, state, resize, invoke } = createHarness();
      const first = deferred();
      invoke.mockReturnValueOnce(first.promise);
      resize.synchronize();
      terminal.resize(100, 30);
      if (condition === "hidden") state.active = false;
      if (condition === "disconnected") resize.setConnected(false);
      if (condition === "disposed") resize.dispose();
      first.resolve();
      await settle();
      expect(invoke).toHaveBeenCalledTimes(1);
      if (condition === "disposed") {
        terminal.resize(120, 40);
        resize.synchronize();
        resize.setConnected(false);
        resize.setConnected(true);
        expect(invoke).toHaveBeenCalledTimes(1);
      }
    }
  );

  it("does not let a disposed session completion affect its replacement", async () => {
    const old = createHarness();
    const first = deferred();
    old.invoke.mockReturnValueOnce(first.promise);
    old.resize.synchronize();
    old.resize.dispose();
    const current = createHarness();
    current.fit.fit();
    await settle();
    first.resolve();
    await settle();
    current.fit.fit();
    expect(old.invoke).toHaveBeenCalledTimes(1);
    expect(current.invoke).toHaveBeenCalledTimes(1);
  });
});
