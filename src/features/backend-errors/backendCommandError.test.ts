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
