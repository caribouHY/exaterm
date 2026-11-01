import { describe, expect, it, vi } from "vitest";
import type { LogSession } from "../../types";
import {
  createLogViewerLoadController,
  createLogViewerLoadState,
  getLogViewerPresentation,
} from "./logViewerLoadController";

const sessions: LogSession[] = [
  {
    session_id: "test-session",
    connection_type: "ssh",
    target: "example.invalid",
    started_at: "2026-10-11T00:00:00Z",
    file_path: "C:\\test-logs\\session.log",
    log_mode: "auto",
  },
];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function setup() {
  const source = {
    getSessions: vi.fn<() => Promise<LogSession[]>>().mockResolvedValue(sessions),
    getLogDir: vi.fn<() => Promise<string>>().mockResolvedValue("C:\\test-logs"),
  };
  const onChange = vi.fn();
  const controller = createLogViewerLoadController(source, onChange);
  return {
    source,
    onChange,
    controller,
    view: (deleting = false) => getLogViewerPresentation(controller.getState(), deleting),
  };
}

describe("log viewer loading", () => {
  it("does not show empty history before the first successful response", async () => {
    const { source, controller, view } = setup();
    const pending = deferred<LogSession[]>();
    source.getSessions.mockReturnValueOnce(pending.promise);
    expect(getLogViewerPresentation(createLogViewerLoadState(), false).showEmpty).toBe(false);
    const loading = controller.load();
    expect(view()).toMatchObject({ showEmpty: false, refreshDisabled: true, deleteDisabled: true });
    pending.resolve([]);
    await loading;
    expect(view()).toMatchObject({ showEmpty: true, showTable: false, sessionsErrorKey: null });
  });

  it("shows an initial invoke failure rather than no logs", async () => {
    const { source, controller, view } = setup();
    source.getSessions.mockRejectedValueOnce(new Error("private diagnostic"));
    await controller.load();
    expect(view()).toMatchObject({
      showEmpty: false,
      showTable: false,
      sessionsErrorKey: "logs.load_failed",
      logDirErrorKey: null,
      refreshLabelKey: "logs.retry",
      refreshDisabled: false,
      deleteDisabled: true,
    });
    expect(JSON.stringify(controller.getState())).not.toContain("private diagnostic");
  });

  it("preserves rows during refresh and on failure, then clears the error on retry", async () => {
    const { source, controller, view } = setup();
    await controller.load();
    const pending = deferred<LogSession[]>();
    source.getSessions.mockReturnValueOnce(pending.promise);
    const refresh = controller.load();
    expect(view()).toMatchObject({ sessions, showTable: true, deleteDisabled: true });
    pending.reject("read failure");
    await refresh;
    expect(view()).toMatchObject({
      sessions,
      showTable: true,
      showEmpty: false,
      sessionsErrorKey: "logs.reload_failed",
      deleteDisabled: true,
    });
    source.getSessions.mockResolvedValueOnce([]);
    await controller.load();
    expect(view()).toMatchObject({
      showEmpty: true,
      sessionsErrorKey: null,
      refreshLabelKey: "logs.refresh",
    });
  });

  it("does not show a previously empty result as current after a failed refresh", async () => {
    const { source, controller, view } = setup();
    source.getSessions.mockResolvedValueOnce([]);
    await controller.load();
    source.getSessions.mockRejectedValueOnce("broken index");
    await controller.load();
    expect(view()).toMatchObject({ showEmpty: false, sessionsErrorKey: "logs.reload_failed" });
  });

  it("keeps history usable when only the folder invoke fails", async () => {
    const { source, controller, view } = setup();
    source.getLogDir.mockRejectedValueOnce("folder failure");
    await controller.load();
    expect(view()).toMatchObject({
      showTable: true,
      deleteDisabled: false,
      logDirErrorKey: "logs.directory_load_failed",
    });
    expect(controller.getState().logDir).toEqual({ phase: "error", data: null });
    await controller.load();
    expect(view().logDirErrorKey).toBeNull();
  });

  it("retains a previously loaded folder on refresh failure", async () => {
    const { source, controller, view } = setup();
    await controller.load();
    source.getLogDir.mockRejectedValueOnce("folder failure");
    await controller.load();
    expect(controller.getState().logDir).toEqual({ phase: "error", data: "C:\\test-logs" });
    expect(view().logDirErrorKey).toBe("logs.directory_reload_failed");
  });

  it("publishes history without waiting for the folder and disables refresh until both settle", async () => {
    const { source, controller, view } = setup();
    const pending = deferred<string>();
    source.getLogDir.mockReturnValueOnce(pending.promise);
    const loading = controller.load();
    await Promise.resolve();
    expect(view()).toMatchObject({ showTable: true, refreshDisabled: true });
    expect(controller.getState().logDir.phase).toBe("loading");
    pending.resolve("C:\\test-logs");
    await loading;
    expect(view().refreshDisabled).toBe(false);
  });

  it("publishes the folder while history is pending", async () => {
    const { source, controller } = setup();
    const pending = deferred<LogSession[]>();
    source.getSessions.mockReturnValueOnce(pending.promise);
    const loading = controller.load();
    await Promise.resolve();
    expect(controller.getState().logDir.phase).toBe("success");
    expect(controller.getState().sessions.phase).toBe("loading");
    pending.resolve(sessions);
    await loading;
  });

  it("resolves refresh failure so a successful deletion is not reported as failed", async () => {
    const { source, controller, view } = setup();
    await controller.load();
    source.getSessions.mockRejectedValueOnce("post-delete read failure");
    await expect(controller.load()).resolves.toBeUndefined();
    expect(view()).toMatchObject({
      sessions,
      sessionsErrorKey: "logs.reload_failed",
      deleteDisabled: true,
    });
  });

  it("disables both controls during deletion", async () => {
    const { controller, view } = setup();
    await controller.load();
    expect(view(true)).toMatchObject({ refreshDisabled: true, deleteDisabled: true });
    expect(view(false)).toMatchObject({ refreshDisabled: false, deleteDisabled: false });
  });

  it.each(["success", "failure"])(
    "ignores obsolete request %s for both resources",
    async (outcome) => {
      const { source, controller, onChange } = setup();
      const oldSessions = deferred<LogSession[]>();
      const oldDir = deferred<string>();
      source.getSessions.mockReturnValueOnce(oldSessions.promise);
      source.getLogDir.mockReturnValueOnce(oldDir.promise);
      const oldLoad = controller.load();
      source.getSessions.mockResolvedValueOnce([]);
      source.getLogDir.mockResolvedValueOnce("C:\\new-logs");
      await controller.load();
      const state = controller.getState();
      onChange.mockClear();
      if (outcome === "success") {
        oldSessions.resolve(sessions);
        oldDir.resolve("C:\\old-logs");
      } else {
        oldSessions.reject("obsolete failure");
        oldDir.reject("obsolete failure");
      }
      await oldLoad;
      expect(controller.getState()).toBe(state);
      expect(onChange).not.toHaveBeenCalled();
    }
  );

  it.each(["success", "failure"])(
    "ignores %s after disposal and does not start more invokes",
    async (outcome) => {
      const { source, controller, onChange } = setup();
      const pendingSessions = deferred<LogSession[]>();
      const pendingDir = deferred<string>();
      source.getSessions.mockReturnValueOnce(pendingSessions.promise);
      source.getLogDir.mockReturnValueOnce(pendingDir.promise);
      const loading = controller.load();
      controller.dispose();
      onChange.mockClear();
      if (outcome === "success") {
        pendingSessions.resolve(sessions);
        pendingDir.resolve("C:\\test-logs");
      } else {
        pendingSessions.reject("disposed failure");
        pendingDir.reject("disposed failure");
      }
      await loading;
      await controller.load();
      expect(onChange).not.toHaveBeenCalled();
      expect(source.getSessions).toHaveBeenCalledOnce();
      expect(source.getLogDir).toHaveBeenCalledOnce();
    }
  );
});
