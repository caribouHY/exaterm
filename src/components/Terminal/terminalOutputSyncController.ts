export interface TerminalOutputEvent {
  session_id: string;
  output: string;
  start_cursor: number;
  cursor: number;
}

export interface TerminalOutputSnapshot extends TerminalOutputEvent {
  truncated: boolean;
  available_chars: number;
}

export interface TerminalOutputChannel {
  event: string;
  replayedBySnapshot: boolean;
}

export type TerminalOutputEventPayload = TerminalOutputEvent | string;
export type TerminalOutputUnlisten = () => void | Promise<void>;

export interface TerminalOutputSyncDependencies {
  listen: (
    event: string,
    handler: (event: { payload: TerminalOutputEventPayload }) => void
  ) => Promise<TerminalOutputUnlisten>;
  getSnapshot: (sessionId: string, maxChars: number) => Promise<TerminalOutputSnapshot>;
  getDelta: (
    sessionId: string,
    cursor: number,
    maxChars: number
  ) => Promise<TerminalOutputSnapshot>;
  schedule?: (task: () => void) => () => void;
}

interface TerminalOutputSyncOptions {
  sessionId: string;
  channels: TerminalOutputChannel[];
  write: (text: string) => void;
  dependencies: TerminalOutputSyncDependencies;
  maxInitialDeltaDrains?: number;
}

export interface TerminalOutputSyncController {
  start: () => Promise<void>;
  dispose: () => void;
}

interface Subscription {
  unlisten: TerminalOutputUnlisten;
  released: boolean;
}

// Match the backend retention ceiling so restoration does not truncate retained history again.
const RESTORE_MAX_CHARS = 2 * 1024 * 1024;
const DEFAULT_MAX_INITIAL_DELTA_DRAINS = 5;
const MAX_TASK_WRITES = 20_000;
const MAX_TASK_DELTA_READS = 5;
const MAX_TASK_EVENTS = 256;

const scheduleTimeout = (task: () => void) => {
  const timer = setTimeout(task, 0);
  return () => {
    clearTimeout(timer);
  };
};

function splitText(text: string, chars: number): [string, string, number] {
  let offset = 0;
  let count = 0;
  for (const char of text) {
    if (count === chars) break;
    offset += char.length;
    count += 1;
  }
  return [text.slice(0, offset), text.slice(offset), count];
}

