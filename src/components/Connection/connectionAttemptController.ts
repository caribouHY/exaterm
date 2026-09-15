import type { ConnectionType } from "../../types";
import type { ConnectionLogState } from "../../features/terminal-logging/connectionLogModel";
import type { SshCredentialPrompt } from "./connectionDialogTypes";
import {
  initialAttemptSnapshot,
  reduceAttempt,
  type AttemptEvent,
  type AttemptSnapshot,
} from "./connectionAttemptState";

export interface ConnectionPlan {
  type: ConnectionType;
  clearCredentials?: () => void;
  connect: (requestId: string) => Promise<string>;
  startLog: (sessionId: string) => Promise<ConnectionLogState>;
  register: (sessionId: string, log: ConnectionLogState) => Promise<void>;
  recordHistory: () => Promise<void>;
}
export interface PreparationContext {
  requestId: string;
  assertActive: () => void;
  credential: (prompt: Omit<SshCredentialPrompt, "requestId" | "promptId">) => Promise<string>;
}
export interface AttemptDependencies<Input> {
  createRequestId: () => string;
  prepare: (input: Input, context: PreparationContext) => Promise<ConnectionPlan>;
  cancel: (type: ConnectionType, requestId: string) => Promise<boolean>;
  disconnect: (type: ConnectionType, sessionId: string) => Promise<void>;
  stopDiagnostics: () => void;
  errorMessage: (error: unknown) => string;
  isCancellation: (error: unknown) => boolean;
}
const abandoned = Symbol("abandoned connection preparation");
interface AttemptRuntime {
  requestId: string;
  type: ConnectionType;
  plan?: ConnectionPlan;
  sessionId?: string;
  log?: ConnectionLogState;
  credential?: { resolve: (value: string) => void; reject: (error: unknown) => void };
}

