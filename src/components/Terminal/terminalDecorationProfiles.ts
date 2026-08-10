import type { TerminalMode } from "../../types";
import type {
  TerminalDecorationProfile,
  TerminalParsedContext,
  TerminalParsedPrompt,
} from "./terminalDecorationTypes";

export interface CiscoIosPrompt extends TerminalParsedPrompt {
  hostname: string;
}

export interface AristaEosPrompt extends TerminalParsedPrompt {
  hostname: string;
}

export interface VyosPrompt extends TerminalParsedPrompt {
  hostname: string;
  username: string;
}

export interface FujitsuSirPrompt extends TerminalParsedPrompt {
  hostname: string;
  configurationFile: "config1" | "config2";
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

const ARISTA_EOS_ERROR_PREFIXES = [
  "% ambiguous command",
  "% incomplete command",
  "% invalid input",
];

const VYOS_ERROR_PREFIXES = ["set failed", "commit failed", "cannot exit:", "invalid command"];

function isNetworkDeviceHostnameCharacter(character: string): boolean {
  const codePoint = character.charCodeAt(0);
  const isAsciiLetter =
    (codePoint >= 65 && codePoint <= 90) || (codePoint >= 97 && codePoint <= 122);
  const isDigit = codePoint >= 48 && codePoint <= 57;
  return isAsciiLetter || isDigit || "_+-.:/[]".includes(character);
}

function parseNetworkDevicePrompt(
  line: string
): (TerminalParsedPrompt & { hostname: string }) | null {
  const trimmedLine = line.trimEnd();
  let cursor = 0;
  while (
    cursor < trimmedLine.length &&
    isNetworkDeviceHostnameCharacter(trimmedLine.charAt(cursor))
  ) {
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

export function parseCiscoIosPrompt(line: string): CiscoIosPrompt | null {
  return parseNetworkDevicePrompt(line);
}

export function parseAristaEosPrompt(line: string): AristaEosPrompt | null {
  return parseNetworkDevicePrompt(line);
}

function isFujitsuSirHostnameCharacter(character: string): boolean {
  const codePoint = character.charCodeAt(0);
  const isAsciiLetter =
    (codePoint >= 65 && codePoint <= 90) || (codePoint >= 97 && codePoint <= 122);
  const isDigit = codePoint >= 48 && codePoint <= 57;
  return isAsciiLetter || isDigit || "_.-".includes(character);
}

function findFujitsuSirHostnameEnd(line: string): number {
  if (line.startsWith("Si-R G")) {
    let cursor = "Si-R G".length;
    const modelStart = cursor;
    const firstModelCharacter = line.charAt(modelStart);
    const firstModelCodePoint = firstModelCharacter.charCodeAt(0);
    if (firstModelCodePoint < 48 || firstModelCodePoint > 57) return 0;
    while (cursor < line.length && isFujitsuSirHostnameCharacter(line.charAt(cursor))) {
      cursor += 1;
    }
    return cursor > modelStart ? cursor : 0;
  }

  let cursor = 0;
  while (cursor < line.length && isFujitsuSirHostnameCharacter(line.charAt(cursor))) {
    cursor += 1;
  }
  return cursor;
}

export function parseFujitsuSirPrompt(line: string): FujitsuSirPrompt | null {
  const trimmedLine = line.trimEnd();
  let cursor = findFujitsuSirHostnameEnd(trimmedLine);
  if (cursor === 0) return null;

  const hostname = trimmedLine.slice(0, cursor);
  let configurationFile: FujitsuSirPrompt["configurationFile"] = "config1";
  if (trimmedLine.slice(cursor, cursor + 8) === " config2") {
    configurationFile = "config2";
    cursor += 8;
  } else if (trimmedLine.charAt(cursor) === " " && "(>#".includes(trimmedLine.charAt(cursor + 1))) {
    cursor += 1;
  }

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
    configurationFile,
    promptText,
    promptStart: 0,
    commandText: trimmedLine.slice(commandStart),
    commandStart,
    commandSeparator: trimmedLine.slice(promptText.length, commandStart),
    variant: isConfigurationPrompt ? "configuration" : "default",
  };
}

function isVyosIdentityCharacter(character: string): boolean {
  const codePoint = character.charCodeAt(0);
  const isAsciiLetter =
    (codePoint >= 65 && codePoint <= 90) || (codePoint >= 97 && codePoint <= 122);
  const isDigit = codePoint >= 48 && codePoint <= 57;
  return isAsciiLetter || isDigit || "_.-".includes(character);
}

export function parseVyosPrompt(line: string): VyosPrompt | null {
  const trimmedLine = line.trimEnd();
  let cursor = 0;
  while (cursor < trimmedLine.length && isVyosIdentityCharacter(trimmedLine.charAt(cursor))) {
    cursor += 1;
  }
  if (cursor === 0 || trimmedLine.charAt(cursor) !== "@") return null;

  const username = trimmedLine.slice(0, cursor);
  cursor += 1;
  const hostnameStart = cursor;
  while (cursor < trimmedLine.length && isVyosIdentityCharacter(trimmedLine.charAt(cursor))) {
    cursor += 1;
  }
  if (cursor === hostnameStart) return null;

  const hostname = trimmedLine.slice(hostnameStart, cursor);
  if (trimmedLine.slice(cursor, cursor + 2) === ":~") cursor += 2;

  const terminator = trimmedLine.charAt(cursor);
  if (terminator !== "$" && terminator !== "#") return null;

  const promptEnd = cursor + 1;
  const promptText = trimmedLine.slice(0, promptEnd);
  const commandStart = promptEnd + (trimmedLine.charAt(promptEnd) === " " ? 1 : 0);
  return {
    hostname,
    username,
    promptText,
    promptStart: 0,
    commandText: trimmedLine.slice(commandStart),
    commandStart,
    commandSeparator: trimmedLine.slice(promptText.length, commandStart),
    variant: terminator === "#" ? "configuration" : "default",
  };
}

export function parseVyosContextLine(line: string): TerminalParsedContext | null {
  const trimmedLine = line.trimEnd();
  let contextStart = 0;
  while (contextStart < trimmedLine.length && trimmedLine.charAt(contextStart) === " ") {
    contextStart += 1;
  }

  const contextText = trimmedLine.slice(contextStart);
  if (!contextText.startsWith("[edit") || !contextText.endsWith("]")) return null;
  if (contextText !== "[edit]" && !contextText.startsWith("[edit ")) return null;
  if (contextText === "[edit ]") return null;

  return { contextText, contextStart, variant: "configuration" };
}

export const CISCO_IOS_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "cisco_ios",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseCiscoIosPrompt,
  isErrorLine: (line) => CISCO_IOS_ERROR_PATTERNS.some((pattern) => pattern.test(line)),
};

export const ARISTA_EOS_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "arista_eos",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseAristaEosPrompt,
  isErrorLine: (line) => {
    const normalizedLine = line.trimStart().toLowerCase();
    return ARISTA_EOS_ERROR_PREFIXES.some((prefix) => normalizedLine.startsWith(prefix));
  },
};

export const VYOS_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "vyos",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseVyosPrompt,
  parseContextLine: parseVyosContextLine,
  isErrorLine: (line) => {
    const normalizedLine = line.trimStart().toLowerCase();
    return (
      VYOS_ERROR_PREFIXES.some((prefix) => normalizedLine.startsWith(prefix)) ||
      (normalizedLine.startsWith("configuration path") && normalizedLine.endsWith("does not exist"))
    );
  },
};

export const FUJITSU_SIR_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "fujitsu_sir",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseFujitsuSirPrompt,
  isErrorLine: (line) => line.startsWith("<ERROR>"),
};

const TERMINAL_DECORATION_PROFILES = new Map<TerminalMode, TerminalDecorationProfile>([
  ["cisco_ios", CISCO_IOS_DECORATION_PROFILE],
  ["arista_eos", ARISTA_EOS_DECORATION_PROFILE],
  ["vyos", VYOS_DECORATION_PROFILE],
  ["fujitsu_sir", FUJITSU_SIR_DECORATION_PROFILE],
]);

export function getTerminalDecorationProfile(
  terminalMode: TerminalMode
): TerminalDecorationProfile | null {
  return TERMINAL_DECORATION_PROFILES.get(terminalMode) ?? null;
}
