export interface TerminalKeyboardEvent {
  altKey: boolean;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

export function getTerminalControlInput(event: TerminalKeyboardEvent): string | null {
  if (
    event.code === "Digit6" &&
    event.ctrlKey &&
    event.shiftKey &&
    !event.altKey &&
    !event.metaKey
  ) {
    return "\x1e";
  }

  return null;
}
