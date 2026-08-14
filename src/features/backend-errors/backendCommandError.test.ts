import { describe, expect, it, vi } from "vitest";
import { parseBackendCommandError, translateBackendCommandError } from "./backendCommandError";

describe("backendCommandError", () => {
  const error = {
    code: "telnet.connect_failed",
    params: { detail: "refused" },
    message: "Failed to connect over Telnet: refused",
  };

  it("parses object and JSON string rejection payloads", () => {
    expect(parseBackendCommandError(error)).toEqual(error);
    expect(parseBackendCommandError(JSON.stringify(error))).toEqual(error);
  });

  it("translates known codes with interpolation parameters", () => {
    const t = vi.fn(
      (_key: string, params?: Record<string, string | number | boolean>) =>
        `接続失敗: ${params?.detail}`
    );
    expect(translateBackendCommandError(error, t, "fallback")).toBe("接続失敗: refused");
  });

  it("translates SSH authentication prompt errors", () => {
    const t = vi.fn((key: string) => `translated:${key}`);
    expect(
      translateBackendCommandError(
        {
          code: "ssh.auth_prompt_timed_out",
          message: "The SSH authentication prompt timed out",
        },
        t,
        "fallback"
      )
    ).toBe("translated:backend_errors.ssh_auth_prompt_timed_out");
    expect(
      translateBackendCommandError(
        {
          code: "ssh.automatic_authentication_failed",
          message: "SSH automatic authentication failed",
        },
        t,
        "fallback"
      )
    ).toBe("translated:backend_errors.ssh_automatic_authentication_failed");
  });

  it("translates SSH host key prompt response errors", () => {
    const t = vi.fn((key: string) => `translated:${key}`);
    expect(
      translateBackendCommandError(
        {
          code: "ssh.host_key_prompt_request_not_found",
          message: "The SSH host key prompt request was not found",
        },
        t,
        "fallback"
      )
    ).toBe("translated:backend_errors.ssh_host_key_prompt_request_not_found");
  });

  it("translates Telnet and Serial connection cancellation errors", () => {
    const t = vi.fn((key: string) => `translated:${key}`);

    expect(
      translateBackendCommandError(
        { code: "telnet.connect_cancelled", message: "Telnet cancelled" },
        t,
        "fallback"
      )
    ).toBe("translated:backend_errors.telnet_connect_cancelled");
    expect(
      translateBackendCommandError(
        { code: "serial.connect_cancelled", message: "Serial cancelled" },
        t,
        "fallback"
      )
    ).toBe("translated:backend_errors.serial_connect_cancelled");
  });

  it("uses English message for an unknown future code", () => {
    const t = vi.fn();
    expect(
      translateBackendCommandError(
        { code: "future.code", message: "Future failure", params: {} },
        t,
        "fallback"
      )
    ).toBe("Future failure");
    expect(t).not.toHaveBeenCalled();
  });

  it("keeps legacy string errors readable during migration", () => {
    expect(translateBackendCommandError("Legacy failure", vi.fn(), "fallback")).toBe(
      "Legacy failure"
    );
  });
});
