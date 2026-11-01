import { describe, expect, it } from "vitest";
import type { SavedConnection } from "../../types";
import { createProfileCopy } from "./connectionProfileCopy";
import { createTelnetProfile } from "./connectionProfileUtils";

const source: SavedConnection = {
  id: "router",
  connection_type: "ssh",
  host: "router.example",
  port: 2222,
  username: "operator",
  auth_method: "public_key",
  private_key_path: "private-key-path",
  jump_profile_id: "bastion",
  encoding: "shift-jis",
  terminal_mode: "cisco_ios",
  group: "Site A",
  memo: "Maintenance",
  external_control_enabled: false,
};

describe("saved profile copies", () => {
  it("preserves supported SSH settings in a separate draft without copying unknown credentials", () => {
    const stored = {
      ...source,
      password: "temporary-password",
      passphrase: "temporary-passphrase",
    };
    const before = JSON.stringify(stored);
    const copy = createProfileCopy(stored, [stored], "router のコピー");
    expect(copy).toEqual({ ...source, id: "router のコピー" });
    expect(copy).not.toBe(stored);
    expect(copy).not.toHaveProperty("password");
    expect(copy).not.toHaveProperty("passphrase");
    expect(JSON.stringify(stored)).toBe(before);
  });

  it("reserves names across groups within the same protocol", () => {
    const profiles = [
      source,
      { ...source, id: "router copy", group: "Other" },
      { ...source, id: "router copy (2)" },
    ];
    expect(createProfileCopy(source, profiles, "router copy").id).toBe("router copy (3)");
    expect(profiles).toHaveLength(3);
  });

  it("allows a name used by a different protocol and excludes SSH-only fields from Telnet", () => {
    const telnet: SavedConnection = { ...source, connection_type: "telnet", port: 23 };
    const copy = createProfileCopy(
      telnet,
      [source, telnet, { ...source, id: "router copy" }],
      "router copy"
    );
    expect(copy).toMatchObject({
      id: "router copy",
      connection_type: "telnet",
      group: "Site A",
      port: 23,
      external_control_enabled: false,
    });
    expect(copy).not.toHaveProperty("jump_profile_id");
    expect(copy).not.toHaveProperty("private_key_path");
    expect(copy).not.toHaveProperty("username");
  });

  it("does not generate an SSH name equal to its jump reference", () => {
    const copy = createProfileCopy(
      { ...source, jump_profile_id: "router copy" },
      [source],
      "router copy"
    );
    expect(copy.id).toBe("router copy (2)");
    expect(copy.jump_profile_id).toBe("router copy");
  });

  it("checks the normalized name that will actually be saved for legacy whitespace-prefixed IDs", () => {
    const legacy: SavedConnection = {
      id: " router",
      connection_type: "telnet",
      host: "router.example",
      port: 23,
    };
    const existing: SavedConnection = { ...legacy, id: "router のコピー" };
    const copy = createProfileCopy(legacy, [legacy, existing], " router のコピー ");
    const saved = createTelnetProfile({
      profileName: copy.id,
      group: copy.group ?? "",
      host: copy.host ?? "",
      port: String(copy.port),
      encoding: "utf-8",
      terminalMode: "general",
      memo: copy.memo ?? "",
      externalControlEnabled: true,
    });
    expect(copy.id).toBe("router のコピー (2)");
    expect(saved.id).toBe(copy.id);
  });
});
