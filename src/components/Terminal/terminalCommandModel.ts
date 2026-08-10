import type {
  TerminalBufferLike,
  TerminalCommandSegment,
  TerminalDecorationProfile,
  TerminalPinnedCommand,
} from "./terminalDecorationTypes";

export function collectTerminalCommandSegments(
  buffer: TerminalBufferLike,
  promptLineIndex: number,
  commandStart: number,
  commandText: string,
  lastLineIndex: number
): TerminalCommandSegment[] {
  if (buffer.type === "alternate") return [];

  const segments: TerminalCommandSegment[] = [];
  if (commandText.length > 0) {
    segments.push({
      lineIndex: promptLineIndex,
      x: commandStart,
      width: commandText.length,
      text: commandText,
    });
  }

  for (let lineIndex = promptLineIndex + 1; lineIndex <= lastLineIndex; lineIndex += 1) {
    const wrappedLine = buffer.getLine(lineIndex);
    if (!wrappedLine?.isWrapped) break;

    const wrappedText = wrappedLine.translateToString(true).trimEnd();
    if (wrappedText.length > 0) {
      segments.push({
        lineIndex,
        x: 0,
        width: wrappedText.length,
        text: wrappedText,
      });
    }
  }

  return segments;
}

export function findTerminalPinnedCommand(
  buffer: TerminalBufferLike,
  profile: TerminalDecorationProfile
): TerminalPinnedCommand | null {
  if (!profile.pinnedCommand || buffer.type === "alternate" || buffer.viewportY <= 0) {
    return null;
  }

  const firstVisibleLineIndex = Math.min(buffer.viewportY, buffer.length - 1);
  const firstVisibleLine = buffer.getLine(firstVisibleLineIndex);
  if (
    firstVisibleLine &&
    !firstVisibleLine.isWrapped &&
    profile.parsePrompt(firstVisibleLine.translateToString(true))
  ) {
    return null;
  }

  for (let lineIndex = firstVisibleLineIndex - 1; lineIndex >= 0; lineIndex -= 1) {
    const line = buffer.getLine(lineIndex);
    if (!line || line.isWrapped) continue;

    const prompt = profile.parsePrompt(line.translateToString(true));
    if (!prompt) continue;

    const segments = collectTerminalCommandSegments(
      buffer,
      lineIndex,
      prompt.commandStart,
      prompt.commandText,
      buffer.length - 1
    );
    if (segments.length === 0) return null;

    const commandEndLineIndex = segments[segments.length - 1].lineIndex;
    if (commandEndLineIndex >= buffer.viewportY) return null;

    const commandText = `${prompt.commandSeparator}${segments
      .map((segment) => segment.text)
      .join("")}`;
    const contextLine = buffer.getLine(lineIndex - 1);
    const context =
      prompt.variant === "configuration" && contextLine && !contextLine.isWrapped
        ? profile.parseContextLine?.(contextLine.translateToString(true))
        : null;
    return {
      displayText: `${prompt.promptText}${commandText}`,
      ...(context ? { contextText: context.contextText } : {}),
      promptText: prompt.promptText,
      commandText,
      promptVariant: prompt.variant,
      promptLineIndex: lineIndex,
      commandLineCount: commandEndLineIndex - lineIndex + 1,
    };
  }

  return null;
}

export function hasTerminalPromptInRange(
  buffer: TerminalBufferLike,
  profile: TerminalDecorationProfile,
  firstLineIndex: number,
  lastLineIndex: number
): boolean {
  if (buffer.type === "alternate") return false;

  const first = Math.max(0, firstLineIndex);
  const last = Math.min(buffer.length - 1, lastLineIndex);
  for (let lineIndex = first; lineIndex <= last; lineIndex += 1) {
    const line = buffer.getLine(lineIndex);
    if (!line || line.isWrapped) continue;
    if (profile.parsePrompt(line.translateToString(true))) return true;
  }

  return false;
}
