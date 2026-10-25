import type { ConnectionHistoryEntry, SavedConnection } from "../../types";
import { groupSavedProfiles } from "./connectionProfileGroups";

export interface ConnectionSearchOption {
  value: string;
  label: string;
  detail?: string;
  searchText: string;
  alwaysVisible?: boolean;
}

export interface ConnectionSearchSection {
  key: string;
  label?: string;
  options: ConnectionSearchOption[];
}

export function profileSearchSections(
  profiles: SavedConnection[],
  getDisplayName: (profile: SavedConnection) => string,
  savedLabel: string,
  ungroupedLabel: string
): ConnectionSearchSection[] {
  return groupSavedProfiles(profiles).map(({ name, profiles: entries }) => ({
    key: `group:${JSON.stringify(name)}`,
    label: `${savedLabel} / ${name ?? ungroupedLabel}`,
    options: entries.map((profile) => {
      const host = profile.host ?? "";
      const displayHost = host.includes(":") && !host.startsWith("[") ? `[${host}]` : host;
      const port = profile.port ?? (profile.connection_type === "telnet" ? 23 : 22);
      const label = getDisplayName(profile);
      const detail = host ? `${displayHost}:${port}` : undefined;
      return {
        value: `profile:${profile.id}`,
        label,
        detail,
        searchText: [
          label,
          profile.id,
          host,
          port,
          detail,
          name ?? ungroupedLabel,
          profile.memo,
        ].join(" "),
      };
    }),
  }));
}

export function historySearchSection(
  entries: ConnectionHistoryEntry[],
  getDisplayName: (entry: ConnectionHistoryEntry) => string,
  label: string
): ConnectionSearchSection {
  return {
    key: "history",
    label,
    options: entries.map((entry) => {
      const displayName = getDisplayName(entry);
      const info = entry.connection_info;
      return {
        value: `history:${entry.id}`,
        label: displayName,
        searchText: [
          displayName,
          info.host,
          info.port,
          info.kind === "ssh" ? info.username : "",
        ].join(" "),
      };
    }),
  };
}

export function filterConnectionSearchSections(
  sections: ConnectionSearchSection[],
  query: string
): ConnectionSearchSection[] {
  const normalized = query.trim().toLowerCase();
  return sections
    .map((section) => ({
      ...section,
      options: section.options.filter(
        (option) => option.alwaysVisible || option.searchText.toLowerCase().includes(normalized)
      ),
    }))
    .filter((section) => section.options.length > 0);
}

export function resolveConnectionSearchSelection(
  options: ConnectionSearchOption[],
  preferred: string | null,
  query: string
): string | null {
  const preferredOption = options.find((option) => option.value === preferred);
  if (preferredOption) return preferredOption.value;
  const firstMatch = options.find((option) => !option.alwaysVisible);
  const fallback = options.find((option) => option.alwaysVisible);
  return firstMatch?.value ?? (query.trim() ? null : (fallback?.value ?? null));
}

export function moveConnectionSearchSelection(
  options: ConnectionSearchOption[],
  current: string | null,
  direction: "next" | "previous"
): string | null {
  if (options.length === 0) return null;
  const index = options.findIndex((option) => option.value === current);
  const offset = direction === "next" ? 1 : -1;
  const nextIndex =
    index < 0
      ? offset === 1
        ? 0
        : options.length - 1
      : (index + offset + options.length) % options.length;
  return options.find((_option, optionIndex) => optionIndex === nextIndex)?.value ?? null;
}
