import { describe, expect, it, vi } from "vitest";
import {
  createConnectionAttemptController,
  type ConnectionPlan,
  type PreparationContext,
} from "./connectionAttemptController";
import { isConnectionCancellation } from "./connectionProtocolEvents";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
const tick = async () => {
  for (let i = 0; i < 12; i++) await Promise.resolve();
};
const log = { isLogging: true, filePath: "test-log", startFailed: false };
const prompt = {
  phase: "target" as const,
  host: "example.invalid",
  port: 22,
  username: "test",
  authMethod: "public_key" as const,
  privateKeyPath: "test-key",
};
function setup(prepare?: (context: PreparationContext) => Promise<void>) {
  const connection = deferred<string>();
  const cancellation = deferred<boolean>();
  const plan: ConnectionPlan = {
    type: "ssh",
    connect: vi.fn(() => connection.promise),
    startLog: vi.fn(async () => log),
    register: vi.fn(async () => {}),
    recordHistory: vi.fn(async () => {}),
    clearCredentials: vi.fn(),
  };
  const deps = {
    createRequestId: vi.fn(() => `request-${ids++}`),
    prepare: vi.fn(async (_input: { target: string }, context: PreparationContext) => {
      await prepare?.(context);
      return plan;
    }),
    cancel: vi.fn(() => cancellation.promise),
    disconnect: vi.fn(async () => {}),
    stopDiagnostics: vi.fn(),
    errorMessage: () => "safe error",
    isCancellation: isConnectionCancellation,
  };
  let ids = 1;
  const controller = createConnectionAttemptController(deps);
  const start = () => controller.start("ssh", { target: "example.invalid" });
  return { controller, deps, plan, connection, cancellation, start };
}

