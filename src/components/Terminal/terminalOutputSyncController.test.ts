import { describe, expect, it, vi } from "vitest";
import {
  createTerminalOutputSyncController,
  type TerminalOutputEvent,
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

const tick = async () => {
  for (let i = 0; i < 1_024; i += 1) await Promise.resolve();
};

function createScheduler() {
  const tasks = new Set<() => void>();
  const schedule = (task: () => void) => {
    tasks.add(task);
    return () => tasks.delete(task);
  };
  const runNext = async () => {
    const next = tasks.values().next();
    if (next.done) throw new Error("No scheduled task");
    const task = next.value;
    tasks.delete(task);
    task();
    await tick();
  };
  const flush = async () => {
    await tick();
    for (let i = 0; tasks.size > 0; i += 1) {
      if (i > 1_000) throw new Error("Scheduler did not settle");
      await runNext();
    }
  };
  return { tasks, schedule, runNext, flush };
}

function outputEvent(output: string, cursor: number): TerminalOutputEvent {
  return {
    session_id: "session-1",
    output,
    start_cursor: cursor - [...output].length,
    cursor,
  };
}

function outputSnapshot(
  output: string,
  cursor: number,
  overrides: Partial<TerminalOutputSnapshot> = {}
): TerminalOutputSnapshot {
  return {
    ...outputEvent(output, cursor),
    truncated: false,
    available_chars: cursor,
    ...overrides,
  };
}

function createHarness(maxInitialDeltaDrains = 5, replayErrors = false) {
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
  const scheduler = createScheduler();
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
    schedule: scheduler.schedule,
  };
  const controller = createTerminalOutputSyncController({
    sessionId: "session-1",
    channels: [
      { event: "data/session-1", replayedBySnapshot: true },
      { event: "error/session-1", replayedBySnapshot: replayErrors },
    ],
    write: (text) => writes.push(text),
    dependencies,
    maxInitialDeltaDrains,
  });
  const emit = (output: string, cursor: number, channel: 0 | 1 = 0) => {
    const event = channel === 0 ? "data/session-1" : "error/session-1";
    const listener = listeners.find((listener) => listener.event === event);
    if (!listener) throw new Error(`No listener for ${event}`);
    listener.handler({ payload: outputEvent(output, cursor) });
  };
  const resolveDelta = (cursor: number, snapshot: TerminalOutputSnapshot) => {
    const delta = deltas
      .slice()
      .reverse()
      .find((delta) => delta.cursor === cursor);
    if (!delta) throw new Error(`No delta requested at cursor ${cursor}`);
    delta.result.resolve(snapshot);
  };
  const subscribe = async () => {
    listeners.forEach(({ registration }) => {
      registration.resolve(vi.fn());
    });
    await tick();
  };
  const startLive = async (history = "", cursor = [...history].length) => {
    const started = controller.start();
    await subscribe();
    snapshots[0].resolve(outputSnapshot(history, cursor));
    await tick();
    deltas[0].result.resolve(outputSnapshot("", cursor));
    await started;
  };
  return {
    controller,
    listeners,
    snapshots,
    deltas,
    writes,
    scheduler,
    emit,
    resolveDelta,
    subscribe,
    startLive,
  };
}

