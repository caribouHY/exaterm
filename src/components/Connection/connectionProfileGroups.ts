import type { SavedConnection } from "../../types";

export const normalizeProfileGroup = (group?: string | null): string | null =>
  group?.trim() || null;

export function groupSavedProfiles(profiles: SavedConnection[]) {
  const groups = new Map<string | null, SavedConnection[]>();
  for (const profile of profiles) {
    const name = normalizeProfileGroup(profile.group);
    const entries = groups.get(name) ?? [];
    entries.push(profile);
    groups.set(name, entries);
  }
  const ungrouped = groups.get(null);
  groups.delete(null);
  if (ungrouped) groups.set(null, ungrouped);
  return Array.from(groups, ([name, entries]) => ({ name, profiles: entries }));
}