export function createTerminalOutputSyncController(
  options: TerminalOutputSyncOptions
): TerminalOutputSyncController {
  let phase: "idle" | "subscribing" | "syncing" | "live" | "disposed" = "idle";
  let startPromise: Promise<void> | null = null;
  let appliedCursor: number | null = null;
  let retainedVersion = 0;
  let receivedVersion = 0;
  let writeBudget = MAX_TASK_WRITES;
  let pumping = false;
  let cancelPump: (() => void) | null = null;
  const pending: TerminalOutputEventPayload[] = [];
  const subscriptions: Subscription[] = [];
  const yields = new Set<() => void>();
  const schedule = options.dependencies.schedule ?? scheduleTimeout;
  const isDisposed = () => phase === "disposed";

  const release = (subscription: Subscription) => {
    if (subscription.released) return;
    subscription.released = true;
    try {
      void Promise.resolve(subscription.unlisten()).catch(() => {
        // Listener cleanup failure must not create an unhandled rejection.
      });
    } catch {
      // Listener cleanup failure must not interrupt disposal of the other channels.
    }
  };

  const yieldTask = () =>
    new Promise<void>((resolve) => {
      const finish = () => {
        cancel();
        yields.delete(finish);
        writeBudget = MAX_TASK_WRITES;
        resolve();
      };
      const cancel = schedule(finish);
      yields.add(finish);
    });

  const writeText = async (text: string, advanceCursor: boolean) => {
    let remaining = text;
    while (remaining.length > 0 && !isDisposed()) {
      if (writeBudget === 0) await yieldTask();
      if (isDisposed()) return;
      const [chunk, rest, count] = splitText(remaining, writeBudget);
      remaining = rest;
      writeBudget -= count;
      if (advanceCursor && appliedCursor !== null) appliedCursor += count;
      options.write(chunk);
    }
  };

  const applyRange = async (range: TerminalOutputEvent, allowGap: boolean) => {
    if (isDisposed()) return;
    if (appliedCursor === null || (allowGap && appliedCursor < range.start_cursor)) {
      appliedCursor = range.start_cursor;
    }
    if (range.cursor <= appliedCursor) return;
    const [, suffix] = splitText(range.output, appliedCursor - range.start_cursor);
    await writeText(suffix, true);
  };

  const pump = async (): Promise<number | null> => {
    let deltaReads = 0;
    let events = 0;
    while (pending.length > 0 && !isDisposed()) {
      if (events === MAX_TASK_EVENTS) {
        await yieldTask();
        events = 0;
        deltaReads = 0;
      }
      if (isDisposed()) return null;
      const output = pending[0];
      if (typeof output === "string") {
        pending.shift();
        events += 1;
        await writeText(output, false);
      } else if (appliedCursor === null || output.start_cursor <= appliedCursor) {
        pending.shift();
        events += 1;
        await applyRange(output, false);
      } else {
        if (deltaReads === MAX_TASK_DELTA_READS) {
          await yieldTask();
          deltaReads = 0;
        }
        if (isDisposed()) return null;
        const requestedCursor = appliedCursor;
        const version = receivedVersion;
        try {
          const delta = await options.dependencies.getDelta(
            options.sessionId,
            requestedCursor,
            RESTORE_MAX_CHARS
          );
          deltaReads += 1;
          if (isDisposed()) return null;
          await applyRange(delta, delta.truncated);
          if (appliedCursor === requestedCursor) return version;
        } catch {
          // Keep the gap and retry on a subsequent event, without an idle retry loop.
          return version;
        }
      }
    }
    return null;
  };

  const schedulePump = () => {
    if (phase !== "live" || pumping || cancelPump || pending.length === 0) return;
    cancelPump = schedule(() => {
      cancelPump = null;
      if (isDisposed()) return;
      writeBudget = MAX_TASK_WRITES;
      pumping = true;
      void pump().then((blockedVersion) => {
        pumping = false;
        if (blockedVersion === null || receivedVersion !== blockedVersion) schedulePump();
      });
    });
  };

  const receive = (channel: TerminalOutputChannel, payload: TerminalOutputEventPayload) => {
    if (isDisposed()) return;
    if (typeof payload !== "string") {
      if (!channel.replayedBySnapshot || payload.session_id !== options.sessionId) return;
      retainedVersion += 1;
    }
    receivedVersion += 1;
    pending.push(payload);
    schedulePump();
  };

  const subscribe = async (channel: TerminalOutputChannel) => {
    const unlisten = await options.dependencies.listen(channel.event, ({ payload }) => {
      receive(channel, payload);
    });
    const subscription = { unlisten, released: false };
    subscriptions.push(subscription);
    if (isDisposed()) release(subscription);
  };

  const enterLive = () => {
    if (isDisposed()) return;
    phase = "live";
    schedulePump();
  };

  const run = async () => {
    phase = "subscribing";
    const registrations = await Promise.allSettled(options.channels.map(subscribe));
    if (isDisposed()) return;
    if (registrations.some((result) => result.status === "rejected")) {
      enterLive();
      return;
    }

    phase = "syncing";
    try {
      const snapshot = await options.dependencies.getSnapshot(options.sessionId, RESTORE_MAX_CHARS);
      if (isDisposed()) return;
      await applyRange(snapshot, true);
      const drainLimit = options.maxInitialDeltaDrains ?? DEFAULT_MAX_INITIAL_DELTA_DRAINS;
      for (let attempt = 0; attempt < drainLimit && !isDisposed(); attempt += 1) {
        const version = retainedVersion;
        const delta = await options.dependencies.getDelta(
          options.sessionId,
          appliedCursor ?? snapshot.cursor,
          RESTORE_MAX_CHARS
        );
        if (isDisposed()) return;
        await applyRange(delta, delta.truncated);
        if (retainedVersion === version) break;
      }
    } catch {
      // Cursor-tagged events remain usable even if restoration fails partway through.
    }
    enterLive();
  };

  return {
    start: () => {
      if (startPromise) return startPromise;
      if (isDisposed()) return Promise.resolve();
      startPromise = run();
      return startPromise;
    },
    dispose: () => {
      if (isDisposed()) return;
      phase = "disposed";
      cancelPump?.();
      cancelPump = null;
      yields.forEach((finish) => {
        finish();
      });
      pending.length = 0;
      subscriptions.forEach(release);
    },
  };
}
