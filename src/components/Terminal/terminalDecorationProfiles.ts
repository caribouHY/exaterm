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

export interface JuniperJunosPrompt extends TerminalParsedPrompt {
  hostname: string;
  username: string;
}

export interface FujitsuSirPrompt extends TerminalParsedPrompt {
  hostname: string;
  configurationFile: "config1" | "config2";
}

export interface AlliedTelesisAwplusPrompt extends TerminalParsedPrompt {
  hostname: string;
  mode: string | null;
}

export interface FurukawaFitelnetPrompt extends TerminalParsedPrompt {
  hostname: string;
  mode: string | null;
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
const JUNIPER_JUNOS_ERROR_PREFIXES = [
  "error:",
  "syntax error,",
  "syntax error ",
  "unknown command.",
];

interface AlliedTelesisAwplusMessage {
  hasPercentPrefix: boolean;
  lowerText: string;
  text: string;
}

type AlliedTelesisAwplusErrorPattern = (message: AlliedTelesisAwplusMessage) => boolean;

function parseAlliedTelesisAwplusMessage(line: string): AlliedTelesisAwplusMessage {
  let text = line.trim();
  const hasPercentPrefix = text.startsWith("%");
  if (hasPercentPrefix) text = text.slice(1).trimStart();
  return { hasPercentPrefix, lowerText: text.toLowerCase(), text };
}

function startsWithAlliedTelesisAwplusBoundary(
  text: string,
  prefix: string,
  boundaryCharacters: string
): boolean {
  if (!text.startsWith(prefix)) return false;
  const boundary = text.charAt(prefix.length);
  return (
    boundary === "" || boundaryCharacters.includes(boundary) || isWhitespaceCharacter(boundary)
  );
}

function isWhitespaceCharacter(character: string): boolean {
  return character !== "" && character.trim() === "";
}

function isNonWhitespaceText(text: string): boolean {
  if (text.length === 0) return false;
  for (const character of text) {
    if (isWhitespaceCharacter(character)) return false;
  }
  return true;
}

function isAsciiDigits(text: string): boolean {
  if (text.length === 0) return false;
  for (const character of text) {
    if (character < "0" || character > "9") return false;
  }
  return true;
}

function matchesAlliedTelesisAwplusReturnedErrorCode(text: string): boolean {
  if (!text.startsWith("'")) return false;
  const closingQuote = text.indexOf("'", 1);
  if (closingQuote < 3 || !isWhitespaceCharacter(text.charAt(closingQuote + 1))) return false;

  const suffix = text.slice(closingQuote + 1).trimStart();
  const prefix = "returned error code:";
  if (!suffix.toLowerCase().startsWith(prefix)) return false;
  return isAsciiDigits(suffix.slice(prefix.length).trim());
}

function matchesAlliedTelesisAwplusNotFound(message: AlliedTelesisAwplusMessage): boolean {
  if (!message.hasPercentPrefix) return false;
  const suffix = message.lowerText.endsWith(" not found.") ? " not found." : " not found";
  if (!message.lowerText.endsWith(suffix)) return false;
  return message.text.slice(0, -suffix.length).trim().length > 0;
}

function matchesAlliedTelesisAwplusOverlap(message: AlliedTelesisAwplusMessage): boolean {
  if (!message.hasPercentPrefix) return false;
  const separator = " overlaps with ";
  const separatorIndex = message.lowerText.indexOf(separator);
  if (separatorIndex <= 0) return false;
  return (
    isNonWhitespaceText(message.text.slice(0, separatorIndex)) &&
    isNonWhitespaceText(message.text.slice(separatorIndex + separator.length))
  );
}

function matchesAlliedTelesisAwplusDetailedError(message: AlliedTelesisAwplusMessage): boolean {
  if (!message.hasPercentPrefix) return false;
  const separator = " error:";
  const separatorIndex = message.lowerText.indexOf(separator);
  if (separatorIndex <= 0) return false;
  return (
    isNonWhitespaceText(message.text.slice(0, separatorIndex)) &&
    message.text.slice(separatorIndex + separator.length).trim().length > 0
  );
}

const ALLIED_TELESIS_AWPLUS_ERROR_PATTERNS: AlliedTelesisAwplusErrorPattern[] = [
  ({ hasPercentPrefix, lowerText }) => hasPercentPrefix && lowerText === "incomplete command.",
  ({ hasPercentPrefix, lowerText }) =>
    hasPercentPrefix && lowerText === "invalid input detected at '^' marker.",
  ({ hasPercentPrefix, lowerText }) =>
    hasPercentPrefix && startsWithAlliedTelesisAwplusBoundary(lowerText, "can't find", ""),
  ({ hasPercentPrefix, lowerText }) => hasPercentPrefix && lowerText === "unrecognized command",
  ({ hasPercentPrefix, lowerText }) => !hasPercentPrefix && lowerText === "login incorrect",
  ({ hasPercentPrefix, lowerText }) =>
    hasPercentPrefix && lowerText === "working set must contain only single node for this command",
  ({ hasPercentPrefix, lowerText }) =>
    hasPercentPrefix && startsWithAlliedTelesisAwplusBoundary(lowerText, "error", ":."),
  ({ hasPercentPrefix, lowerText }) =>
    hasPercentPrefix && startsWithAlliedTelesisAwplusBoundary(lowerText, "bad secret", ":."),
  ({ lowerText }) => startsWithAlliedTelesisAwplusBoundary(lowerText, "bad passwords", ":."),
  ({ lowerText }) => startsWithAlliedTelesisAwplusBoundary(lowerText, "ambiguous command", "."),
  ({ lowerText }) => startsWithAlliedTelesisAwplusBoundary(lowerText, "connection timed out", "."),
  matchesAlliedTelesisAwplusNotFound,
  ({ text }) => matchesAlliedTelesisAwplusReturnedErrorCode(text),
  ({ lowerText }) => startsWithAlliedTelesisAwplusBoundary(lowerText, "bad mask", ":."),
  matchesAlliedTelesisAwplusOverlap,
  matchesAlliedTelesisAwplusDetailedError,
  ({ lowerText }) =>
    startsWithAlliedTelesisAwplusBoundary(lowerText, "command authorization failed", ":."),
];

const FURUKAWA_FITELNET_EXACT_ERRORS = new Set([
  "% Can not refresh",
  "% Command failed.",
  "% Entry not found.",
  "% Invalid source address",
  "% Invalid default ICMP source address",
  "% Please answer 'yes or 'no'.",
  "% Bad minimum size",
  "% Bad maximum size",
  "% Bad Interval size",
  "% Only one source route option allowed",
  "% No room for that option",
  "% Up to 9 routes can be specified",
  "% Invalid Number of Hops",
  "% Invalid Number of Timestamps",
  "% No such VRF",
  "This command cannot be executed.",
  "Time out, Operation failed.",
  "****Warning! sendto failed***",
]);

function hasNonEmptyMarkerPayload(line: string, marker: string): boolean {
  return line.startsWith(marker) && line.slice(marker.length).trim().length > 0;
}

function matchesFitelnetDecimalRange(line: string): boolean {
  const prefix = "% A decimal number between ";
  if (!line.startsWith(prefix) || !line.endsWith(".")) return false;
  const range = line.slice(prefix.length, -1);
  const separator = " and ";
  const separatorIndex = range.indexOf(separator);
  if (separatorIndex <= 0 || range.indexOf(separator, separatorIndex + separator.length) !== -1) {
    return false;
  }
  return (
    isAsciiDigits(range.slice(0, separatorIndex)) &&
    isAsciiDigits(range.slice(separatorIndex + separator.length))
  );
}

function isAsciiHexDigits(text: string): boolean {
  if (text.length === 0) return false;
  for (const character of text) {
    const lowerCharacter = character.toLowerCase();
    if (!(isAsciiDigits(character) || (lowerCharacter >= "a" && lowerCharacter <= "f"))) {
      return false;
    }
  }
  return true;
}

function matchesFitelnetHexRange(line: string): boolean {
  const prefix = "% A Hex number between 0x";
  if (!line.startsWith(prefix) || !line.endsWith(".")) return false;
  const range = line.slice(prefix.length, -1);
  const separator = " and 0x";
  const separatorIndex = range.indexOf(separator);
  if (separatorIndex <= 0 || range.indexOf(separator, separatorIndex + separator.length) !== -1) {
    return false;
  }
  return (
    isAsciiHexDigits(range.slice(0, separatorIndex)) &&
    isAsciiHexDigits(range.slice(separatorIndex + separator.length))
  );
}

function matchesFitelnetCannotResolve(line: string): boolean {
  const prefix = '% Cannot resolve "';
  if (!line.startsWith(prefix) || !line.endsWith(")")) return false;
  const closingQuote = line.indexOf('"', prefix.length);
  if (closingQuote === -1 || closingQuote === prefix.length) return false;
  const reasonStart = closingQuote + 2;
  return (
    line.charAt(closingQuote + 1) === " " &&
    line.charAt(reasonStart) === "(" &&
    reasonStart + 1 < line.length - 1
  );
}

function matchesFitelnetUnknownValue(line: string, prefix: string): boolean {
  if (!line.startsWith(prefix)) return false;
  return isNonWhitespaceText(line.slice(prefix.length));
}

function matchesFitelnetUnknownProtocol(line: string): boolean {
  const prefix = 'Unknown protocol -"';
  const suffix = '", type ping ? for help';
  return (
    line.startsWith(prefix) && line.endsWith(suffix) && line.length > prefix.length + suffix.length
  );
}

function matchesFitelnetPacketTooShort(line: string): boolean {
  const prefix = "packet too short (";
  const separator = " bytes) from ";
  if (!line.startsWith(prefix)) return false;
  const separatorIndex = line.indexOf(separator, prefix.length);
  return (
    separatorIndex > prefix.length &&
    isAsciiDigits(line.slice(prefix.length, separatorIndex)) &&
    isNonWhitespaceText(line.slice(separatorIndex + separator.length))
  );
}

function matchesFitelnetWrongTotalLength(line: string): boolean {
  const prefix = "wrong total length ";
  const separator = " instead of ";
  if (!line.startsWith(prefix)) return false;
  const separatorIndex = line.indexOf(separator, prefix.length);
  return (
    separatorIndex > prefix.length &&
    isAsciiDigits(line.slice(prefix.length, separatorIndex)) &&
    isAsciiDigits(line.slice(separatorIndex + separator.length))
  );
}

function matchesFitelnetWrongDataByte(line: string): boolean {
  const prefix = "wrong data byte #";
  const expectedSeparator = " should have been ";
  const actualSeparator = " but was ";
  if (!line.startsWith(prefix)) return false;
  const expectedIndex = line.indexOf(expectedSeparator, prefix.length);
  if (expectedIndex <= prefix.length) return false;
  const actualIndex = line.indexOf(actualSeparator, expectedIndex + expectedSeparator.length);
  if (actualIndex <= expectedIndex + expectedSeparator.length) return false;
  return [
    line.slice(prefix.length, expectedIndex),
    line.slice(expectedIndex + expectedSeparator.length, actualIndex),
    line.slice(actualIndex + actualSeparator.length),
  ].every(isNonWhitespaceText);
}

function isFurukawaFitelnetErrorLine(line: string): boolean {
  const trimmedLine = line.trimEnd();
  return (
    hasNonEmptyMarkerPayload(trimmedLine, "ERROR:") ||
    hasNonEmptyMarkerPayload(trimmedLine, "<ERROR>") ||
    FURUKAWA_FITELNET_EXACT_ERRORS.has(trimmedLine) ||
    matchesFitelnetDecimalRange(trimmedLine) ||
    matchesFitelnetHexRange(trimmedLine) ||
    matchesFitelnetCannotResolve(trimmedLine) ||
    matchesFitelnetUnknownProtocol(trimmedLine) ||
    matchesFitelnetUnknownValue(trimmedLine, "Unknown output interface ") ||
    matchesFitelnetUnknownValue(trimmedLine, "Unknown source interface ") ||
    matchesFitelnetPacketTooShort(trimmedLine) ||
    matchesFitelnetWrongTotalLength(trimmedLine) ||
    matchesFitelnetWrongDataByte(trimmedLine) ||
    (trimmedLine.startsWith("unknown option 0x") &&
      isAsciiHexDigits(trimmedLine.slice("unknown option 0x".length)))
  );
}

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

function isAlliedTelesisAwplusModeCharacter(character: string): boolean {
  const codePoint = character.charCodeAt(0);
  const isAsciiLetter =
    (codePoint >= 65 && codePoint <= 90) || (codePoint >= 97 && codePoint <= 122);
  const isDigit = codePoint >= 48 && codePoint <= 57;
  return isAsciiLetter || isDigit || character === "-" || character === "_";
}

export function parseAlliedTelesisAwplusPrompt(line: string): AlliedTelesisAwplusPrompt | null {
  const trimmedLine = line.trimEnd();
  let cursor = 0;
  while (cursor < trimmedLine.length && cursor < 64) {
    const character = trimmedLine.charAt(cursor);
    if (character === "(" || character === ">" || character === "#") break;
    if (/\s/.test(character) || character === "?" || character === ")") return null;
    cursor += 1;
  }
  if (cursor === 0 || cursor >= trimmedLine.length) return null;

  const hostname = trimmedLine.slice(0, cursor);
  let mode: string | null = null;
  if (trimmedLine.charAt(cursor) === "(") {
    const modeStart = cursor + 1;
    const closingParenthesis = trimmedLine.indexOf(")", modeStart);
    if (closingParenthesis === -1 || closingParenthesis === modeStart) return null;

    mode = trimmedLine.slice(modeStart, closingParenthesis);
    if (
      mode.length > 64 ||
      Array.from(mode).some((character) => !isAlliedTelesisAwplusModeCharacter(character))
    ) {
      return null;
    }
    cursor = closingParenthesis + 1;
  }

  const terminator = trimmedLine.charAt(cursor);
  if (terminator !== ">" && terminator !== "#") return null;
  if (mode !== null && terminator !== "#") return null;

  const promptEnd = cursor + 1;
  const promptText = trimmedLine.slice(0, promptEnd);
  const commandStart = promptEnd + (trimmedLine.charAt(promptEnd) === " " ? 1 : 0);
  return {
    hostname,
    mode,
    promptText,
    promptStart: 0,
    commandText: trimmedLine.slice(commandStart),
    commandStart,
    commandSeparator: trimmedLine.slice(promptText.length, commandStart),
    variant: mode === null ? "default" : "configuration",
  };
}

function isFurukawaFitelnetHostnameCharacter(character: string): boolean {
  const codePoint = character.charCodeAt(0);
  const isAsciiLetter =
    (codePoint >= 65 && codePoint <= 90) || (codePoint >= 97 && codePoint <= 122);
  const isDigit = codePoint >= 48 && codePoint <= 57;
  return isAsciiLetter || isDigit || "_.-".includes(character);
}

function isFurukawaFitelnetModeCharacter(character: string): boolean {
  return isFurukawaFitelnetHostnameCharacter(character) || " /:".includes(character);
}

export function parseFurukawaFitelnetPrompt(line: string): FurukawaFitelnetPrompt | null {
  const trimmedLine = line.trimEnd();
  let cursor = 0;
  while (
    cursor < trimmedLine.length &&
    cursor < 254 &&
    isFurukawaFitelnetHostnameCharacter(trimmedLine.charAt(cursor))
  ) {
    cursor += 1;
  }
  if (cursor === 254 && isFurukawaFitelnetHostnameCharacter(trimmedLine.charAt(cursor))) {
    return null;
  }

  const hostname = trimmedLine.slice(0, cursor);
  let mode: string | null = null;
  if (trimmedLine.charAt(cursor) === "(") {
    const modeStart = cursor + 1;
    const closingParenthesis = trimmedLine.indexOf(")", modeStart);
    if (closingParenthesis === -1 || closingParenthesis === modeStart) return null;

    mode = trimmedLine.slice(modeStart, closingParenthesis);
    if (
      mode.length > 128 ||
      (mode !== "config" && (!mode.startsWith("config-") || mode === "config-")) ||
      mode.endsWith(" ") ||
      Array.from(mode).some((character) => !isFurukawaFitelnetModeCharacter(character))
    ) {
      return null;
    }
    cursor = closingParenthesis + 1;
  }

  const terminator = trimmedLine.charAt(cursor);
  if (terminator !== ">" && terminator !== "#") return null;
  if (mode !== null && terminator !== "#") return null;

  const promptEnd = cursor + 1;
  const promptText = trimmedLine.slice(0, promptEnd);
  const commandStart = promptEnd + (trimmedLine.charAt(promptEnd) === " " ? 1 : 0);
  return {
    hostname,
    mode,
    promptText,
    promptStart: 0,
    commandText: trimmedLine.slice(commandStart),
    commandStart,
    commandSeparator: trimmedLine.slice(promptText.length, commandStart),
    variant: mode === null ? "default" : "configuration",
  };
}

function isNetworkOsIdentityCharacter(character: string): boolean {
  const codePoint = character.charCodeAt(0);
  const isAsciiLetter =
    (codePoint >= 65 && codePoint <= 90) || (codePoint >= 97 && codePoint <= 122);
  const isDigit = codePoint >= 48 && codePoint <= 57;
  return isAsciiLetter || isDigit || "_.-".includes(character);
}

export function parseVyosPrompt(line: string): VyosPrompt | null {
  const trimmedLine = line.trimEnd();
  let cursor = 0;
  while (cursor < trimmedLine.length && isNetworkOsIdentityCharacter(trimmedLine.charAt(cursor))) {
    cursor += 1;
  }
  if (cursor === 0 || trimmedLine.charAt(cursor) !== "@") return null;

  const username = trimmedLine.slice(0, cursor);
  cursor += 1;
  const hostnameStart = cursor;
  while (cursor < trimmedLine.length && isNetworkOsIdentityCharacter(trimmedLine.charAt(cursor))) {
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

export function parseJuniperJunosPrompt(line: string): JuniperJunosPrompt | null {
  const trimmedLine = line.trimEnd();
  let cursor = 0;
  while (cursor < trimmedLine.length && isNetworkOsIdentityCharacter(trimmedLine.charAt(cursor))) {
    cursor += 1;
  }
  if (cursor === 0 || trimmedLine.charAt(cursor) !== "@") return null;

  const username = trimmedLine.slice(0, cursor);
  cursor += 1;
  const hostnameStart = cursor;
  while (cursor < trimmedLine.length && isNetworkOsIdentityCharacter(trimmedLine.charAt(cursor))) {
    cursor += 1;
  }
  if (cursor === hostnameStart) return null;

  const hostname = trimmedLine.slice(hostnameStart, cursor);
  const terminator = trimmedLine.charAt(cursor);
  if (terminator !== ">" && terminator !== "#") return null;

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

export function parseJuniperJunosContextLine(line: string): TerminalParsedContext | null {
  return parseVyosContextLine(line);
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

export const JUNIPER_JUNOS_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "juniper_junos",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseJuniperJunosPrompt,
  parseContextLine: parseJuniperJunosContextLine,
  isErrorLine: (line) => {
    const normalizedLine = line.trimStart().toLowerCase();
    return (
      normalizedLine === "syntax error" ||
      JUNIPER_JUNOS_ERROR_PREFIXES.some((prefix) => normalizedLine.startsWith(prefix))
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
  isWarningLine: (line) => line.startsWith("<WARNING>"),
};

export const ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "allied_telesis_awplus",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseAlliedTelesisAwplusPrompt,
  isErrorLine: (line) => {
    const message = parseAlliedTelesisAwplusMessage(line);
    return ALLIED_TELESIS_AWPLUS_ERROR_PATTERNS.some((pattern) => pattern(message));
  },
};

export const FURUKAWA_FITELNET_DECORATION_PROFILE: TerminalDecorationProfile = {
  mode: "furukawa_fitelnet",
  decorationLookback: 80,
  decorationStyle: "text-only-v1",
  pinnedCommand: true,
  parsePrompt: parseFurukawaFitelnetPrompt,
  isErrorLine: isFurukawaFitelnetErrorLine,
  isWarningLine: (line) => {
    const trimmedLine = line.trimEnd();
    return (
      hasNonEmptyMarkerPayload(trimmedLine, "WARNING:") ||
      hasNonEmptyMarkerPayload(trimmedLine, "<WARNING>")
    );
  },
};

const TERMINAL_DECORATION_PROFILES = new Map<TerminalMode, TerminalDecorationProfile>([
  ["cisco_ios", CISCO_IOS_DECORATION_PROFILE],
  ["arista_eos", ARISTA_EOS_DECORATION_PROFILE],
  ["juniper_junos", JUNIPER_JUNOS_DECORATION_PROFILE],
  ["vyos", VYOS_DECORATION_PROFILE],
  ["fujitsu_sir", FUJITSU_SIR_DECORATION_PROFILE],
  ["allied_telesis_awplus", ALLIED_TELESIS_AWPLUS_DECORATION_PROFILE],
  ["furukawa_fitelnet", FURUKAWA_FITELNET_DECORATION_PROFILE],
]);

export function getTerminalDecorationProfile(
  terminalMode: TerminalMode
): TerminalDecorationProfile | null {
  return TERMINAL_DECORATION_PROFILES.get(terminalMode) ?? null;
}
