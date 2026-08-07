import type { ConnectionHistoryEntry, ConnectionType } from "../../types";
import type { ConnectionDialogInitialValues } from "./connectionDialogTypes";

const HISTORY_PREFIX = "history:";
const PROFILE_PREFIX = "profile:";

export type ConnectionSourceSelection =
  | { kind: "manual" }
  | { kind: "history"; id: string }
  | { kind: "profile"; id: string };

export function encodeConnectionSource(
  selectedProfileId: string,
  selectedHistoryId: string
): string {
  if (selectedHistoryId) return `${HISTORY_PREFIX}${selectedHistoryId}`;
  if (selectedProfileId) return `${PROFILE_PREFIX}${selectedProfileId}`;
  return "";
}

export function parseConnectionSource(value: string): ConnectionSourceSelection {
  if (value.startsWith(HISTORY_PREFIX)) {
    return { kind: "history", id: value.slice(HISTORY_PREFIX.length) };
  }
  if (value.startsWith(PROFILE_PREFIX)) {
    return { kind: "profile", id: value.slice(PROFILE_PREFIX.length) };
  }
  return { kind: "manual" };
}

export function historyEntriesForType(
  entries: ConnectionHistoryEntry[],
  type: Extract<ConnectionType, "ssh" | "telnet">
): ConnectionHistoryEntry[] {
  return entries.filter((entry) => entry.connection_info.kind === type);
}

export function historyEntryToInitialValues(
  entry: ConnectionHistoryEntry
): ConnectionDialogInitialValues {
  return {
    connectionInfo: entry.connection_info,
    encoding: entry.encoding,
    terminalMode: entry.terminal_mode,
  };
}

export function formatHistoryEntryLabel(entry: ConnectionHistoryEntry, locale: string): string {
  const info = entry.connection_info;
  const target =
    info.kind === "ssh"
      ? `${info.username}@${info.host}:${info.port}`
      : `${info.host}:${info.port}`;
  const date = new Date(entry.last_connected_at);
  if (Number.isNaN(date.getTime())) return target;
  return `${target} — ${new Intl.DateTimeFormat(locale, {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date)}`;
}
