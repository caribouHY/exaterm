import type { FitAddon } from "@xterm/addon-fit";

interface TerminalFitOptions {
  container: Pick<HTMLElement, "clientWidth" | "clientHeight">;
  fitAddon: Pick<FitAddon, "fit">;
  isActive: () => boolean;
  onFit: () => void;
}

export interface TerminalFitController {
  fit: () => boolean;
  schedule: (afterFit?: () => void) => () => void;
  cancelPending: () => void;
  dispose: () => void;
}

export function createTerminalFitController(options: TerminalFitOptions): TerminalFitController {
  let disposed = false;
  const timers = new Set<ReturnType<typeof setTimeout>>();

  const fit = () => {
    // Fitting a hidden container can shrink xterm and permanently truncate its cursor line.
    if (
      disposed ||
      !options.isActive() ||
      !(options.container.clientWidth > 0) ||
      !(options.container.clientHeight > 0)
    ) {
      return false;
    }
    options.fitAddon.fit();
    options.onFit();
    return true;
  };

  const cancelPending = () => {
    timers.forEach(clearTimeout);
    timers.clear();
  };

  return {
    fit,
    schedule: (afterFit) => {
      if (disposed || !options.isActive()) return () => {};
      const timer = setTimeout(() => {
        timers.delete(timer);
        if (fit()) afterFit?.();
      }, 50);
      timers.add(timer);
      return () => {
        clearTimeout(timer);
        timers.delete(timer);
      };
    },
    cancelPending,
    dispose: () => {
      disposed = true;
      cancelPending();
    },
  };
}
