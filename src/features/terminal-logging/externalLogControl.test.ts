import { describe, expect, it, vi } from "vitest";
import { ManualLogOperationError, type ManualLogController } from "./manualLogController";
import {
  createExternalLogControlHandlers,
  externalLogControlErrorMessage,
  subscribeExternalLogControl,
  type ExternalLogControlRequestPayload,
  type ExternalLogControlResponse,
} from "./externalLogControl";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const payload: ExternalLogControlRequestPayload = {
  request_id: "request-1",
  session_id: "session-1",
  connection_type: "ssh",
  target: "Terminal",
};

function setup() {
  const order: string[] = [];
  const start = vi.fn(async () => {
    order.push("operation");
    return {
      filePath: "C:\\logs\\session.log",
      alreadyActive: false,
      metadataChanged: true,
    };
  });
  const stop = vi.fn(async () => {
    order.push("operation");
    return { alreadyInactive: false, metadataChanged: true };
  });
  const controller = {
    start,
    stop,
  } as unknown as ManualLogController;
  const submit = vi.fn(async (_response: ExternalLogControlResponse) => {
    order.push("ack");
  });
  const waitForUiUpdate = vi.fn(async () => {
    order.push("ui");
  });
  const onSubmitError = vi.fn();
  const handlers = createExternalLogControlHandlers({
    controller,
    submit,
    waitForUiUpdate,
    onSubmitError,
  });
  return { handlers, start, stop, submit, waitForUiUpdate, onSubmitError, order };
}

describe("externalLogControl", () => {
  it.each([
    ["session_not_found", "Session not found."],
    ["session_changed", "Session not found."],
    ["session_disconnected", "Session is disconnected."],
    ["operation_in_progress", "A log operation is already in progress for this session."],
    ["logging_not_active", "Manual logging is not active."],
  ] as const)("maps %s to a stable external-control message", (code, expected) => {
    expect(
      externalLogControlErrorMessage(new ManualLogOperationError("session", code, null), "start")
    ).toBe(expected);
  });

  it("acks a start only after the shared operation and UI reflection", async () => {
    const h = setup();
    await h.handlers.start(payload);

    expect(h.order).toEqual(["operation", "ui", "ack"]);
    expect(h.submit).toHaveBeenCalledOnce();
    expect(h.submit).toHaveBeenCalledWith({
      requestId: "request-1",
      filePath: "C:\\logs\\session.log",
      error: null,
    });
  });

  it("preserves stop ordering through UI reflection and a single ack", async () => {
    const h = setup();
    await h.handlers.stop(payload);

    expect(h.order).toEqual(["operation", "ui", "ack"]);
    expect(h.submit).toHaveBeenCalledOnce();
  });

  it("does not wait for UI when an idempotent operation changed no metadata", async () => {
    const h = setup();
    h.stop.mockResolvedValueOnce({
      alreadyInactive: true,
      metadataChanged: false,
    });

    await h.handlers.stop(payload);

    expect(h.waitForUiUpdate).not.toHaveBeenCalled();
    expect(h.submit).toHaveBeenCalledOnce();
  });

  it("returns operation and UI-wait failures exactly once", async () => {
    const operationFailure = setup();
    operationFailure.start.mockRejectedValueOnce(new Error("start failed"));
    await operationFailure.handlers.start(payload);
    expect(operationFailure.submit).toHaveBeenCalledOnce();
    expect(operationFailure.submit.mock.calls[0][0].error).toContain("start failed");

    const waitFailure = setup();
    waitFailure.waitForUiUpdate.mockRejectedValueOnce(new Error("wait failed"));
    await waitFailure.handlers.stop(payload);
    expect(waitFailure.submit).toHaveBeenCalledOnce();
    expect(waitFailure.submit.mock.calls[0][0].error).toContain("wait failed");
  });

  it("does not retry a rejected acknowledgement", async () => {
    const h = setup();
    h.submit.mockRejectedValueOnce(new Error("request expired"));

    await h.handlers.start(payload);

    expect(h.submit).toHaveBeenCalledOnce();
    expect(h.onSubmitError).toHaveBeenCalledOnce();
  });

  it("unlistens registrations that resolve after disposal and ignores obsolete callbacks", async () => {
    const startRegistration = deferred<() => void>();
    const stopRegistration = deferred<() => void>();
    const listeners = new Map<
      string,
      (event: { payload: ExternalLogControlRequestPayload }) => void
    >();
    const unlistenStart = vi.fn();
    const unlistenStop = vi.fn();
    const listen: Parameters<typeof subscribeExternalLogControl>[0] = <T>(
      event: string,
      listener: (event: { payload: T }) => void
    ): Promise<() => void> => {
      listeners.set(
        event,
        listener as (event: { payload: ExternalLogControlRequestPayload }) => void
      );
      return event.endsWith("start-request")
        ? (startRegistration.promise as Promise<() => void>)
        : (stopRegistration.promise as Promise<() => void>);
    };
    const handlers = { start: vi.fn(async () => {}), stop: vi.fn(async () => {}) };
    const dispose = subscribeExternalLogControl(listen, handlers, vi.fn());

    dispose();
    listeners.get("external-control://log-start-request")?.({ payload });
    startRegistration.resolve(unlistenStart);
    stopRegistration.resolve(unlistenStop);
    await Promise.all([startRegistration.promise, stopRegistration.promise]);
    await Promise.resolve();

    expect(handlers.start).not.toHaveBeenCalled();
    expect(unlistenStart).toHaveBeenCalledOnce();
    expect(unlistenStop).toHaveBeenCalledOnce();
  });

  it("allows a handler accepted before disposal to finish", async () => {
    const handling = deferred<undefined>();
    const listeners = new Map<
      string,
      (event: { payload: ExternalLogControlRequestPayload }) => void
    >();
    const listen: Parameters<typeof subscribeExternalLogControl>[0] = async <T>(
      event: string,
      listener: (event: { payload: T }) => void
    ) => {
      listeners.set(
        event,
        listener as (event: { payload: ExternalLogControlRequestPayload }) => void
      );
      return () => {};
    };
    const handlers = {
      start: vi.fn(() => handling.promise),
      stop: vi.fn(async () => {}),
    };
    const dispose = subscribeExternalLogControl(listen, handlers, vi.fn());
    await Promise.resolve();

    listeners.get("external-control://log-start-request")?.({ payload });
    dispose();
    handling.resolve(undefined);
    await handling.promise;

    expect(handlers.start).toHaveBeenCalledOnce();
  });

  it("disposes one successful listener when the other registration fails", async () => {
    const unlisten = vi.fn();
    const onRegistrationError = vi.fn();
    const listen: Parameters<typeof subscribeExternalLogControl>[0] = <T>(
      event: string,
      _listener: (event: { payload: T }) => void
    ) =>
      event.endsWith("start-request")
        ? Promise.resolve(unlisten)
        : Promise.reject(new Error("registration failed"));
    const dispose = subscribeExternalLogControl(
      listen,
      { start: vi.fn(async () => {}), stop: vi.fn(async () => {}) },
      onRegistrationError
    );
    await Promise.resolve();
    await Promise.resolve();

    dispose();

    expect(onRegistrationError).toHaveBeenCalledOnce();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});
