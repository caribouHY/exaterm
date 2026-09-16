import type { Encoding } from "../../types";

export interface TerminalOutputSnapshot {
  session_id: string;
  output: string;
  truncated: boolean;
  available_chars: number;
  start_cursor: number;
  cursor: number;
}

export interface TerminalOutputChannel {
  event: string;
  replayedBySnapshot: boolean;
}

export type TerminalOutputEventPayload = number[] | string;
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
}

interface TerminalOutputSyncOptions {
  sessionId: string;
  encoding: Encoding;
  maxChars: number;
  channels: TerminalOutputChannel[];
  write: (text: string) => void;
  dependencies: TerminalOutputSyncDependencies;
  maxInitialDeltaDrains?: number;
}

export interface TerminalOutputSyncController {
  start: () => Promise<void>;
  setEncoding: (encoding: Encoding) => void;
  dispose: () => void;
}

interface Subscription {
  unlisten: TerminalOutputUnlisten;
  released: boolean;
}

const DEFAULT_MAX_INITIAL_DELTA_DRAINS = 5;

export function createTerminalOutputSyncController(
  options: TerminalOutputSyncOptions
): TerminalOutputSyncController {
  let phase: "idle" | "subscribing" | "syncing" | "live" | "disposed" = "idle";
  let decoder = new TextDecoder(options.encoding);
  let startPromise: Promise<void> | null = null;
  let retainedOutputSeen = false;
  let bufferedOutput: string[] = [];
  let bufferedNonReplayableOutput: string[] = [];
  const subscriptions: Subscription[] = [];
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

  const write = (text: string) => {
    if (isDisposed() || text.length === 0) return;
    options.write(text);
  };

  const decodePayload = (payload: TerminalOutputEventPayload) => {
    if (typeof payload === "string") return payload;
    return decoder.decode(new Uint8Array(payload), { stream: true });
  };

  const receive = (channel: TerminalOutputChannel, payload: TerminalOutputEventPayload) => {
    if (isDisposed()) return;

    const text = decodePayload(payload);
    if (phase === "live") {
      write(text);
      return;
    }

    // Retained events only trigger another cursor-based read because live events carry no cursor.
    if (channel.replayedBySnapshot) {
      retainedOutputSeen = true;
    } else if (text.length > 0) {
      bufferedNonReplayableOutput.push(text);
    }
    if (text.length > 0) bufferedOutput.push(text);
  };

  const subscribe = async (channel: TerminalOutputChannel) => {
    const unlisten = await options.dependencies.listen(channel.event, ({ payload }) => {
      receive(channel, payload);
    });
    const subscription = { unlisten, released: false };
    subscriptions.push(subscription);
    if (isDisposed()) release(subscription);
  };

  const enterLiveWithBufferedOutput = () => {
    const pending = bufferedOutput;
    bufferedOutput = [];
    bufferedNonReplayableOutput = [];
    if (isDisposed()) return;
    phase = "live";
    pending.forEach(write);
  };

  const enterLiveAfterRestore = () => {
    const pending = bufferedNonReplayableOutput;
    bufferedOutput = [];
    bufferedNonReplayableOutput = [];
    if (isDisposed()) return;
    phase = "live";
    pending.forEach(write);
  };

  const run = async () => {
    phase = "subscribing";
    const registrations = await Promise.allSettled(options.channels.map(subscribe));
    if (isDisposed()) return;

    if (registrations.some((result) => result.status === "rejected")) {
      enterLiveWithBufferedOutput();
      return;
    }

    phase = "syncing";
    try {
      const snapshot = await options.dependencies.getSnapshot(options.sessionId, options.maxChars);
      if (isDisposed()) return;
      write(snapshot.output);

      let cursor = snapshot.cursor;
      const drainLimit = options.maxInitialDeltaDrains ?? DEFAULT_MAX_INITIAL_DELTA_DRAINS;
      for (let attempt = 0; attempt < drainLimit; attempt += 1) {
        retainedOutputSeen = false;
        const delta = await options.dependencies.getDelta(
          options.sessionId,
          cursor,
          options.maxChars
        );
        if (isDisposed()) return;
        write(delta.output);
        cursor = delta.cursor;
        if (!retainedOutputSeen) break;
      }

      enterLiveAfterRestore();
    } catch {
      enterLiveWithBufferedOutput();
    }
  };

  return {
    start: () => {
      if (startPromise) return startPromise;
      if (phase === "disposed") return Promise.resolve();
      startPromise = run();
      return startPromise;
    },
    setEncoding: (encoding) => {
      if (phase !== "disposed") decoder = new TextDecoder(encoding);
    },
    dispose: () => {
      if (phase === "disposed") return;
      phase = "disposed";
      bufferedOutput = [];
      bufferedNonReplayableOutput = [];
      subscriptions.forEach(release);
    },
  };
}