export function createConnectionAttemptController<Input>(deps: AttemptDependencies<Input>) {
  let snapshot = initialAttemptSnapshot;
  let runtime: AttemptRuntime | undefined;
  let disposed = false;
  let promptSequence = 0;
  const listeners = new Set<() => void>();
  const transition = (event: AttemptEvent) => {
    const next = reduceAttempt(snapshot, event);
    if (next === snapshot) return;
    snapshot = Object.freeze({ ...next, prompt: next.prompt ? Object.freeze(next.prompt) : null });
    for (const listener of listeners) listener();
  };
  const active = (attempt: AttemptRuntime) => !disposed && runtime === attempt;
  const setStatus = (
    attempt: AttemptRuntime,
    status: AttemptSnapshot["status"],
    extra: { error?: string; cancelError?: string; prompt?: SshCredentialPrompt } = {}
  ) => {
    transition({ type: "transition", requestId: attempt.requestId, status, ...extra });
  };
  const release = (attempt: AttemptRuntime) => {
    attempt.credential?.reject(abandoned);
    attempt.credential = undefined;
    attempt.plan?.clearCredentials?.();
    attempt.plan = undefined;
    if (runtime === attempt) {
      runtime = undefined;
      deps.stopDiagnostics();
    }
  };
  const disconnect = async (attempt: AttemptRuntime) => {
    if (!attempt.sessionId) return;
    try {
      await deps.disconnect(attempt.type, attempt.sessionId);
    } catch {
      console.error("Failed to release an unregistered connection.");
    }
    attempt.sessionId = undefined;
  };
  const finish = (attempt: AttemptRuntime, error = "") => {
    if (!active(attempt)) return;
    setStatus(attempt, "editing", { error });
    release(attempt);
  };
  const register = async (attempt: AttemptRuntime) => {
    const { plan, sessionId, log } = attempt;
    if (!plan || !sessionId || !log) return;
    try {
      // Registration can itself unmount the dialog; ownership transfers only when it resolves.
      await plan.register(sessionId, log);
    } catch (error) {
      if (disposed) {
        await disconnect(attempt);
        release(attempt);
        return;
      }
      setStatus(attempt, "finalization_failed", { error: deps.errorMessage(error) });
      return;
    }
    attempt.sessionId = undefined;
    if (!disposed) setStatus(attempt, "complete");
    release(attempt);
    try {
      await plan.recordHistory();
    } catch {
      console.warn("Failed to save connection history.");
    }
  };
  const run = async (attempt: AttemptRuntime, input: Input) => {
    try {
      const assertActive = () => {
        if (!active(attempt)) throw abandoned;
      };
      attempt.plan = await deps.prepare(input, {
        requestId: attempt.requestId,
        assertActive,
        credential: (prompt) => {
          assertActive();
          return new Promise<string>((resolve, reject) => {
            attempt.credential = { resolve, reject };
            setStatus(attempt, "credential", {
              prompt: {
                ...prompt,
                requestId: attempt.requestId,
                promptId: `${attempt.requestId}:${++promptSequence}`,
              },
            });
          });
        },
      });
      assertActive();
      setStatus(attempt, "connecting");
      attempt.sessionId = await attempt.plan.connect(attempt.requestId);
      if (!active(attempt)) {
        await disconnect(attempt);
        release(attempt);
        return;
      }
      setStatus(attempt, "finalizing");
      deps.stopDiagnostics();
      try {
        attempt.log = await attempt.plan.startLog(attempt.sessionId);
      } catch {
        attempt.log = { isLogging: false, filePath: null, startFailed: true };
      }
      if (disposed) {
        await disconnect(attempt);
        release(attempt);
        return;
      }
      await register(attempt);
    } catch (error) {
      if (!active(attempt)) release(attempt);
      else if (error !== abandoned)
        finish(attempt, deps.isCancellation(error) ? "" : deps.errorMessage(error));
    }
  };
  const cancel = async () => {
    const attempt = runtime;
    if (!attempt || disposed) return;
    if (snapshot.status === "preparing" || snapshot.status === "credential") {
      finish(attempt);
      return;
    }
    if (snapshot.status !== "connecting") return;
    setStatus(attempt, "cancelling");
    try {
      const accepted = await deps.cancel(attempt.type, attempt.requestId);
      if (active(attempt) && (snapshot as AttemptSnapshot).status === "cancelling" && !accepted)
        setStatus(attempt, "connecting");
    } catch (error) {
      if (active(attempt) && (snapshot as AttemptSnapshot).status === "cancelling")
        setStatus(attempt, "connecting", { cancelError: deps.errorMessage(error) });
    }
  };
  return {
    getSnapshot: () => snapshot,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    start: (type: ConnectionType, input: Input, target = "") => {
      if (disposed || runtime || snapshot.status !== "editing") return Promise.resolve();
      const attempt: AttemptRuntime = { requestId: deps.createRequestId(), type };
      runtime = attempt;
      transition({ type: "start", target, requestId: attempt.requestId, connectionType: type });
      return run(attempt, structuredClone(input));
    },
    submitCredential: (requestId: string, promptId: string, value: string) => {
      const attempt = runtime;
      if (
        !attempt ||
        disposed ||
        snapshot.status !== "credential" ||
        snapshot.requestId !== requestId ||
        snapshot.prompt?.promptId !== promptId ||
        !attempt.credential
      )
        return;
      const pending = attempt.credential;
      attempt.credential = undefined;
      setStatus(attempt, "preparing");
      pending.resolve(value);
    },
    cancel,
    retryRegistration: () => {
      if (!runtime || disposed || snapshot.status !== "finalization_failed")
        return Promise.resolve();
      setStatus(runtime, "finalizing");
      return register(runtime);
    },
    dispose: () => {
      if (disposed) return;
      disposed = true;
      deps.stopDiagnostics();
      listeners.clear();
      const attempt = runtime;
      if (!attempt) return;
      attempt.credential?.reject(abandoned);
      attempt.credential = undefined;
      if (snapshot.status === "connecting")
        void deps.cancel(attempt.type, attempt.requestId).catch(() => {});
      if (snapshot.status === "finalization_failed")
        void disconnect(attempt).then(() => {
          release(attempt);
        });
    },
  };
}
