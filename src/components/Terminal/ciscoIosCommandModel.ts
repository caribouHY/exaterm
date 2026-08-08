import type { TerminalMode } from "../../types";

export interface CiscoIosBufferLine {
  isWrapped: boolean;
  translateToString: (trimRight?: boolean) => string;
}

export interface CiscoIosBuffer {
  type: "normal" | "alternate";
  viewportY: number;
  length: number;
  getLine: (lineIndex: number) => CiscoIosBufferLine | undefined;
}

export interface CiscoIosPrompt {
  hostname: string;
  promptText: string;
  commandText: string;
  commandStart: number;
  commandSeparator: string;
  isConfigPrompt: boolean;
}

export interface CiscoIosCommandSegment {
  lineIndex: number;
  x: number;
  width: number;
  text: string;
}

export interface CiscoIosPinnedCommand {
  displayText: string;
  promptText: string;
  commandText: string;
  isConfigPrompt: boolean;
  promptLineIndex: number;
  commandLineCount: number;
}

function isCiscoIosHostnameCharacter(character: string): boolean {
  const codePoint = character.charCodeAt(0);
  const isAsciiLetter =
    (codePoint >= 65 && codePoint <= 90) || (codePoint >= 97 && codePoint <= 122);
  const isDigit = codePoint >= 48 && codePoint <= 57;
  return isAsciiLetter || isDigit || "_+-.:/[]".includes(character);
}

export function parseCiscoIosPrompt(line: string): CiscoIosPrompt | null {
  const trimmedLine = line.trimEnd();
  let cursor = 0;
  while (cursor < trimmedLine.length && isCiscoIosHostnameCharacter(trimmedLine.charAt(cursor))) {
    cursor += 1;
  }
  if (cursor === 0) return null;

  const hostname = trimmedLine.slice(0, cursor);
  const modes: string[] = [];
  while (modes.length < 3 && trimmedLine.charAt(cursor) === "(") {
    const closingParenthesis = trimmedLine.indexOf(")", cursor + 1);
    if (closingParenthesis <= cursor + 1) return null;
    modes.push(trimmedLine.slice(cursor + 1, closingParenthesis));
    cursor = closingParenthesis + 1;
  }

  const terminator = trimmedLine.charAt(cursor);
  if (terminator !== ">" && terminator !== "#") return null;

  const promptEnd = cursor + 1;
  const promptText = trimmedLine.slice(0, promptEnd);
  const commandStart = promptEnd + (trimmedLine.charAt(promptEnd) === " " ? 1 : 0);
  const [lastMode = ""] = modes.slice(-1);

  return {
    hostname,
    promptText,
    commandText: trimmedLine.slice(commandStart),
    commandStart,
    commandSeparator: trimmedLine.slice(promptText.length, commandStart),
    isConfigPrompt: terminator === "#" && (lastMode === "config" || lastMode.startsWith("config-")),
  };
}

export function collectCiscoIosCommandSegments(
  buffer: CiscoIosBuffer,
  promptLineIndex: number,
  commandStart: number,
  commandText: string,
  lastLineIndex: number
): CiscoIosCommandSegment[] {
  if (buffer.type === "alternate") return [];

  const segments: CiscoIosCommandSegment[] = [];
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

export function findCiscoIosPinnedCommand(
  buffer: CiscoIosBuffer,
  terminalMode: TerminalMode
): CiscoIosPinnedCommand | null {
  if (terminalMode !== "cisco_ios" || buffer.type === "alternate" || buffer.viewportY <= 0) {
    return null;
  }

  const firstVisibleLineIndex = Math.min(buffer.viewportY, buffer.length - 1);
  const firstVisibleLine = buffer.getLine(firstVisibleLineIndex);
  if (
    firstVisibleLine &&
    !firstVisibleLine.isWrapped &&
    parseCiscoIosPrompt(firstVisibleLine.translateToString(true))
  ) {
    return null;
  }

  for (let lineIndex = firstVisibleLineIndex - 1; lineIndex >= 0; lineIndex -= 1) {
    const line = buffer.getLine(lineIndex);
    if (!line || line.isWrapped) continue;

    const prompt = parseCiscoIosPrompt(line.translateToString(true));
    if (!prompt) continue;

    const segments = collectCiscoIosCommandSegments(
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
    return {
      displayText: `${prompt.promptText}${commandText}`,
      promptText: prompt.promptText,
      commandText,
      isConfigPrompt: prompt.isConfigPrompt,
      promptLineIndex: lineIndex,
      commandLineCount: commandEndLineIndex - lineIndex + 1,
    };
  }

  return null;
}

export function hasCiscoIosPromptInRange(
  buffer: CiscoIosBuffer,
  firstLineIndex: number,
  lastLineIndex: number
): boolean {
  if (buffer.type === "alternate") return false;

  const first = Math.max(0, firstLineIndex);
  const last = Math.min(buffer.length - 1, lastLineIndex);
  for (let lineIndex = first; lineIndex <= last; lineIndex += 1) {
    const line = buffer.getLine(lineIndex);
    if (!line || line.isWrapped) continue;
    if (parseCiscoIosPrompt(line.translateToString(true))) return true;
  }

  return false;
}
