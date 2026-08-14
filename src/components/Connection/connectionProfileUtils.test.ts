import { describe, expect, it } from "vitest";
import {
  createSshProfile,
  normalizeSshAuthMethod,
  resolveSshAuthentication,
  SSH_AUTH_METHODS,
  usesPrivateKeyAuthentication,
} from "./connectionProfileUtils";

describe("SSH authentication methods", () => {
  it("lists automatic before the explicit authentication options", () => {
    expect(SSH_AUTH_METHODS.map((entry) => entry.value)).toEqual([
      "auto",
      "password",
      "keyboard_interactive",
      "public_key",
    ]);
  });

  it("preserves supported methods, defaults missing values to automatic, and rejects unknown values safely", () => {
    expect(normalizeSshAuthMethod("auto")).toBe("auto");
    expect(normalizeSshAuthMethod("keyboard_interactive")).toBe("keyboard_interactive");
    expect(normalizeSshAuthMethod("unknown")).toBe("password");
    expect(normalizeSshAuthMethod(null)).toBe("auto");
    expect(normalizeSshAuthMethod(undefined)).toBe("auto");
    expect(normalizeSshAuthMethod("  ")).toBe("auto");
  });

  it("resolves private keys only for automatic and explicit public-key authentication", () => {
    expect(resolveSshAuthentication("auto", " connection-key ", " default-key ")).toEqual({
      authMethod: "auto",
      privateKeyPath: "connection-key",
    });
    expect(resolveSshAuthentication("auto", "", " default-key ")).toEqual({
      authMethod: "auto",
      privateKeyPath: "default-key",
    });
    expect(resolveSshAuthentication("auto", "", "")).toEqual({
      authMethod: "auto",
      privateKeyPath: "",
    });
    expect(resolveSshAuthentication("public_key", "", "default-key")).toEqual({
      authMethod: "public_key",
      privateKeyPath: "",
    });
    expect(resolveSshAuthentication("password", "connection-key", "default-key")).toEqual({
      authMethod: "password",
      privateKeyPath: "",
    });
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

  it("retains an optional private-key path for automatic profiles", () => {
    const profile = createSshProfile({
      profileName: "router",
      host: "router.example",
      port: "22",
      username: "operator",
      authMethod: "auto",
      privateKeyPath: " connection-key ",
      jumpProfileId: "",
      encoding: "utf-8",
      terminalMode: "general",
      memo: "",
      externalControlEnabled: true,
    });

    expect(profile.auth_method).toBe("auto");
    expect(profile.private_key_path).toBe("connection-key");
  });
});
