import { describe, expect, it } from "vitest";
import type { ConnectionHistoryEntry, SavedConnection } from "../../types";
import {
  filterConnectionSearchSections,
  historySearchSection,
  moveConnectionSearchSelection,
  profileSearchSections,
  resolveConnectionSearchSelection,
  type ConnectionSearchSection,
} from "./connectionSearchModel";

const profiles: SavedConnection[] = [
  {
    id: "東京-router",
    connection_type: "ssh",
    host: "ROUTER.Example",
    group: "本社",
    memo: "Maintenance window",
    private_key_path: "private-key-not-searchable",
  },
  { id: "edge", connection_type: "ssh", host: "2001:db8::1", port: 2222, group: "Lab" },
  { id: "switch", connection_type: "telnet", host: "switch.example" },
];
const history: ConnectionHistoryEntry = {
  id: "東京-router",
  connection_info: {
    kind: "ssh",
    host: "old.example",
    port: 2200,
    username: "operator",
    auth_method: "auto",
    private_key_path: "history-private-key",
  },
  encoding: "utf-8",
  terminal_mode: "general",
  last_connected_at: "2026-10-01T00:00:00Z",
};
const sections: ConnectionSearchSection[] = [
  { key: "manual", options: [{ value: "", label: "Manual", searchText: "", alwaysVisible: true }] },
  historySearchSection([history], () => "Recent device", "Recent"),
  ...profileSearchSections(profiles, (profile) => profile.id, "Saved", "Ungrouped"),
];
const results = (query: string) =>
  filterConnectionSearchSections(sections, query).flatMap((section) => section.options);
const values = (query: string) => results(query).map((option) => option.value);

describe("connection profile search", () => {
  it("matches name, host, group, memo, and port without case or surrounding whitespace sensitivity", () => {
    for (const query of ["東京", " router.example ", "本社", " MAINTENANCE ", "22"]) {
      expect(values(query)).toContain("profile:東京-router");
    }
    expect(values("lab")).toEqual(["", "profile:edge"]);
    expect(values("router.example:22")).toEqual(["", "profile:東京-router"]);
    expect(values("[2001:db8::1]:2222")).toEqual(["", "profile:edge"]);
    expect(values("Ungrouped")).toEqual(["", "profile:switch"]);
    expect(values("2001:db8")).toEqual(["", "profile:edge"]);
    expect(values("private-key-not-searchable")).toEqual([""]);
    expect(values("history-private-key")).toEqual([""]);
  });

  it("retains recent and saved categories, group order, and source IDs", () => {
    const filtered = filterConnectionSearchSections(sections, "  ");
    expect(filtered.map((section) => section.label)).toEqual([
      undefined,
      "Recent",
      "Saved / 本社",
      "Saved / Lab",
      "Saved / Ungrouped",
    ]);
    expect(values("")).toEqual([
      "",
      "history:東京-router",
      "profile:東京-router",
      "profile:edge",
      "profile:switch",
    ]);
    expect(values("Recent device")).toEqual(["", "history:東京-router"]);
    expect(values("OLD.EXAMPLE")).toEqual(["", "history:東京-router"]);
    expect(values("operator")).toEqual(["", "history:東京-router"]);
  });

  it("formats endpoints with protocol defaults and IPv6 brackets", () => {
    expect(results("").find((option) => option.value === "profile:東京-router")?.detail).toBe(
      "ROUTER.Example:22"
    );
    expect(results("").find((option) => option.value === "profile:edge")?.detail).toBe(
      "[2001:db8::1]:2222"
    );
    expect(results("").find((option) => option.value === "profile:switch")?.detail).toBe(
      "switch.example:23"
    );
  });

  it("keeps manual entry accessible without auto-selecting it on an unmatched query", () => {
    const unmatched = results("missing");
    expect(unmatched.map((option) => option.value)).toEqual([""]);
    expect(resolveConnectionSearchSelection(unmatched, "profile:edge", "missing")).toBeNull();
    expect(resolveConnectionSearchSelection(unmatched, null, "missing")).toBeNull();
    expect(resolveConnectionSearchSelection(unmatched, "", "missing")).toBe("");
    expect(moveConnectionSearchSelection(unmatched, null, "next")).toBe("");
    expect(resolveConnectionSearchSelection(unmatched, "", "")).toBe("");
  });

  it("uses only the provided protocol and jump-profile candidates", () => {
    const jumpSections = profileSearchSections(
      profiles.filter((profile) => profile.connection_type === "ssh" && profile.id !== "edge"),
      (profile) => profile.id,
      "Saved",
      "Ungrouped"
    );
    expect(filterConnectionSearchSections(jumpSections, "switch")).toEqual([]);
    expect(filterConnectionSearchSections(jumpSections, "edge")).toEqual([]);
    expect(filterConnectionSearchSections(jumpSections, "本社")).toHaveLength(1);
  });

  it("leaves profiles, group metadata, and the complete source list intact while filtering", () => {
    const before = JSON.stringify({ profiles, sections });
    results("missing");
    results("lab");
    expect(JSON.stringify({ profiles, sections })).toBe(before);
    expect(values("")).toHaveLength(5);
  });

  it("resolves valid highlights, recovers after filtering or removal, and wraps in both directions", () => {
    const options = results("");
    expect(resolveConnectionSearchSelection(options, "profile:edge", "")).toBe("profile:edge");
    expect(resolveConnectionSearchSelection(results("本社"), "profile:edge", "本社")).toBe(
      "profile:東京-router"
    );
    expect(moveConnectionSearchSelection(options, "", "previous")).toBe("profile:switch");
    expect(moveConnectionSearchSelection(options, "profile:switch", "next")).toBe("");
    expect(moveConnectionSearchSelection(options, null, "previous")).toBe("profile:switch");
    expect(moveConnectionSearchSelection([], null, "next")).toBeNull();
    expect(resolveConnectionSearchSelection([], "removed", "")).toBeNull();
  });
});
