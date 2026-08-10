import type { TerminalMode } from "../../types";

export type TerminalPromptVariant = "default" | "configuration";

export interface TerminalParsedPrompt {
  promptText: string;
  promptStart: number;
  commandText: string;
  commandStart: number;
  commandSeparator: string;
  variant: TerminalPromptVariant;
}

export interface TerminalParsedContext {
  contextText: string;
  contextStart: number;
  variant: TerminalPromptVariant;
}

export interface TerminalDecorationProfile {
  mode: TerminalMode;
  decorationLookback: number;
  decorationStyle: string;
  pinnedCommand: boolean;
  parsePrompt: (line: string) => TerminalParsedPrompt | null;
  parseContextLine?: (line: string) => TerminalParsedContext | null;
  isErrorLine: (line: string) => boolean;
}

export interface TerminalBufferLineLike {
  isWrapped: boolean;
  translateToString: (trimRight?: boolean) => string;
}

export interface TerminalBufferLike {
  type: "normal" | "alternate";
  viewportY: number;
  length: number;
  getLine: (lineIndex: number) => TerminalBufferLineLike | undefined;
}

export interface TerminalCommandSegment {
  lineIndex: number;
  x: number;
  width: number;
  text: string;
}

export interface TerminalPinnedCommand {
  displayText: string;
  contextText?: string;
  promptText: string;
  commandText: string;
  promptVariant: TerminalPromptVariant;
  promptLineIndex: number;
  commandLineCount: number;
}
