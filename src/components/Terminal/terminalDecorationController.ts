import { Terminal } from "@xterm/xterm";
import type { IDecoration, IMarker } from "@xterm/xterm";
import {
  collectTerminalCommandSegments,
  findTerminalPinnedCommand,
  hasTerminalPromptInRange,
} from "./terminalCommandModel";
import { getTerminalPromptColor, TERMINAL_DECORATION_COLORS } from "./terminalDecorationTheme";
import type {
  TerminalCommandSegment,
  TerminalDecorationProfile,
  TerminalPinnedCommand,
} from "./terminalDecorationTypes";

interface PromptDecorationSet {
  promptSignature: string;
  commandSignature: string;
  marker: IMarker;
  promptDecoration: IDecoration;
  commandDecorations: CommandDecoration[];
}

interface LineDecorationSet {
  signature: string;
  marker: IMarker;
  decoration: IDecoration;
}

interface CommandDecoration {
  decoration: IDecoration;
  marker?: IMarker;
}

interface LineRange {
  firstLineIndex: number;
  lastLineIndex: number;
}

interface PinnedCommandContext {
  marker: IMarker;
  commandLineCount: number;
  pinnedCommand: TerminalPinnedCommand;
  viewportY: number;
}

interface TerminalDecorationControllerOptions {
  onPinnedCommandChange: (command: TerminalPinnedCommand | null) => void;
  requestFrame?: (callback: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
}

export interface TerminalDecorationController {
  setProfile: (profile: TerminalDecorationProfile | null, terminal?: Terminal) => void;
  schedule: (terminal: Terminal, rebuild?: boolean) => void;
  decorateNow: (terminal: Terminal) => void;
  clear: () => void;
}

export function createTerminalDecorationController({
  onPinnedCommandChange,
  requestFrame = (callback) => window.requestAnimationFrame(callback),
  cancelFrame = (handle) => {
    window.cancelAnimationFrame(handle);
  },
}: TerminalDecorationControllerOptions): TerminalDecorationController {
  let profile: TerminalDecorationProfile | null = null;
  let decorationFrame: number | null = null;
  let decorationRebuild = false;
  let pinnedCommandContext: PinnedCommandContext | null = null;
  const promptDecorations = new Map<number, PromptDecorationSet>();
  const errorDecorations = new Map<number, LineDecorationSet>();

  const disposeCommandDecoration = ({ decoration, marker }: CommandDecoration) => {
    decoration.dispose();
    marker?.dispose();
  };

  const disposePromptDecorationSet = ({
    commandDecorations,
    promptDecoration,
    marker,
  }: PromptDecorationSet) => {
    commandDecorations.forEach(disposeCommandDecoration);
    promptDecoration.dispose();
    marker.dispose();
  };

  const disposeLineDecorationSet = ({ decoration, marker }: LineDecorationSet) => {
    decoration.dispose();
    marker.dispose();
  };

  const clearDecorationSets = () => {
    promptDecorations.forEach(disposePromptDecorationSet);
    promptDecorations.clear();
    errorDecorations.forEach(disposeLineDecorationSet);
    errorDecorations.clear();
  };

  const clearPinnedCommand = () => {
    pinnedCommandContext?.marker.dispose();
    pinnedCommandContext = null;
    onPinnedCommandChange(null);
  };

  const cancelScheduledDecoration = () => {
    decorationRebuild = false;
    if (decorationFrame === null) return;
    cancelFrame(decorationFrame);
    decorationFrame = null;
  };

  const getDecorationRanges = (
    terminal: Terminal,
    activeProfile: TerminalDecorationProfile
  ): LineRange[] => {
    const buffer = terminal.buffer.active;
    if (buffer.type === "alternate") return [];

    const lastBufferLineIndex = Math.max(0, buffer.length - 1);
    const cursorLineIndex = Math.min(lastBufferLineIndex, buffer.baseY + buffer.cursorY);
    const viewportLastLineIndex = Math.min(
      lastBufferLineIndex,
      buffer.viewportY + terminal.rows - 1
    );
    const ranges: LineRange[] = [];

    const addRange = (firstLineIndex: number, lastLineIndex: number) => {
      const nextRange = {
        firstLineIndex: Math.max(0, firstLineIndex),
        lastLineIndex: Math.min(lastBufferLineIndex, lastLineIndex),
      };
      if (nextRange.lastLineIndex < nextRange.firstLineIndex) return;
      ranges.push(nextRange);
    };

    addRange(cursorLineIndex - activeProfile.decorationLookback, cursorLineIndex);
    addRange(buffer.viewportY - activeProfile.decorationLookback, viewportLastLineIndex);

    return ranges
      .sort((a, b) => a.firstLineIndex - b.firstLineIndex)
      .reduce<LineRange[]>((mergedRanges, range) => {
        if (mergedRanges.length === 0) {
          mergedRanges.push({ ...range });
          return mergedRanges;
        }

        const previousRange = mergedRanges[mergedRanges.length - 1];
        if (range.firstLineIndex > previousRange.lastLineIndex + 1) {
          mergedRanges.push({ ...range });
          return mergedRanges;
        }

        previousRange.lastLineIndex = Math.max(previousRange.lastLineIndex, range.lastLineIndex);
        return mergedRanges;
      }, []);
  };

  const isLineInRanges = (lineIndex: number, ranges: LineRange[]) =>
    ranges.some((range) => lineIndex >= range.firstLineIndex && lineIndex <= range.lastLineIndex);

  const decorateErrors = (terminal: Terminal, activeProfile: TerminalDecorationProfile) => {
    const buffer = terminal.buffer.active;
    if (buffer.type === "alternate") return;

    const cursorLineIndex = buffer.baseY + buffer.cursorY;
    const decorationRanges = getDecorationRanges(terminal, activeProfile);
    const visitedErrorLineIndexes = new Set<number>();

    decorationRanges.forEach(({ firstLineIndex, lastLineIndex }) => {
      for (let lineIndex = firstLineIndex; lineIndex <= lastLineIndex; lineIndex += 1) {
        const line = buffer.getLine(lineIndex)?.translateToString(true) ?? "";
        const trimmedLine = line.trimEnd();
        if (!trimmedLine || activeProfile.parsePrompt(trimmedLine)) continue;
        if (!activeProfile.isErrorLine(trimmedLine)) continue;

        const decorationStart = Math.max(0, trimmedLine.search(/\S/));
        const decorationWidth = trimmedLine.length - decorationStart;
        if (decorationWidth <= 0) continue;

        const signature = `${decorationStart}:${decorationWidth}:${trimmedLine}`;
        const existingDecorationSet = errorDecorations.get(lineIndex);
        visitedErrorLineIndexes.add(lineIndex);
        if (existingDecorationSet?.signature === signature) continue;
        if (existingDecorationSet) {
          disposeLineDecorationSet(existingDecorationSet);
          errorDecorations.delete(lineIndex);
        }

        const marker = terminal.registerMarker(lineIndex - cursorLineIndex);
        if (!marker) continue;

        const decoration = terminal.registerDecoration({
          marker,
          x: decorationStart,
          width: decorationWidth,
          foregroundColor: TERMINAL_DECORATION_COLORS.error,
          layer: "top",
        });

        if (!decoration) {
          marker.dispose();
          continue;
        }

        decoration.onDispose(() => errorDecorations.delete(lineIndex));
        errorDecorations.set(lineIndex, { signature, marker, decoration });
      }
    });

    errorDecorations.forEach((decorationSet, decoratedLineIndex) => {
      if (
        !isLineInRanges(decoratedLineIndex, decorationRanges) ||
        visitedErrorLineIndexes.has(decoratedLineIndex)
      ) {
        return;
      }

      disposeLineDecorationSet(decorationSet);
      errorDecorations.delete(decoratedLineIndex);
    });
  };

  const registerCommandDecorations = (
    terminal: Terminal,
    promptMarker: IMarker,
    promptLineIndex: number,
    segments: TerminalCommandSegment[],
    cursorLineIndex: number
  ): CommandDecoration[] => {
    const commandDecorations: CommandDecoration[] = [];

    segments.forEach((segment) => {
      const isPromptLine = segment.lineIndex === promptLineIndex;
      const marker = isPromptLine
        ? promptMarker
        : terminal.registerMarker(segment.lineIndex - cursorLineIndex);
      if (!marker) return;

      const decoration = terminal.registerDecoration({
        marker,
        x: segment.x,
        width: segment.width,
        foregroundColor: TERMINAL_DECORATION_COLORS.command,
        layer: "top",
      });

      if (!decoration) {
        if (!isPromptLine) marker.dispose();
        return;
      }

      commandDecorations.push({
        decoration,
        marker: isPromptLine ? undefined : marker,
      });
    });

    return commandDecorations;
  };

  const decoratePrompts = (terminal: Terminal, activeProfile: TerminalDecorationProfile) => {
    const buffer = terminal.buffer.active;
    if (buffer.type === "alternate") return;

    const cursorLineIndex = buffer.baseY + buffer.cursorY;
    const decorationRanges = getDecorationRanges(terminal, activeProfile);
    const visitedPromptLineIndexes = new Set<number>();

    decorationRanges.forEach(({ firstLineIndex, lastLineIndex }) => {
      for (let lineIndex = firstLineIndex; lineIndex <= lastLineIndex; lineIndex += 1) {
        const bufferLine = buffer.getLine(lineIndex);
        if (!bufferLine || bufferLine.isWrapped) continue;

        const trimmedLine = bufferLine.translateToString(true).trimEnd();
        const prompt = activeProfile.parsePrompt(trimmedLine);
        if (!prompt) continue;

        const { promptStart, promptText, commandText, commandStart, variant } = prompt;
        const promptWidth = promptText.length;
        if (promptStart < 0 || promptWidth === 0) continue;
        const commandSegments = collectTerminalCommandSegments(
          buffer,
          lineIndex,
          commandStart,
          commandText,
          lastLineIndex
        );
        const promptSignature = `${promptStart}:${promptWidth}:${promptText}:${variant}:${activeProfile.decorationStyle}`;
        const commandSignature = commandSegments
          .map(
            (segment) =>
              `${segment.lineIndex - lineIndex}:${segment.x}:${segment.width}:${segment.text}`
          )
          .join("\n");
        const existingDecorationSet = promptDecorations.get(lineIndex);
        visitedPromptLineIndexes.add(lineIndex);

        if (existingDecorationSet?.promptSignature === promptSignature) {
          if (existingDecorationSet.commandSignature === commandSignature) continue;

          existingDecorationSet.commandDecorations.forEach(disposeCommandDecoration);
          existingDecorationSet.commandDecorations = registerCommandDecorations(
            terminal,
            existingDecorationSet.marker,
            lineIndex,
            commandSegments,
            cursorLineIndex
          );
          existingDecorationSet.commandSignature = commandSignature;
          continue;
        }

        if (existingDecorationSet) {
          disposePromptDecorationSet(existingDecorationSet);
          promptDecorations.delete(lineIndex);
        }

        const marker = terminal.registerMarker(lineIndex - cursorLineIndex);
        if (!marker) continue;

        const promptDecoration = terminal.registerDecoration({
          marker,
          x: promptStart,
          width: promptWidth,
          foregroundColor: getTerminalPromptColor(variant),
          layer: "top",
        });

        if (!promptDecoration) {
          marker.dispose();
          continue;
        }

        const commandDecorations = registerCommandDecorations(
          terminal,
          marker,
          lineIndex,
          commandSegments,
          cursorLineIndex
        );

        promptDecoration.onDispose(() => promptDecorations.delete(lineIndex));
        promptDecorations.set(lineIndex, {
          promptSignature,
          commandSignature,
          marker,
          promptDecoration,
          commandDecorations,
        });
      }
    });

    promptDecorations.forEach((decorationSet, decoratedLineIndex) => {
      if (
        !isLineInRanges(decoratedLineIndex, decorationRanges) ||
        visitedPromptLineIndexes.has(decoratedLineIndex)
      ) {
        return;
      }

      disposePromptDecorationSet(decorationSet);
      promptDecorations.delete(decoratedLineIndex);
    });
  };

  const updatePinnedCommand = (terminal: Terminal, activeProfile: TerminalDecorationProfile) => {
    const buffer = terminal.buffer.active;
    if (!activeProfile.pinnedCommand || buffer.type === "alternate") {
      clearPinnedCommand();
      return;
    }

    const cachedContext = pinnedCommandContext;
    if (cachedContext && !cachedContext.marker.isDisposed) {
      const commandEndLineIndex = cachedContext.marker.line + cachedContext.commandLineCount - 1;
      const crossedPrompt =
        buffer.viewportY > cachedContext.viewportY &&
        hasTerminalPromptInRange(
          buffer,
          activeProfile,
          Math.max(commandEndLineIndex + 1, cachedContext.viewportY),
          buffer.viewportY
        );

      if (buffer.viewportY > commandEndLineIndex && !crossedPrompt) {
        cachedContext.viewportY = buffer.viewportY;
        onPinnedCommandChange(cachedContext.pinnedCommand);
        return;
      }
    }

    clearPinnedCommand();
    const pinnedCommand = findTerminalPinnedCommand(buffer, activeProfile);
    if (!pinnedCommand) return;

    const cursorLineIndex = buffer.baseY + buffer.cursorY;
    const marker = terminal.registerMarker(pinnedCommand.promptLineIndex - cursorLineIndex);
    if (marker) {
      pinnedCommandContext = {
        marker,
        commandLineCount: pinnedCommand.commandLineCount,
        pinnedCommand,
        viewportY: buffer.viewportY,
      };
    }
    onPinnedCommandChange(pinnedCommand);
  };

  const decorateNow = (terminal: Terminal) => {
    if (!profile) return;
    decoratePrompts(terminal, profile);
    decorateErrors(terminal, profile);
    updatePinnedCommand(terminal, profile);
  };

  const clear = () => {
    cancelScheduledDecoration();
    clearDecorationSets();
    clearPinnedCommand();
  };

  const setProfile = (nextProfile: TerminalDecorationProfile | null, terminal?: Terminal) => {
    if (profile === nextProfile) {
      if (terminal && profile) decorateNow(terminal);
      return;
    }

    clear();
    profile = nextProfile;
    if (terminal && profile) decorateNow(terminal);
  };

  const schedule = (terminal: Terminal, rebuild = false) => {
    if (!profile) return;
    decorationRebuild = decorationRebuild || rebuild;
    if (decorationFrame !== null) return;

    decorationFrame = requestFrame(() => {
      decorationFrame = null;
      if (decorationRebuild) {
        clearDecorationSets();
        decorationRebuild = false;
      }
      decorateNow(terminal);
    });
  };

  return { setProfile, schedule, decorateNow, clear };
}
