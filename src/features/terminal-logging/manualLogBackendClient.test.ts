import { describe, it, expect, vi } from "vitest";
import type { WorkspaceSnapshot } from "../../types";
import { createManualLogBackendClient } from "./manualLogBackendClient";
import { createManualLogController } from "./manualLogController";
import { createExternalLogControlHandlers } from "./externalLogControl";

describe("manual log backend integration", () => {
  it("applies the returned workspace revision before UI acknowledgement without an event", async () => {
    const order: string[] = [];
    const snapshot = { revision: 12, window_id: "main" } as WorkspaceSnapshot;
    const status = { active: true, paused: false, filePath: "synthetic.log" };
    const invoke = async <T>(command: string): Promise<T> => {
      order.push(command);
      return (
        command === "logger_get_manual_status" ? { ...status, active: false } : { status, snapshot }
      ) as T;
    };
    const apply = vi.fn(() => {
      order.push("apply-revision");
    });
    const controller = createManualLogController({
      ...createManualLogBackendClient(invoke, apply),
      getTabBySessionId: () => ({ id: "tab", sessionId: "session", isConnected: true }) as never,
    });
    const handlers = createExternalLogControlHandlers({
      controller,
      waitForUiUpdate: async () => {
        order.push("ui-commit");
      },
      submit: async () => {
        order.push("ack");
      },
      onSubmitError: () => {},
    });
    await handlers.start({
      request_id: "request",
      session_id: "session",
      connection_type: "ssh",
      target: "synthetic",
      file_path: null,
      write_mode: null,
    });
    expect(apply).toHaveBeenCalledWith(snapshot);
    expect(order).toEqual([
      "logger_get_manual_status",
      "logger_start_manual",
      "apply-revision",
      "ui-commit",
      "ack",
    ]);
  });

  it("applies pause and stop results before their promises settle", async () => {
    const apply = vi.fn();
    const snapshot = { revision: 13 } as WorkspaceSnapshot;
    const invoke = async <T>(): Promise<T> =>
      ({ status: { active: false, paused: false, filePath: null }, snapshot }) as T;
    const client = createManualLogBackendClient(invoke, apply);
    await client.setBackendPaused("session", true);
    expect(apply).toHaveBeenCalledTimes(1);
    await client.stopBackend("session");
    expect(apply).toHaveBeenCalledTimes(2);
  });
});
