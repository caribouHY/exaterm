import { describe, expect, it, vi } from "vitest";
import {
  createSshDiagnosticSubscription,
  type DiagnosticEventListener,
} from "./sshDiagnosticSubscription";

function harness() {
  const pending: Array<{
    event: string;
    emit: (payload: unknown) => void;
    resolve: (unlisten: () => void) => void;
    reject: (error: unknown) => void;
  }> = [];
  const listen: DiagnosticEventListener = (event, handler) =>
    new Promise((resolve, reject) => {
      const receive = handler as (event: { payload: unknown }) => void;
      pending.push({ event, emit: (payload) => receive({ payload }), resolve, reject });
    });
  const diagnostic = vi.fn();
  const progress = vi.fn();
  const subscription = createSshDiagnosticSubscription(listen, diagnostic, progress);
  return { pending, diagnostic, progress, subscription };
}

const diagnostic = { level: "info", message: "Test diagnostic" };
const progress = { phase: "authenticating", target: "target" };
const tick = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

describe("SSH diagnostic subscription", () => {
  it("delivers current events and ignores queued callbacks after stop", async () => {
    const h = harness();
    const start = h.subscription.start("first");
    const stopDiagnostic = vi.fn();
    h.pending[0].resolve(stopDiagnostic);
    await tick();
    const stopProgress = vi.fn();
    h.pending[1].resolve(stopProgress);
    await start;
    expect(h.pending.map((item) => item.event)).toEqual([
      "ssh://connect-diagnostic/first",
      "ssh://connect-progress/first",
    ]);
    h.pending[0].emit(diagnostic);
    h.pending[1].emit(progress);
    expect(h.diagnostic).toHaveBeenCalledWith(diagnostic);
    expect(h.progress).toHaveBeenCalledWith("first", progress);
    h.subscription.stop();
    h.subscription.stop();
    h.pending[0].emit(diagnostic);
    h.pending[1].emit(progress);
    expect(h.diagnostic).toHaveBeenCalledTimes(1);
    expect(h.progress).toHaveBeenCalledTimes(1);
    expect(stopDiagnostic).toHaveBeenCalledTimes(1);
    expect(stopProgress).toHaveBeenCalledTimes(1);
  });

  it("releases a listener which finishes registering after cancellation", async () => {
    const h = harness();
    const start = h.subscription.start("cancelled");
    h.subscription.stop();
    h.pending[0].emit(diagnostic);
    const unlisten = vi.fn();
    h.pending[0].resolve(unlisten);
    await start;
    expect(unlisten).toHaveBeenCalledOnce();
    expect(h.pending).toHaveLength(1);
    expect(h.diagnostic).not.toHaveBeenCalled();
  });

  it("releases the first listener while progress registration is still pending", async () => {
    const h = harness();
    const start = h.subscription.start("cancelled");
    const stopDiagnostic = vi.fn();
    h.pending[0].resolve(stopDiagnostic);
    await tick();
    h.subscription.stop();
    expect(stopDiagnostic).toHaveBeenCalledOnce();
    const stopProgress = vi.fn();
    h.pending[1].resolve(stopProgress);
    await start;
    expect(stopProgress).toHaveBeenCalledOnce();
    h.pending[1].emit(progress);
    expect(h.progress).not.toHaveBeenCalled();
  });

  it("does not start the next subscription after cancellation between registration steps", async () => {
    const h = harness();
    const start = h.subscription.start("cancelled");
    const unlisten = vi.fn();
    h.pending[0].resolve(unlisten);
    queueMicrotask(h.subscription.stop);
    await start;
    expect(h.pending).toHaveLength(1);
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("does not let failure of an obsolete subscription stop its replacement", async () => {
    const h = harness();
    const first = h.subscription.start("old");
    const stopOld = vi.fn();
    h.pending[0].resolve(stopOld);
    await tick();
    const second = h.subscription.start("new");
    const stopNew = vi.fn();
    h.pending[2].resolve(stopNew);
    await tick();
    h.pending[3].resolve(vi.fn());
    await second;
    const failed = expect(first).rejects.toThrow("subscription failed");
    h.pending[1].reject(new Error("subscription failed"));
    await failed;
    h.pending[0].emit(diagnostic);
    h.pending[1].emit(progress);
    h.pending[2].emit(diagnostic);
    h.pending[3].emit(progress);
    expect(stopOld).toHaveBeenCalledOnce();
    expect(stopNew).not.toHaveBeenCalled();
    expect(h.diagnostic).toHaveBeenCalledTimes(1);
    expect(h.progress).toHaveBeenCalledExactlyOnceWith("new", progress);
    h.subscription.stop();
  });

  it("cleans up partial registration on failure and permits a new subscription", async () => {
    const h = harness();
    const first = h.subscription.start("first");
    const unlisten = vi.fn();
    h.pending[0].resolve(unlisten);
    await tick();
    const failed = expect(first).rejects.toThrow("subscription failed");
    h.pending[1].reject(new Error("subscription failed"));
    await failed;
    expect(unlisten).toHaveBeenCalledOnce();
    h.pending[0].emit(diagnostic);
    expect(h.diagnostic).not.toHaveBeenCalled();
    const second = h.subscription.start("next");
    h.pending[2].resolve(vi.fn());
    await tick();
    h.pending[3].resolve(vi.fn());
    await second;
    h.pending[3].emit(progress);
    expect(h.progress).toHaveBeenCalledExactlyOnceWith("next", progress);
    h.subscription.stop();
  });
});
