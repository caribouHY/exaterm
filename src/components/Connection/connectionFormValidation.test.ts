import { describe, expect, it } from "vitest";
import {
  isActiveConnectionFormValid,
  parseConnectionPort,
  validateSerialConnectionForm,
  validateSshConnectionForm,
  validateTelnetConnectionForm,
  type ConnectionFormValidationState,
} from "./connectionFormValidation";

const validSshInput = {
  host: "router.example.com",
  port: "22",
  username: "admin",
  authMethod: "auto" as const,
  privateKeyPath: "",
};

describe("connection form validation", () => {
  it.each(["", "0", "65536", "22.5", "1e2", "abc"])(
    "rejects an invalid connection port: %s",
    (port) => {
      expect(parseConnectionPort(port)).toBeNull();
    }
  );

  it.each(["1", "22", "65535", " 23 "])("accepts a valid connection port: %s", (port) => {
    expect(parseConnectionPort(port)).not.toBeNull();
  });

  it("requires SSH host, port, and username", () => {
    const validation = validateSshConnectionForm({
      ...validSshInput,
      host: " ",
      port: "",
      username: "",
    });

    expect(validation).toEqual({
      isValid: false,
      errors: { host: "required", port: "port_range", username: "required" },
    });
  });

  it("requires a private key only for explicit public-key authentication", () => {
    expect(
      validateSshConnectionForm({
        ...validSshInput,
        authMethod: "public_key",
      }).errors.privateKeyPath
    ).toBe("required");

    for (const authMethod of ["auto", "password", "keyboard_interactive"] as const) {
      expect(validateSshConnectionForm({ ...validSshInput, authMethod }).isValid).toBe(true);
    }
  });

  it("accepts a complete SSH form", () => {
    expect(validateSshConnectionForm(validSshInput)).toEqual({ isValid: true, errors: {} });
  });

  it("requires a Telnet host and a valid port", () => {
    expect(validateTelnetConnectionForm({ host: " ", port: "65536" })).toEqual({
      isValid: false,
      errors: { host: "required", port: "port_range" },
    });
    expect(validateTelnetConnectionForm({ host: "router.example.com", port: "23" }).isValid).toBe(
      true
    );
  });

  it("requires a selected Serial port", () => {
    expect(validateSerialConnectionForm({ selectedPort: "" })).toEqual({
      isValid: false,
      errors: { selectedPort: "serial_port_required" },
    });
    expect(validateSerialConnectionForm({ selectedPort: "COM3" }).isValid).toBe(true);
  });

  it("uses only the active protocol when deciding whether connection is allowed", () => {
    const validation: ConnectionFormValidationState = {
      ssh: { isValid: true, errors: {} },
      telnet: { isValid: false, errors: { host: "required" } },
      serial: { isValid: false, errors: { selectedPort: "serial_port_required" } },
    };

    expect(isActiveConnectionFormValid("ssh", validation)).toBe(true);
    expect(isActiveConnectionFormValid("telnet", validation)).toBe(false);
    expect(isActiveConnectionFormValid("serial", validation)).toBe(false);
  });
});