describe("connection attempt controller", () => {
  it("synchronously owns a single attempt across start clicks and publishes snapshots", async () => {
    const h = setup();
    const listener = vi.fn();
    h.controller.subscribe(listener);
    const pending = h.start();
    void h.start();
    expect(h.controller.getSnapshot().status).toBe("preparing");
    await tick();
    expect(h.plan.connect).toHaveBeenCalledTimes(1);
    h.connection.resolve("session");
    await pending;
    expect(h.controller.getSnapshot().status).toBe("complete");
    expect(listener).toHaveBeenCalled();
  });
  it("copies connection input before any asynchronous preparation", async () => {
    const gate = deferred<void>();
    let target = "";
    const h = setup();
    h.deps.prepare.mockImplementation(async (input) => {
      await gate.promise;
      target = input.target;
      return h.plan;
    });
    const input = { target: "original" };
    const pending = h.controller.start("ssh", input);
    input.target = "changed";
    gate.resolve();
    await tick();
    expect(target).toBe("original");
    h.connection.resolve("session");
    await pending;
  });
  it("consumes a credential prompt synchronously and rejects duplicate submit and new start", async () => {
    let secret = "";
    const h = setup(async (context) => {
      secret = await context.credential(prompt);
    });
    const pending = h.start();
    await tick();
    const p = h.controller.getSnapshot().prompt!;
    void h.start();
    h.controller.submitCredential(p.requestId, p.promptId, "secret");
    h.controller.submitCredential(p.requestId, p.promptId, "duplicate");
    expect(h.controller.getSnapshot().prompt).toBeNull();
    expect(JSON.stringify(h.controller.getSnapshot())).not.toContain("secret");
    await tick();
    expect(secret).toBe("secret");
    expect(h.plan.connect).toHaveBeenCalledTimes(1);
    h.connection.resolve("session");
    await pending;
    expect(h.plan.register).toHaveBeenCalledTimes(1);
  });
  it("rejects the old jump prompt while accepting the next target prompt", async () => {
    const values: string[] = [];
    const h = setup(async (c) => {
      values.push(await c.credential({ ...prompt, phase: "jump" }));
      values.push(await c.credential(prompt));
    });
    const pending = h.start();
    await tick();
    const first = h.controller.getSnapshot().prompt!;
    h.controller.submitCredential(first.requestId, first.promptId, "jump");
    await tick();
    const second = h.controller.getSnapshot().prompt!;
    expect(second.promptId).not.toBe(first.promptId);
    h.controller.submitCredential(first.requestId, first.promptId, "stale");
    await tick();
    expect(h.plan.connect).not.toHaveBeenCalled();
    h.controller.submitCredential(second.requestId, second.promptId, "target");
    await tick();
    h.connection.resolve("session");
    await pending;
    expect(values).toEqual(["jump", "target"]);
  });
  it("cancels credential waiting, rejects stale submission, and permits a fresh attempt", async () => {
    const h = setup(async (c) => {
      await c.credential(prompt);
    });
    const firstRun = h.start();
    await tick();
    const old = h.controller.getSnapshot().prompt!;
    await h.controller.cancel();
    await firstRun;
    const pending = h.start();
    await tick();
    const current = h.controller.getSnapshot().prompt!;
    h.controller.submitCredential(old.requestId, old.promptId, "stale");
    expect(h.controller.getSnapshot().prompt).toEqual(current);
    h.controller.submitCredential(current.requestId, current.promptId, "new");
    await tick();
    h.connection.resolve("session");
    await pending;
    expect(h.plan.connect).toHaveBeenCalledTimes(1);
  });
  it("cancels immediately after submit without invoking the connection", async () => {
    const h = setup(async (c) => {
      await c.credential(prompt);
      c.assertActive();
    });
    const pending = h.start();
    await tick();
    const p = h.controller.getSnapshot().prompt!;
    h.controller.submitCredential(p.requestId, p.promptId, "secret");
    await h.controller.cancel();
    await pending;
    expect(h.plan.connect).not.toHaveBeenCalled();
    expect(h.controller.getSnapshot().status).toBe("editing");
  });
  it("cancels pending preparation without a backend cancellation and releases its eventual plan", async () => {
    const gate = deferred<void>();
    const h = setup(async () => gate.promise);
    const pending = h.start();
    await h.controller.cancel();
    gate.resolve();
    await pending;
    expect(h.deps.cancel).not.toHaveBeenCalled();
    expect(h.plan.connect).not.toHaveBeenCalled();
    expect(h.plan.clearCredentials).toHaveBeenCalled();
  });
  for (const outcome of ["accepted", "rejected", "error"] as const) {
    for (const connectionFirst of [false, true]) {
      it(`preserves success when cancel is ${outcome}, connection first=${connectionFirst}`, async () => {
        const h = setup();
        const pending = h.start();
        await tick();
        const cancellation = h.controller.cancel();
        const settleCancel = () => {
          if (outcome === "error") h.cancellation.reject(new Error("cancel"));
          else h.cancellation.resolve(outcome === "accepted");
        };
        if (connectionFirst) {
          h.connection.resolve("session");
          await pending;
          settleCancel();
        } else {
          settleCancel();
          await cancellation;
          h.connection.resolve("session");
          await pending;
        }
        await cancellation;
        expect(h.controller.getSnapshot().status).toBe("complete");
        expect(h.plan.register).toHaveBeenCalledTimes(1);
      });
      it(`handles failure when cancel is ${outcome}, connection first=${connectionFirst}`, async () => {
        const h = setup();
        const pending = h.start();
        await tick();
        const cancellation = h.controller.cancel();
        const settleCancel = () => {
          if (outcome === "error") h.cancellation.reject(new Error("cancel"));
          else h.cancellation.resolve(outcome === "accepted");
        };
        if (connectionFirst) {
          h.connection.reject(new Error("connect"));
          await pending;
          settleCancel();
        } else {
          settleCancel();
          await cancellation;
          h.connection.reject(new Error("connect"));
          await pending;
        }
        await cancellation;
        expect(h.controller.getSnapshot().status).toBe("editing");
        expect(h.controller.getSnapshot().error).toBe("safe error");
        expect(h.plan.register).not.toHaveBeenCalled();
      });
    }
  }
  it("waits for the backend terminal result after accepted cancellation", async () => {
    const h = setup();
    const pending = h.start();
    await tick();
    const cancel = h.controller.cancel();
    h.cancellation.resolve(true);
    await cancel;
    expect(h.controller.getSnapshot().status).toBe("cancelling");
    void h.start();
    expect(h.deps.prepare).toHaveBeenCalledTimes(1);
    h.connection.reject({ code: "ssh.connect_cancelled", message: "cancelled" });
    await pending;
    expect(h.controller.getSnapshot().error).toBe("");
  });
  it("enters finalizing before logging and ignores late cancellation errors", async () => {
    const gate = deferred<typeof log>();
    const h = setup();
    vi.mocked(h.plan.startLog).mockReturnValue(gate.promise);
    const pending = h.start();
    await tick();
    const cancel = h.controller.cancel();
    h.connection.resolve("session");
    await tick();
    expect(h.controller.getSnapshot().status).toBe("finalizing");
    h.cancellation.reject(new Error("late"));
    await cancel;
    expect(h.controller.getSnapshot().status).toBe("finalizing");
    await h.controller.cancel();
    expect(h.deps.cancel).toHaveBeenCalledTimes(1);
    gate.resolve(log);
    await pending;
  });
  it("keeps the session on log failure", async () => {
    const h = setup();
    vi.mocked(h.plan.startLog).mockRejectedValue(new Error("log"));
    const pending = h.start();
    await tick();
    h.connection.resolve("session");
    await pending;
    expect(h.plan.register).toHaveBeenCalledWith("session", {
      isLogging: false,
      filePath: null,
      startFailed: true,
    });
    expect(h.deps.disconnect).not.toHaveBeenCalled();
  });
  it("retries registration once with the same session and log without reconnecting", async () => {
    const h = setup();
    vi.mocked(h.plan.register).mockRejectedValueOnce(new Error("register"));
    const pending = h.start();
    await tick();
    h.connection.resolve("session");
    await pending;
    expect(h.controller.getSnapshot().status).toBe("finalization_failed");
    await h.controller.cancel();
    void h.start();
    const retry = h.controller.retryRegistration();
    void h.controller.retryRegistration();
    await retry;
    expect(h.plan.connect).toHaveBeenCalledTimes(1);
    expect(h.plan.startLog).toHaveBeenCalledTimes(1);
    expect(h.plan.register).toHaveBeenCalledTimes(2);
    expect(h.plan.register).toHaveBeenLastCalledWith("session", log);
    expect(h.plan.recordHistory).toHaveBeenCalledTimes(1);
  });
  it("does not roll back a registered session on history failure", async () => {
    const h = setup();
    vi.mocked(h.plan.recordHistory).mockRejectedValue(new Error("history"));
    const pending = h.start();
    await tick();
    h.connection.resolve("session");
    await pending;
    expect(h.controller.getSnapshot().status).toBe("complete");
    expect(h.deps.disconnect).not.toHaveBeenCalled();
  });
  it("disconnects only its own unregistered session after dispose during connect", async () => {
    const h = setup();
    const pending = h.start();
    await tick();
    h.controller.dispose();
    h.connection.resolve("orphan");
    await pending;
    expect(h.deps.disconnect).toHaveBeenCalledExactlyOnceWith("ssh", "orphan");
    expect(h.plan.register).not.toHaveBeenCalled();
  });
  it("preserves a session when registration unmounts the dialog", async () => {
    const h = setup();
    vi.mocked(h.plan.register).mockImplementation(async () => {
      h.controller.dispose();
    });
    const pending = h.start();
    await tick();
    h.connection.resolve("session");
    await pending;
    expect(h.deps.disconnect).not.toHaveBeenCalled();
    expect(h.plan.recordHistory).toHaveBeenCalledTimes(1);
  });
  it("waits for pending registration before cleaning up on dispose", async () => {
    const registration = deferred<void>();
    const h = setup();
    vi.mocked(h.plan.register).mockReturnValue(registration.promise);
    const pending = h.start();
    await tick();
    h.connection.resolve("session");
    await tick();
    h.controller.dispose();
    expect(h.deps.disconnect).not.toHaveBeenCalled();
    registration.reject(new Error("register"));
    await pending;
    expect(h.deps.disconnect).toHaveBeenCalledExactlyOnceWith("ssh", "session");
  });
  it("releases a session on dispose after registration failure", async () => {
    const h = setup();
    vi.mocked(h.plan.register).mockRejectedValue(new Error("register"));
    const pending = h.start();
    await tick();
    h.connection.resolve("session");
    await pending;
    h.controller.dispose();
    await tick();
    expect(h.deps.disconnect).toHaveBeenCalledExactlyOnceWith("ssh", "session");
  });
  it("releases a session after dispose while logging", async () => {
    const gate = deferred<typeof log>();
    const h = setup();
    vi.mocked(h.plan.startLog).mockReturnValue(gate.promise);
    const pending = h.start();
    await tick();
    h.connection.resolve("session");
    await tick();
    h.controller.dispose();
    gate.resolve(log);
    await pending;
    expect(h.deps.disconnect).toHaveBeenCalledExactlyOnceWith("ssh", "session");
    expect(h.plan.register).not.toHaveBeenCalled();
  });
  it("releases credentials on dispose while prompting", async () => {
    const h = setup(async (c) => {
      await c.credential(prompt);
    });
    const pending = h.start();
    await tick();
    h.controller.dispose();
    await pending;
    expect(h.plan.connect).not.toHaveBeenCalled();
  });
});
