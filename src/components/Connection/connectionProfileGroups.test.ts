import { describe, expect, it } from "vitest";
import type { SavedConnection } from "../../types";
import { groupSavedProfiles, normalizeProfileGroup } from "./connectionProfileGroups";
import { createSshProfile, createTelnetProfile } from "./connectionProfileUtils";

describe("saved profile groups", () => {
  it("keeps named groups in first-seen order and puts legacy and blank groups last", () => {
    const profiles: SavedConnection[] = [
      { id: "legacy", connection_type: "ssh" },
      { id: "router", connection_type: "ssh", group: " 拠点A " },
      { id: "switch", connection_type: "telnet", group: "Lab" },
      { id: "router2", connection_type: "ssh", group: "拠点A" },
      { id: "blank", connection_type: "telnet", group: "  " },
      { id: "null", connection_type: "ssh", group: null },
      { id: "special", connection_type: "ssh", group: "__proto__" },
    ];
    expect(groupSavedProfiles(profiles)).toEqual([
      { name: "拠点A", profiles: [profiles[1], profiles[3]] },
      { name: "Lab", profiles: [profiles[2]] },
      { name: "__proto__", profiles: [profiles[6]] },
      { name: null, profiles: [profiles[0], profiles[4], profiles[5]] },
    ]);
    expect(profiles[1].group).toBe(" 拠点A ");
    expect(groupSavedProfiles([])).toEqual([]);
  });

  it("normalizes optional group values without changing case or internal spaces", () => {
    expect(normalizeProfileGroup(undefined)).toBeNull();
    expect(normalizeProfileGroup(null)).toBeNull();
    expect(normalizeProfileGroup(" \t ")).toBeNull();
    expect(normalizeProfileGroup(" Lab A ")).toBe("Lab A");
  });

  it("saves group metadata separately from SSH identity and jump references", () => {
    const profile = createSshProfile({
      profileName: "router",
      group: " 拠点A ",
      host: "router.example",
      port: "22",
      username: "operator",
      authMethod: "auto",
      privateKeyPath: "key",
      jumpProfileId: "bastion",
      encoding: "utf-8",
      terminalMode: "general",
      memo: "memo",
      externalControlEnabled: false,
    });
    expect(profile).toMatchObject({
      id: "router",
      group: "拠点A",
      jump_profile_id: "bastion",
      host: "router.example",
      external_control_enabled: false,
    });
  });

  it("supports assigning and clearing Telnet groups", () => {
    const draft = {
      profileName: "switch",
      group: " Lab ",
      host: "switch.example",
      port: "23",
      encoding: "utf-8" as const,
      terminalMode: "general" as const,
      memo: "",
      externalControlEnabled: true,
    };
    expect(createTelnetProfile(draft)).toMatchObject({ id: "switch", group: "Lab" });
    expect(createTelnetProfile({ ...draft, group: "  " }).group).toBeNull();
  });
});
