import { describe, expect, it, vi } from "vitest";
import { connectionLogTarget, startConnectionLog } from "./connectionLogModel";

describe("startConnectionLog", () => {
  it("does not start a log when the connection setting is disabled", async () => {
    const start = vi.fn<() => Promise<string>>();

    await expect(startConnectionLog(false, start)).resolves.toEqual({
      isLogging: false,
      filePath: null,
      startFailed: false,
    });
    expect(start).not.toHaveBeenCalled();
  });

  it("returns the active log path after a successful start", async () => {
    const start = vi.fn().mockResolvedValue("C:\\logs\\session.log");

    await expect(startConnectionLog(true, start)).resolves.toEqual({
      isLogging: true,
      filePath: "C:\\logs\\session.log",
      startFailed: false,
    });
  });

  it("reports a log failure without rejecting the connection flow", async () => {
    const start = vi.fn().mockRejectedValue(new Error("log unavailable"));

    await expect(startConnectionLog(true, start)).resolves.toEqual({
      isLogging: false,
      filePath: null,
      startFailed: true,
    });
  });
});

describe("connectionLogTarget", () => {
  it("uses actual SSH and Telnet endpoints when a tab title is a profile name", () => {
    expect(
      connectionLogTarget({
        title: "office-router",
        connectionInfo: {
          kind: "ssh",
          host: "edited.invalid",
          port: 2222,
          username: "admin",
          auth_method: "password",
        },
      })
    ).toBe("admin@edited.invalid:2222");
    expect(
      connectionLogTarget({
        title: "office-router",
        connectionInfo: {
          kind: "telnet",
          host: "router.invalid",
          port: 2323,
        },
      })
    ).toBe("router.invalid:2323");
    expect(connectionLogTarget({ title: "COM3" })).toBe("COM3");
  });
});
