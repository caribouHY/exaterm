import { describe, expect, it } from "vitest";
import {
  createSshProfile,
  normalizeSshAuthMethod,
  SSH_AUTH_METHODS,
  usesPrivateKeyAuthentication,
} from "./connectionProfileUtils";

describe("SSH authentication methods", () => {
  it("lists explicit password, keyboard-interactive, and public-key options", () => {
    expect(SSH_AUTH_METHODS.map((entry) => entry.value)).toEqual([
      "password",
      "keyboard_interactive",
      "public_key",
    ]);
  });

  it("preserves keyboard-interactive and defaults unknown values to password", () => {
    expect(normalizeSshAuthMethod("keyboard_interactive")).toBe("keyboard_interactive");
    expect(normalizeSshAuthMethod("unknown")).toBe("password");
    expect(normalizeSshAuthMethod(null)).toBe("password");
  });

  it("does not retain a private-key path for keyboard-interactive profiles", () => {
    expect(usesPrivateKeyAuthentication("keyboard_interactive")).toBe(false);
    expect(usesPrivateKeyAuthentication("public_key")).toBe(true);

    const profile = createSshProfile({
      profileName: "router",
      host: "router.example",
      port: "22",
      username: "operator",
      authMethod: "keyboard_interactive",
      privateKeyPath: "unused-key",
      jumpProfileId: "",
      encoding: "utf-8",
      terminalMode: "general",
      memo: "",
      externalControlEnabled: true,
    });

    expect(profile.auth_method).toBe("keyboard_interactive");
    expect(profile.private_key_path).toBeNull();
  });
});
