import { describe, expect, it } from "vitest";
import type { ConnectionHistoryEntry } from "../../types";
import {
  encodeConnectionSource,
  formatHistoryEntryLabel,
  historyEntriesForType,
  historyEntryToInitialValues,
  parseConnectionSource,
  shouldRecordConnectionHistory,
} from "./connectionHistoryModel";

const sshEntry: ConnectionHistoryEntry = {
  id: "ssh-entry",
  connection_info: {
    kind: "ssh",
    host: "router.example",
    port: 22,
    username: "admin",
    auth_method: "public_key",
    private_key_path: "key-path",
    jump_profile_id: "bastion",
  },
  encoding: "shift-jis",
  terminal_mode: "cisco_ios",
  last_connected_at: "2026-08-01T00:00:00Z",
};

const telnetEntry: ConnectionHistoryEntry = {
  id: "telnet-entry",
  connection_info: { kind: "telnet", host: "switch.example", port: 23 },
  encoding: "utf-8",
  terminal_mode: "general",
  last_connected_at: "invalid-date",
};

describe("connection history model", () => {
  it("encodes and parses each connection source", () => {
    expect(parseConnectionSource(encodeConnectionSource("profile:name", ""))).toEqual({
      kind: "profile",
      id: "profile:name",
    });
    expect(parseConnectionSource(encodeConnectionSource("", "history:id"))).toEqual({
      kind: "history",
      id: "history:id",
    });
    expect(parseConnectionSource(encodeConnectionSource("", ""))).toEqual({ kind: "manual" });
  });

  it("filters history by SSH and Telnet without a Serial history type", () => {
    expect(historyEntriesForType([sshEntry, telnetEntry], "ssh")).toEqual([sshEntry]);
    expect(historyEntriesForType([sshEntry, telnetEntry], "telnet")).toEqual([telnetEntry]);
  });

  it("records manual and history-based connections but skips saved profiles", () => {
    expect(shouldRecordConnectionHistory("")).toBe(true);
    expect(shouldRecordConnectionHistory("ssh-profile")).toBe(false);
    expect(shouldRecordConnectionHistory("telnet-profile")).toBe(false);
  });

  it("converts a history entry to dialog initial values", () => {
    expect(historyEntryToInitialValues(sshEntry)).toEqual({
      connectionInfo: sshEntry.connection_info,
      encoding: "shift-jis",
      terminalMode: "cisco_ios",
    });
  });

  it("omits an invalid timestamp without losing the target label", () => {
    expect(formatHistoryEntryLabel(telnetEntry, "en")).toBe("switch.example:23");
  });
});
