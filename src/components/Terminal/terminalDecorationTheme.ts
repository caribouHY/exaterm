import type { TerminalPromptVariant } from "./terminalDecorationTypes";

export const TERMINAL_DECORATION_COLORS = {
  prompt: "#7dd3fc",
  configurationPrompt: "#facc15",
  command: "#6ee7b7",
  error: "#f87171",
} as const;

export function getTerminalPromptColor(variant: TerminalPromptVariant): string {
  return variant === "configuration"
    ? TERMINAL_DECORATION_COLORS.configurationPrompt
    : TERMINAL_DECORATION_COLORS.prompt;
}
