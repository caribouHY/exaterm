import type { TerminalMode } from "../types";

export const DEFAULT_TERMINAL_MODE: TerminalMode = "general";

export const TERMINAL_MODE_OPTIONS: Array<{ labelKey: string; value: TerminalMode }> = [
  { labelKey: "terminal_mode.general", value: "general" },
  { labelKey: "terminal_mode.cisco_ios", value: "cisco_ios" },
  { labelKey: "terminal_mode.arista_eos", value: "arista_eos" },
  { labelKey: "terminal_mode.vyos", value: "vyos" },
  { labelKey: "terminal_mode.fujitsu_sir", value: "fujitsu_sir" },
];

export function normalizeTerminalMode(terminalMode: string | null | undefined): TerminalMode {
  return TERMINAL_MODE_OPTIONS.some((entry) => entry.value === terminalMode)
    ? (terminalMode as TerminalMode)
    : DEFAULT_TERMINAL_MODE;
}
