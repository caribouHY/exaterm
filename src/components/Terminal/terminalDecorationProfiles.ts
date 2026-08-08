import type { TerminalMode } from "../../types";
import type { TerminalDecorationProfile, TerminalParsedPrompt } from "./terminalDecorationTypes";

export interface CiscoIosPrompt extends TerminalParsedPrompt {
  hostname: string;
}

const CISCO_IOS_ERROR_PATTERNS = [
  /ERROR:/i,
  /% ?Bad secret/,
  /(?:^|%) Bad passwords/,
  /invalid input/i,
  /(?:incomplete|ambiguous) command/i,
  /connection timed out/i,
  /[^\r\n]+ not found/,
  /'[^']+' +returned error code: ?\d+/,
  /Bad mask/i,
  /% ?\S+ ?overlaps with ?\S+/i,
  /% ?\S+ ?Error: ?\s+/i,
  /% ?\S+ ?Informational: ?\s+/i,
  /Command authorization failed/,
  /Command Rejected[ \t]*:[ \t]+/i,
  /Command Rejected[ \t]*\([^\r\n)]*\)[ \t]*:[ \t]+/i,
  /% General session commands not allowed under the address family/i,
  /% BGP: Error initializing topology/i,
  /%SNMP agent not enabled/i,
  /% Invalid/i,
  /%You must disable VTPv1 and VTPv2 or switch to VTPv3 before configuring a VLAN name longer than 32 characters/i,
];

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
  const isConfigurationPrompt =
    terminator === "#" && (lastMode === "config" || lastMode.startsWith("config-"));

  return {
    hostname,
    promptText,
    promptStart: 0,
    commandText: trimmedLine.slice(commandStart),
    commandStart,
    commandSeparator: trimmedLine.slice(promptText.length, commandStart),
    variant: isConfigurationPrompt ? "configuration" : "default",
  };
}

export const CISCO_IOS_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "cisco_ios",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseCiscoIosPrompt,
  isErrorLine: (line) => CISCO_IOS_ERROR_PATTERNS.some((pattern) => pattern.test(line)),
};

const TERMINAL_DECORATION_PROFILES = new Map<TerminalMode, TerminalDecorationProfile>([
  ["cisco_ios", CISCO_IOS_DECORATION_PROFILE],
]);

export function getTerminalDecorationProfile(
  terminalMode: TerminalMode
): TerminalDecorationProfile | null {
  return TERMINAL_DECORATION_PROFILES.get(terminalMode) ?? null;
}
