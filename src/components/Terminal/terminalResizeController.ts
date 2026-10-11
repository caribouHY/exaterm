import type { Terminal } from "@xterm/xterm";

interface TerminalSize {
  cols: number;
  rows: number;
}

interface TerminalResizeOptions {
  terminal: Pick<Terminal, "cols" | "rows" | "onResize">;
  container: Pick<HTMLElement, "clientWidth" | "clientHeight">;
  isActive: () => boolean;
  isConnected: boolean;
  sendResize: ((size: TerminalSize) => Promise<unknown>) | null;
}

export interface TerminalResizeController {
  synchronize: () => void;
  setConnected: (connected: boolean) => void;
  dispose: () => void;
}

const sameSize = (left: TerminalSize | null, right: TerminalSize) =>
  left?.cols === right.cols && left.rows === right.rows;

export function createTerminalResizeController(
  options: TerminalResizeOptions
): TerminalResizeController {
  let connected = options.isConnected;
  let disposed = false;
  let generation = 0;
  let sending = false;
  let pending: TerminalSize | null = null;
  let synchronized: TerminalSize | null = null;

  const canSend = () =>
    !disposed &&
    connected &&
    options.sendResize !== null &&
    options.isActive() &&
    options.container.clientWidth > 0 &&
    options.container.clientHeight > 0;

  const completeResize = (size: TerminalSize, sendGeneration: number, succeeded: boolean) => {
    sending = false;
    if (disposed) return;
    if (sendGeneration === generation) {
      if (succeeded) synchronized = size;
      else if (pending && sameSize(pending, size)) pending = null;
    }
    flush();
  };

  const flush = () => {
    if (sending) return;
    if (!canSend()) {
      pending = null;
      return;
    }
    const size = pending;
    const sendResize = options.sendResize;
    pending = null;
    if (!size || !sendResize || sameSize(synchronized, size)) return;

    sending = true;
    const sendGeneration = generation;
    void (async () => {
      let succeeded = false;
      try {
        await sendResize(size);
        succeeded = true;
      } catch {
        // A failed size remains unsynchronized until another fit or connection trigger.
      }
      completeResize(size, sendGeneration, succeeded);
    })();
  };

  const synchronize = () => {
    if (!canSend()) {
      pending = null;
      return;
    }
    pending = { cols: options.terminal.cols, rows: options.terminal.rows };
    flush();
  };

  const subscription = options.terminal.onResize(synchronize);

  return {
    synchronize,
    setConnected: (nextConnected) => {
      if (disposed || connected === nextConnected) return;
      connected = nextConnected;
      generation += 1;
      synchronized = null;
      pending = null;
      if (connected) synchronize();
    },
    dispose: () => {
      if (disposed) return;
      disposed = true;
      generation += 1;
      pending = null;
      subscription.dispose();
    },
  };
}
