import type { TerminalMode } from "../types";

export const DEFAULT_TERMINAL_MODE: TerminalMode = "general";

export const TERMINAL_MODE_OPTIONS: Array<{ labelKey: string; value: TerminalMode }> = [
  { labelKey: "terminal_mode.general", value: "general" },
  { labelKey: "terminal_mode.cisco_ios", value: "cisco_ios" },
];

export function normalizeTerminalMode(terminalMode: string | null | undefined): TerminalMode {
  return TERMINAL_MODE_OPTIONS.some((entry) => entry.value === terminalMode)
    ? (terminalMode as TerminalMode)
    : DEFAULT_TERMINAL_MODE;
}
