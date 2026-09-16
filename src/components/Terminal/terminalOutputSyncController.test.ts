import { describe, expect, it, vi } from "vitest";
import {
  createTerminalOutputSyncController,
  type TerminalOutputEventPayload,
  type TerminalOutputSnapshot,
  type TerminalOutputSyncDependencies,
  type TerminalOutputUnlisten,
} from "./terminalOutputSyncController";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function outputSnapshot(
  output: string,
  cursor: number,
  overrides: Partial<TerminalOutputSnapshot> = {}
): TerminalOutputSnapshot {
  return {
    session_id: "session-1",
    output,
    truncated: false,
    available_chars: cursor,
    start_cursor: cursor - [...output].length,
    cursor,
    ...overrides,
  };
}

function createHarness(maxInitialDeltaDrains = 5) {
  const listeners: Array<{
    event: string;
    handler: (event: { payload: TerminalOutputEventPayload }) => void;
    registration: ReturnType<typeof deferred<TerminalOutputUnlisten>>;
  }> = [];
  const snapshots: Array<ReturnType<typeof deferred<TerminalOutputSnapshot>>> = [];
  const deltas: Array<{
    cursor: number;
    result: ReturnType<typeof deferred<TerminalOutputSnapshot>>;
  }> = [];
  const writes: string[] = [];

  const dependencies: TerminalOutputSyncDependencies = {
    listen: (event, handler) => {
      const registration = deferred<TerminalOutputUnlisten>();
      listeners.push({ event, handler, registration });
      return registration.promise;
    },
    getSnapshot: () => {
      const result = deferred<TerminalOutputSnapshot>();
      snapshots.push(result);
      return result.promise;
    },
    getDelta: (_sessionId, cursor) => {
      const result = deferred<TerminalOutputSnapshot>();
      deltas.push({ cursor, result });
      return result.promise;
    },
  };

  const controller = createTerminalOutputSyncController({
    sessionId: "session-1",
    encoding: "utf-8",
    maxChars: 100,
    channels: [
      { event: "data/session-1", replayedBySnapshot: true },
      { event: "error/session-1", replayedBySnapshot: false },
    ],
    write: (text) => writes.push(text),
    dependencies,
    maxInitialDeltaDrains,
  });

  return { controller, listeners, snapshots, deltas, writes };
}

const bytes = (text: string) => [...new TextEncoder().encode(text)];
const tick = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