describe("terminal output sync controller", () => {
  it.each([
    ["500 complete lines", `${"x".repeat(70)}\r\n`.repeat(500), "next\r\n".repeat(2_000)],
    ["long lines, Unicode and ANSI", `${"界😀".repeat(20_000)}\x1b[31mred\x1b[0m\r\n`, "界😀\r\n"],
    ["retention ceiling", `${"界😀\x1b[31m".repeat(300_000)}\x1b[0m\r\n`, "live\r\n"],
  ])(
    "restores bounded history without an additional truncation for %s",
    async (_name, history, additional) => {
      const ceiling = 2 * 1024 * 1024;
      const historyChars = Array.from(history);
      const retainedChars = historyChars.slice(-ceiling);
      const writes: string[] = [];
      const scheduler = createScheduler();
      const getSnapshot = vi.fn(async (_sessionId: string, maxChars: number) =>
        outputSnapshot(retainedChars.slice(-maxChars).join(""), historyChars.length, {
          available_chars: retainedChars.length,
          truncated: historyChars.length > retainedChars.length,
        })
      );
      const getDelta = vi.fn(async (_sessionId: string, cursor: number, _maxChars: number) =>
        outputSnapshot(additional, cursor + [...additional].length)
      );
      const controller = createTerminalOutputSyncController({
        sessionId: "session-1",
        channels: [{ event: "data/session-1", replayedBySnapshot: true }],
        dependencies: {
          listen: async () => vi.fn<TerminalOutputUnlisten>(),
          getSnapshot,
          getDelta,
          schedule: scheduler.schedule,
        },
        write: (text) => writes.push(text),
      });
      const started = controller.start();
      await scheduler.flush();
      await started;
      expect(getSnapshot).toHaveBeenCalledExactlyOnceWith("session-1", ceiling);
      expect(getDelta).toHaveBeenCalledExactlyOnceWith("session-1", historyChars.length, ceiling);
      expect(writes.join("")).toBe(retainedChars.join("") + additional);
      expect(writes.every((text) => [...text].length <= 20_000)).toBe(true);
      controller.dispose();
    }
  );

  it("waits for all subscriptions and suppresses delayed events covered by restoration", async () => {
    const h = createHarness();
    const started = h.controller.start();
    const stopData = vi.fn();
    const stopError = vi.fn();
    h.emit("A", 8);
    h.listeners[0].registration.resolve(stopData);
    await tick();
    expect(h.snapshots).toHaveLength(0);
    h.listeners[1].registration.resolve(stopError);
    await tick();
    h.snapshots[0].resolve(outputSnapshot("historyA", 8));
    await tick();
    h.emit("B", 9);
    expect(h.deltas[0].cursor).toBe(8);
    h.deltas[0].result.resolve(outputSnapshot("B", 9));
    await tick();
    h.deltas[1].result.resolve(outputSnapshot("", 9));
    await started;
    h.emit("historyA", 8);
    h.emit("C", 10);
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("historyABC");
    h.controller.dispose();
    h.controller.dispose();
    expect(stopData).toHaveBeenCalledOnce();
    expect(stopError).toHaveBeenCalledOnce();
  });

  it("preserves F arriving after the fifth delta snapshot but before its response is handled", async () => {
    const h = createHarness();
    const started = h.controller.start();
    await h.subscribe();
    h.snapshots[0].resolve(outputSnapshot("", 0));
    await tick();
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const text = String.fromCharCode(65 + attempt);
      h.emit(text, attempt + 1);
      expect(h.deltas).toHaveLength(attempt + 1);
      h.resolveDelta(attempt, outputSnapshot(text, attempt + 1));
      if (attempt === 4) h.emit("F", 6);
      await tick();
    }
    await started;
    h.emit("G", 7);
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("ABCDEFG");
    expect(h.deltas).toHaveLength(5);
  });

  it("deduplicates complete and partial ranges, retaining equal text at different cursors", async () => {
    const h = createHarness(5, true);
    await h.startLive("A😀", 2);
    h.emit("A😀", 2);
    h.emit("😀BC", 4, 1);
    h.emit("BC", 4);
    h.emit("BC", 6);
    h.emit("", 6);
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("A😀BCBC");
    expect(h.deltas).toHaveLength(1);
  });

  it("retains the suffix of an event partially covered by the last delta", async () => {
    const h = createHarness(1);
    const started = h.controller.start();
    await h.subscribe();
    h.snapshots[0].resolve(outputSnapshot("A", 1));
    await tick();
    h.emit("B😀D", 4);
    h.deltas[0].result.resolve(outputSnapshot("B😀", 3));
    await started;
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("AB😀D");
  });

  it("preserves string errors including non-retained SSH overflow notices", async () => {
    const h = createHarness(5, true);
    const started = h.controller.start();
    await h.subscribe();
    h.listeners[1].handler({ payload: "overflow notice" });
    h.snapshots[0].resolve(outputSnapshot("A", 1));
    await tick();
    h.deltas[0].result.resolve(outputSnapshot("", 1));
    await started;
    h.emit("B", 2);
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("Aoverflow noticeB");
  });

  it.each(["subscription", "snapshot"])(
    "uses received ranges as the baseline after %s failure",
    async (failure) => {
      const h = createHarness();
      const started = h.controller.start();
      h.emit("queued", 106);
      h.listeners[0].registration.resolve(vi.fn());
      if (failure === "subscription") {
        h.listeners[1].registration.reject(new Error("registration failed"));
      } else {
        h.listeners[1].registration.resolve(vi.fn());
        await tick();
        h.snapshots[0].reject(new Error("snapshot failed"));
      }
      await started;
      h.emit("queued", 106);
      h.emit("live", 110);
      await h.scheduler.flush();
      expect(h.writes.join("")).toBe("queuedlive");
    }
  );

  it("does not replay already applied output after a delta failure", async () => {
    const h = createHarness();
    const started = h.controller.start();
    await h.subscribe();
    h.emit("A", 1);
    h.snapshots[0].resolve(outputSnapshot("A", 1));
    await tick();
    h.emit("B", 2);
    h.deltas[0].result.reject(new Error("delta failed"));
    await started;
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("AB");
  });

  it("recovers a cursor gap with a single delta in flight and filters overlapping queued events", async () => {
    const h = createHarness();
    await h.startLive("A", 1);
    h.emit("D", 4);
    await h.scheduler.runNext();
    expect(h.deltas[1].cursor).toBe(1);
    h.emit("E", 5);
    h.emit("BC", 3);
    await tick();
    expect(h.deltas).toHaveLength(2);
    h.deltas[1].result.resolve(outputSnapshot("BCD", 4));
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("ABCDE");
    expect(h.deltas).toHaveLength(2);
  });

  it.each(["error", "no progress"])(
    "keeps a gap after %s and retries only on new events",
    async (failure) => {
      const h = createHarness();
      await h.startLive("A", 1);
      h.emit("D", 4);
      await h.scheduler.runNext();
      if (failure === "error") h.deltas[1].result.reject(new Error("delta failed"));
      else h.deltas[1].result.resolve(outputSnapshot("", 1));
      await h.scheduler.flush();
      expect(h.scheduler.tasks.size).toBe(0);
      expect(h.writes.join("")).toBe("A");
      h.emit("E", 5);
      await h.scheduler.runNext();
      expect(h.deltas[2].cursor).toBe(1);
      h.deltas[2].result.resolve(outputSnapshot("BC", 3));
      await h.scheduler.flush();
      expect(h.writes.join("")).toBe("ABCDE");
    }
  );

  it("retries an unsuccessful in-flight read when another event arrived during the read", async () => {
    const h = createHarness();
    await h.startLive("A", 1);
    h.emit("D", 4);
    await h.scheduler.runNext();
    h.emit("E", 5);
    h.deltas[1].result.reject(new Error("delta failed"));
    await tick();
    expect(h.scheduler.tasks.size).toBe(1);
    await h.scheduler.runNext();
    h.deltas[2].result.resolve(outputSnapshot("BC", 3));
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("ABCDE");
  });

  it("yields after five gap reads without discarding the pending event", async () => {
    const h = createHarness();
    await h.startLive("A", 1);
    h.emit("H", 8);
    await h.scheduler.runNext();
    for (let index = 1; index <= 5; index += 1) {
      h.resolveDelta(index, outputSnapshot(String.fromCharCode(65 + index), index + 1));
      await tick();
    }
    expect(h.deltas).toHaveLength(6);
    expect(h.scheduler.tasks.size).toBe(1);
    await h.scheduler.runNext();
    expect(h.deltas).toHaveLength(7);
    h.deltas[6].result.resolve(outputSnapshot("G", 7));
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("ABCDEFGH");
  });

  it("bounds each task to 20000 code points across events and yields under continuous output", async () => {
    const h = createHarness();
    await h.startLive();
    const text = "界😀".repeat(20_001);
    h.emit(text, 40_002);
    await h.scheduler.runNext();
    expect(h.writes.join("")).toBe(text.slice(0, 30_000));
    expect([...h.writes.join("")]).toHaveLength(20_000);
    h.emit("tail", 40_006);
    const beforeSecond = h.writes.length;
    await h.scheduler.runNext();
    expect([...h.writes.slice(beforeSecond).join("")]).toHaveLength(20_000);
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe(text + "tail");
  });

  it("cancels scheduled writes and releases late registrations exactly once", async () => {
    const h = createHarness();
    const started = h.controller.start();
    const stopData = vi.fn(() => Promise.reject(new Error("cleanup failed")));
    const stopError = vi.fn();
    h.controller.dispose();
    h.controller.dispose();
    h.emit("ignored", 7);
    h.listeners[0].registration.resolve(stopData);
    h.listeners[1].registration.resolve(stopError);
    await started;
    await tick();
    expect(stopData).toHaveBeenCalledOnce();
    expect(stopError).toHaveBeenCalledOnce();
    expect(h.snapshots).toHaveLength(0);
    expect(h.scheduler.tasks.size).toBe(0);
    expect(h.writes).toEqual([]);
  });

  it("shares the write budget between queued events", async () => {
    const h = createHarness();
    await h.startLive();
    h.emit("A".repeat(15_000), 15_000);
    h.emit("😀".repeat(15_000), 30_000);
    await h.scheduler.runNext();
    expect([...h.writes.join("")]).toHaveLength(20_000);
    expect(h.writes.join("")).toBe("A".repeat(15_000) + "😀".repeat(5_000));
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("A".repeat(15_000) + "😀".repeat(15_000));
  });

  it("yields while processing a burst of duplicate events even without new writes", async () => {
    const h = createHarness();
    await h.startLive("A", 1);
    for (let i = 0; i < 257; i += 1) h.emit("A", 1);
    h.emit("B", 2);
    await h.scheduler.runNext();
    expect(h.writes.join("")).toBe("A");
    expect(h.scheduler.tasks.size).toBe(1);
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("AB");
  });

  it("settles restoration when disposed during a scheduled continuation", async () => {
    const h = createHarness();
    const started = h.controller.start();
    await h.subscribe();
    h.snapshots[0].resolve(outputSnapshot("x".repeat(20_001), 20_001));
    await tick();
    expect(h.writes.join("")).toHaveLength(20_000);
    expect(h.scheduler.tasks.size).toBe(1);
    h.controller.dispose();
    await started;
    expect(h.scheduler.tasks.size).toBe(0);
    expect(h.deltas).toHaveLength(0);
    expect(h.writes.join("")).toHaveLength(20_000);
  });

  it.each(["snapshot", "delta", "live gap"])(
    "ignores late %s replies after disposal",
    async (stage) => {
      const h = createHarness();
      const started = h.controller.start();
      await h.subscribe();
      if (stage !== "snapshot") {
        h.snapshots[0].resolve(outputSnapshot("", 0));
        await tick();
      }
      if (stage === "live gap") {
        h.deltas[0].result.resolve(outputSnapshot("", 0));
        await started;
        h.emit("B", 2);
        await h.scheduler.runNext();
      }
      h.controller.dispose();
      h.emit("ignored", 7);
      if (stage === "snapshot") h.snapshots[0].resolve(outputSnapshot("late", 4));
      else h.deltas[h.deltas.length - 1].result.resolve(outputSnapshot("late", 4));
      await started;
      await h.scheduler.flush();
      expect(h.writes).toEqual([]);
      expect(h.scheduler.tasks.size).toBe(0);
    }
  );

  it("cancels a live continuation without losing disposal completion", async () => {
    const h = createHarness();
    await h.startLive();
    h.emit("x".repeat(20_001), 20_001);
    await h.scheduler.runNext();
    h.controller.dispose();
    await h.scheduler.flush();
    expect(h.writes.join("")).toHaveLength(20_000);
    expect(h.scheduler.tasks.size).toBe(0);
  });

  it("restores on remount and rejects old-window callbacks and foreign-session payloads", async () => {
    const oldWindow = createHarness();
    await oldWindow.startLive("A", 1);
    oldWindow.emit("B", 2);
    oldWindow.controller.dispose();
    const destination = createHarness();
    await destination.startLive("AB", 2);
    oldWindow.emit("C", 3);
    destination.emit("B", 2);
    destination.listeners[0].handler({
      payload: { ...outputEvent("private", 9), session_id: "other" },
    });
    destination.emit("C", 3);
    await oldWindow.scheduler.flush();
    await destination.scheduler.flush();
    expect(oldWindow.writes.join("")).toBe("A");
    expect(destination.writes.join("")).toBe("ABC");
    expect(destination.deltas).toHaveLength(1);
  });

  it("honors the backend retention gap and does not replay its covered events", async () => {
    const h = createHarness();
    await h.startLive("A", 1);
    h.emit("Z", 30);
    await h.scheduler.runNext();
    h.deltas[1].result.resolve(outputSnapshot("tail", 29, { truncated: true, start_cursor: 25 }));
    await h.scheduler.flush();
    expect(h.writes.join("")).toBe("AtailZ");
  });
});
