import { describe, expect, it, vi } from "vitest";
import type { StartupCliRequest } from "../../types";
import { createStartupCliRequestCoordinator } from "./startupCliRequestCoordinator";

const request = (target: string): StartupCliRequest => ({
  kind: "telnet",
  target,
  port: null,
});

const flushPromises = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("startup CLI request coordinator", () => {
  it("serializes overlapping availability checks", async () => {
    let resolveTake: ((value: StartupCliRequest | null) => void) | undefined;
    const takeNext = vi.fn(
      () =>
        new Promise<StartupCliRequest | null>((resolve) => {
          resolveTake = resolve;
        })
    );
    const onRequest = vi.fn();
    const coordinator = createStartupCliRequestCoordinator({
      takeNext,
      onRequest,
      onError: vi.fn(),
    });

    coordinator.check();
    coordinator.check();
    expect(takeNext).toHaveBeenCalledTimes(1);

    resolveTake?.(null);
    await flushPromises();
    expect(takeNext).toHaveBeenCalledTimes(2);
  });

  it("waits for the active dialog before taking the next request", async () => {
    const takeNext = vi
      .fn<() => Promise<StartupCliRequest | null>>()
      .mockResolvedValueOnce(request("first"))
      .mockResolvedValueOnce(request("second"))
      .mockResolvedValue(null);
    const onRequest = vi.fn();
    const coordinator = createStartupCliRequestCoordinator({
      takeNext,
      onRequest,
      onError: vi.fn(),
    });

    coordinator.check();
    await flushPromises();
    expect(onRequest).toHaveBeenLastCalledWith(request("first"));

    coordinator.check();
    await flushPromises();
    expect(takeNext).toHaveBeenCalledTimes(1);

    coordinator.setBlocked(false);
    await flushPromises();
    expect(onRequest).toHaveBeenLastCalledWith(request("second"));
    expect(takeNext).toHaveBeenCalledTimes(2);
  });

  it("reports take failures and remains retryable", async () => {
    const error = new Error("unavailable");
    const takeNext = vi
      .fn<() => Promise<StartupCliRequest | null>>()
      .mockRejectedValueOnce(error)
      .mockResolvedValueOnce(request("retry"));
    const onRequest = vi.fn();
    const onError = vi.fn();
    const coordinator = createStartupCliRequestCoordinator({ takeNext, onRequest, onError });

    coordinator.check();
    await flushPromises();
    expect(onError).toHaveBeenCalledWith(error);

    coordinator.check();
    await flushPromises();
    expect(onRequest).toHaveBeenCalledWith(request("retry"));
  });
});