describe("terminal output sync controller", () => {
  it("waits for every subscription and restores output that arrives during initial sync", async () => {
    const h = createHarness();
    const started = h.controller.start();
    const stopData = vi.fn();
    const stopError = vi.fn();

    expect(h.listeners.map(({ event }) => event)).toEqual(["data/session-1", "error/session-1"]);
    h.listeners[0].handler({ payload: bytes("A") });
    h.listeners[0].registration.resolve(stopData);
    await tick();
    expect(h.snapshots).toHaveLength(0);

    h.listeners[1].registration.resolve(stopError);
    await tick();
    expect(h.snapshots).toHaveLength(1);
    h.snapshots[0].resolve(outputSnapshot("historyA", 8));
    await tick();

    h.listeners[0].handler({ payload: bytes("B") });
    expect(h.deltas).toHaveLength(1);
    expect(h.deltas[0].cursor).toBe(8);
    h.deltas[0].result.resolve(outputSnapshot("B", 9));
    await tick();
    expect(h.deltas).toHaveLength(2);
    h.deltas[1].result.resolve(outputSnapshot("", 9));
    await started;

    h.listeners[0].handler({ payload: bytes("C") });
    expect(h.writes).toEqual(["historyA", "B", "C"]);
    h.controller.dispose();
    expect(stopData).toHaveBeenCalledOnce();
    expect(stopError).toHaveBeenCalledOnce();
  });

  it("bounds a continuous initial drain and then handles empty and continuing live output", async () => {
    const h = createHarness();
    const started = h.controller.start();
    h.listeners.forEach(({ registration }) => registration.resolve(vi.fn()));
    await tick();
    h.snapshots[0].resolve(outputSnapshot("base", 4));
    await tick();

    for (let attempt = 0; attempt < 5; attempt += 1) {
      const text = String(attempt + 1);
      h.listeners[0].handler({ payload: bytes(text) });
      h.deltas[attempt].result.resolve(outputSnapshot(text, 5 + attempt));
      await tick();
    }
    await started;

    expect(h.deltas).toHaveLength(5);
    h.listeners[0].handler({ payload: [] });
    h.listeners[0].handler({ payload: bytes("6") });
    expect(h.writes).toEqual(["base", "1", "2", "3", "4", "5", "6"]);
  });

  it("delivers non-replayable string errors after a successful restore", async () => {
    const h = createHarness();
    const started = h.controller.start();
    h.listeners.forEach(({ registration }) => registration.resolve(vi.fn()));
    await tick();
    h.snapshots[0].resolve(outputSnapshot("base", 4));
    await tick();
    h.listeners[1].handler({ payload: "serial read failed" });
    h.deltas[0].result.resolve(outputSnapshot("", 4));
    await started;

    expect(h.writes).toEqual(["base", "serial read failed"]);
  });

  it("falls back to registered live channels when one subscription fails", async () => {
    const h = createHarness();
    const started = h.controller.start();
    const stopData = vi.fn();
    h.listeners[0].handler({ payload: bytes("queued") });
    h.listeners[0].registration.resolve(stopData);
    h.listeners[1].registration.reject(new Error("registration failed"));
    await started;

    expect(h.snapshots).toHaveLength(0);
    expect(h.writes).toEqual(["queued"]);
    h.listeners[0].handler({ payload: bytes("live") });
    expect(h.writes).toEqual(["queued", "live"]);
    h.controller.dispose();
    h.controller.dispose();
    expect(stopData).toHaveBeenCalledOnce();
  });

  it("falls back to buffered events in arrival order when snapshot or delta retrieval fails", async () => {
    const snapshotFailure = createHarness();
    const snapshotStart = snapshotFailure.controller.start();
    snapshotFailure.listeners.forEach(({ registration }) => registration.resolve(vi.fn()));
    await tick();
    snapshotFailure.listeners[0].handler({ payload: bytes("data") });
    snapshotFailure.listeners[1].handler({ payload: "error" });
    snapshotFailure.snapshots[0].reject(new Error("snapshot failed"));
    await snapshotStart;
    expect(snapshotFailure.writes).toEqual(["data", "error"]);

    const deltaFailure = createHarness();
    const deltaStart = deltaFailure.controller.start();
    deltaFailure.listeners.forEach(({ registration }) => registration.resolve(vi.fn()));
    await tick();
    deltaFailure.snapshots[0].resolve(outputSnapshot("history", 7));
    await tick();
    deltaFailure.listeners[0].handler({ payload: bytes("pending") });
    deltaFailure.deltas[0].result.reject(new Error("delta failed"));
    await deltaStart;
    expect(deltaFailure.writes).toEqual(["history", "pending"]);
  });

  it("releases registrations that finish after dispose exactly once", async () => {
    const h = createHarness();
    const started = h.controller.start();
    const stopData = vi.fn(() => Promise.reject(new Error("cleanup failed")));
    const stopError = vi.fn();

    h.controller.dispose();
    h.controller.dispose();
    h.listeners[0].handler({ payload: bytes("ignored") });
    h.listeners[0].registration.resolve(stopData);
    h.listeners[1].registration.resolve(stopError);
    await started;
    await tick();

    expect(stopData).toHaveBeenCalledOnce();
    expect(stopError).toHaveBeenCalledOnce();
    expect(h.snapshots).toHaveLength(0);
    expect(h.writes).toEqual([]);
  });

  it("ignores late restore results and old callbacks after dispose", async () => {
    const h = createHarness();
    const started = h.controller.start();
    const unlisteners = [vi.fn(), vi.fn()];
    h.listeners.forEach(({ registration }, index) => registration.resolve(unlisteners[index]));
    await tick();

    h.controller.dispose();
    h.listeners[0].handler({ payload: bytes("old session") });
    h.snapshots[0].resolve(outputSnapshot("late snapshot", 13));
    await started;

    expect(h.deltas).toHaveLength(0);
    expect(h.writes).toEqual([]);
    unlisteners.forEach((unlisten) => expect(unlisten).toHaveBeenCalledOnce());
  });

  it("preserves split UTF-8 characters across live events", async () => {
    const h = createHarness();
    const started = h.controller.start();
    h.listeners.forEach(({ registration }) => registration.resolve(vi.fn()));
    await tick();
    h.snapshots[0].resolve(outputSnapshot("", 0));
    await tick();
    h.deltas[0].result.resolve(outputSnapshot("", 0));
    await started;

    const encoded = bytes("界");
    h.listeners[0].handler({ payload: encoded.slice(0, 1) });
    expect(h.writes).toEqual([]);
    h.listeners[0].handler({ payload: encoded.slice(1) });
    expect(h.writes).toEqual(["界"]);
  });

  it("switches the streaming decoder without restarting the session sync", async () => {
    const h = createHarness();
    const started = h.controller.start();
    h.listeners.forEach(({ registration }) => registration.resolve(vi.fn()));
    await tick();
    h.snapshots[0].resolve(outputSnapshot("", 0));
    await tick();
    h.deltas[0].result.resolve(outputSnapshot("", 0));
    await started;

    h.listeners[0].handler({ payload: [0xe7] });
    h.controller.setEncoding("shift-jis");
    h.listeners[0].handler({ payload: [0x82] });
    h.listeners[0].handler({ payload: [0xa0] });

    expect(h.writes).toEqual(["あ"]);
    expect(h.listeners).toHaveLength(2);
    expect(h.snapshots).toHaveLength(1);
  });

  it("forwards the bounded text returned by truncated snapshots and deltas", async () => {
    const h = createHarness();
    const started = h.controller.start();
    h.listeners.forEach(({ registration }) => registration.resolve(vi.fn()));
    await tick();
    h.snapshots[0].resolve(
      outputSnapshot("tail", 20, { truncated: true, available_chars: 20, start_cursor: 16 })
    );
    await tick();
    h.deltas[0].result.resolve(
      outputSnapshot("new-tail", 40, {
        truncated: true,
        available_chars: 40,
        start_cursor: 32,
      })
    );
    await started;

    expect(h.writes).toEqual(["tail", "new-tail"]);
  });
});
