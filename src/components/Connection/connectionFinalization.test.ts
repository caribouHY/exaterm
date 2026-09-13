import { describe, expect, it, vi } from "vitest";
import { finalizeConnectionSession } from "./connectionFinalization";

describe("finalizeConnectionSession", () => {
  it("retries registration for the same session and records history once after success", async () => {
    const registerSession = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new Error("registration failed"))
      .mockResolvedValueOnce();
    const recordHistory = vi.fn();

    await expect(finalizeConnectionSession({ registerSession, recordHistory })).rejects.toThrow(
      "registration failed"
    );
    expect(recordHistory).not.toHaveBeenCalled();

    await finalizeConnectionSession({ registerSession, recordHistory });

    expect(registerSession).toHaveBeenCalledTimes(2);
    expect(recordHistory).toHaveBeenCalledTimes(1);
  });
});
