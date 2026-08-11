export const CLEAR_TERMINAL_VIEWPORT_SEQUENCE = "\x1b[2J\x1b[H";

interface ClearableTerminal {
  write: (data: string, callback?: () => void) => void;
  clear: () => void;
  clearSelection: () => void;
  focus: () => void;
}

function finishTerminalClear(terminal: ClearableTerminal, onCleared?: () => void) {
  terminal.clearSelection();
  onCleared?.();
  terminal.focus();
}

export function clearTerminalViewport(terminal: ClearableTerminal, onCleared?: () => void) {
  terminal.write(CLEAR_TERMINAL_VIEWPORT_SEQUENCE, () => {
    finishTerminalClear(terminal, onCleared);
  });
}

export function clearTerminalBuffer(terminal: ClearableTerminal, onCleared?: () => void) {
  terminal.clear();
  terminal.write(CLEAR_TERMINAL_VIEWPORT_SEQUENCE, () => {
    finishTerminalClear(terminal, onCleared);
  });
}
