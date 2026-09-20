import { describe, expect, it, vi } from "vitest";
import type { TabInfo } from "../../types";
import {
  createManualLogController,
  ManualLogOperationError,
  type ManualLogControllerDependencies,
} from "./manualLogController";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function terminalTab(overrides: Partial<TabInfo> = {}): TabInfo {
  return {
    kind: "terminal",
    id: "tab-1",
    sessionId: "session-1",
    title: "Terminal",
    connectionType: "ssh",
    isConnected: true,
    encoding: "utf-8",
    terminalMode: "general",
    isManualLogging: false,
    isManualLoggingPaused: false,
    ...overrides,
  };
}

function setup(initialTabs = [terminalTab()]) {
  let tabs = initialTabs;
  const calls: string[] = [];
  const dependencies: ManualLogControllerDependencies = {
    getTabBySessionId: vi.fn((sessionId) => {
      calls.push(`resolve:${sessionId}`);
      return tabs.find((tab) => tab.sessionId === sessionId) ?? null;
    }),
    startBackend: vi.fn(async ({ sessionId }) => {
      calls.push(`start:${sessionId}`);
      return `C:\\logs\\${sessionId}.log`;
    }),
    stopBackend: vi.fn(async (sessionId) => {
      calls.push(`stop:${sessionId}`);
    }),
    isBackendActive: vi.fn(async () => false),
    flush: vi.fn(async (tabId) => {
      calls.push(`flush:${tabId}`);
    }),
    updateMetadata: vi.fn(async (tabId, patch) => {
      calls.push(`metadata:${tabId}:${String(patch.isManualLogging)}`);
      tabs = tabs.map((tab) =>
        tab.id === tabId
          ? {
              ...tab,
              ...patch,
            }
          : tab
      );
    }),
  };
  const controller = createManualLogController(dependencies);
  return {
    controller,
    dependencies,
    calls,
    setTabs(nextTabs: TabInfo[]) {
      tabs = nextTabs;
    },
    getTabs() {
      return tabs;
    },
  };
}

const startTarget = {
  sessionId: "session-1",
  expectedTabId: "tab-1",
  connectionType: "ssh" as const,
  target: "Terminal",
};

describe("manualLogController", () => {
  it("runs dialog and immediate starts through the same backend and metadata path", async () => {
    const gui = setup();
    const lease = gui.controller.beginStart(startTarget);
    await expect(
      lease.commit({ filePath: "C:\\chosen.log", writeMode: "append" })
    ).resolves.toMatchObject({ alreadyActive: false, metadataChanged: true });
    lease.release();

    expect(gui.dependencies.startBackend).toHaveBeenCalledWith({
      sessionId: "session-1",
      connectionType: "ssh",
      target: "Terminal",
      filePath: "C:\\chosen.log",
      writeMode: "append",
    });
    expect(gui.dependencies.updateMetadata).toHaveBeenCalledWith("tab-1", {
      isManualLogging: true,
      isManualLoggingPaused: false,
      manualLogFilePath: "C:\\logs\\session-1.log",
    });

    const external = setup();
    await external.controller.start(
      { ...startTarget, expectedTabId: undefined },
      { filePath: null, writeMode: "overwrite" }
    );
    expect(external.dependencies.startBackend).toHaveBeenCalledTimes(1);
    expect(external.dependencies.updateMetadata).toHaveBeenCalledTimes(1);
  });

  it("orders flush, backend stop, and metadata using tab id rather than session id", async () => {
    const h = setup([
      terminalTab({
        id: "tab-different",
        isManualLogging: true,
        manualLogFilePath: "C:\\logs\\active.log",
      }),
    ]);

    await h.controller.stop({ sessionId: "session-1", expectedTabId: "tab-different" });

    expect(h.calls).toEqual([
      "resolve:session-1",
      "flush:tab-different",
      "stop:session-1",
      "metadata:tab-different:false",
    ]);
  });

  it("resumes an already active paused log without starting another backend log", async () => {
    const h = setup([
      terminalTab({
        isManualLogging: true,
        isManualLoggingPaused: true,
        manualLogFilePath: "C:\\logs\\active.log",
      }),
    ]);

    await expect(
      h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" })
    ).resolves.toEqual({
      filePath: "C:\\logs\\active.log",
      alreadyActive: true,
      metadataChanged: true,
    });
    expect(h.dependencies.startBackend).not.toHaveBeenCalled();
    expect(h.dependencies.updateMetadata).toHaveBeenCalledWith("tab-1", {
      isManualLoggingPaused: false,
    });
  });

  it("treats an already inactive stop as successful without flushing or calling backend", async () => {
    const h = setup();

    await expect(h.controller.stop({ sessionId: "session-1" })).resolves.toEqual({
      alreadyInactive: true,
      metadataChanged: false,
    });
    expect(h.dependencies.flush).not.toHaveBeenCalled();
    expect(h.dependencies.stopBackend).not.toHaveBeenCalled();
  });

  it("holds a synchronous reservation across the dialog and releases it on cancel", async () => {
    const h = setup();
    const lease = h.controller.beginStart(startTarget);

    await expect(
      h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" })
    ).rejects.toMatchObject({ stage: "busy", code: "operation_in_progress" });
    expect(h.dependencies.startBackend).not.toHaveBeenCalled();

    lease.release();
    await expect(
      h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" })
    ).resolves.toMatchObject({ alreadyActive: false });
  });

  it("stops a backend log after failed start metadata and transfer to another controller", async () => {
    const h = setup();
    let backendActive = false;
    vi.mocked(h.dependencies.startBackend).mockImplementation(async () => {
      backendActive = true;
      return "C:\\logs\\active.log";
    });
    vi.mocked(h.dependencies.isBackendActive).mockImplementation(async () => backendActive);
    vi.mocked(h.dependencies.stopBackend).mockImplementation(async () => {
      backendActive = false;
    });
    vi.mocked(h.dependencies.updateMetadata).mockRejectedValueOnce(new Error("metadata failed"));
    await expect(
      h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" })
    ).rejects.toMatchObject({ stage: "metadata" });
    expect(h.getTabs()[0].isManualLogging).toBe(false);

    const destination = createManualLogController(h.dependencies);
    await expect(destination.stop({ sessionId: "session-1" })).resolves.toEqual({
      alreadyInactive: false,
      metadataChanged: true,
    });
    expect(backendActive).toBe(false);
    expect(h.dependencies.stopBackend).toHaveBeenCalledOnce();
  });

  it("reports a backend state lookup failure instead of acknowledging an inactive stop", async () => {
    const h = setup();
    vi.mocked(h.dependencies.isBackendActive).mockRejectedValueOnce(new Error("lookup failed"));
    await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toMatchObject({
      stage: "backend",
    });
    expect(h.controller.isBusy("session-1")).toBe(false);
    expect(h.dependencies.stopBackend).not.toHaveBeenCalled();
  });

  it("rejects same-session overlap while allowing a different session to run", async () => {
    const firstStart = deferred<string>();
    const h = setup([terminalTab(), terminalTab({ id: "tab-2", sessionId: "session-2" })]);
    vi.mocked(h.dependencies.startBackend).mockImplementation(async ({ sessionId }) => {
      if (sessionId === "session-1") return firstStart.promise;
      return "C:\\logs\\session-2.log";
    });

    const first = h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" });
    await Promise.resolve();
    await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toMatchObject({
      stage: "busy",
    });
    await expect(
      h.controller.start(
        {
          sessionId: "session-2",
          expectedTabId: "tab-2",
          connectionType: "ssh",
          target: "Other",
        },
        { filePath: null, writeMode: "overwrite" }
      )
    ).resolves.toMatchObject({ filePath: "C:\\logs\\session-2.log" });

    firstStart.resolve("C:\\logs\\session-1.log");
    await first;
  });

  it("revalidates the fixed target after a dialog without following the active tab", async () => {
    const h = setup();
    const lease = h.controller.beginStart(startTarget);
    h.setTabs([terminalTab({ id: "other-tab", sessionId: "other-session" })]);

    await expect(
      lease.commit({ filePath: "C:\\chosen.log", writeMode: "overwrite" })
    ).rejects.toMatchObject({ stage: "session", code: "session_not_found" });
    expect(h.dependencies.startBackend).not.toHaveBeenCalled();
    lease.release();
  });

  it("rejects a disconnected or replaced tab after a dialog", async () => {
    const disconnected = setup();
    const disconnectedLease = disconnected.controller.beginStart(startTarget);
    disconnected.setTabs([terminalTab({ isConnected: false })]);
    await expect(
      disconnectedLease.commit({ filePath: "C:\\chosen.log", writeMode: "overwrite" })
    ).rejects.toMatchObject({ code: "session_disconnected" });
    disconnectedLease.release();

    const replaced = setup();
    const replacedLease = replaced.controller.beginStart(startTarget);
    replaced.setTabs([terminalTab({ id: "replacement-tab" })]);
    await expect(
      replacedLease.commit({ filePath: "C:\\chosen.log", writeMode: "overwrite" })
    ).rejects.toMatchObject({ code: "session_changed" });
    replacedLease.release();
  });

  it("flushes before pausing and never invokes stop for pause or resume", async () => {
    const h = setup([
      terminalTab({ isManualLogging: true, manualLogFilePath: "C:\\logs\\active.log" }),
    ]);

    await h.controller.setPaused({ sessionId: "session-1" }, true);
    expect(h.calls.slice(-2)).toEqual(["flush:tab-1", "metadata:tab-1:undefined"]);
    expect(h.dependencies.stopBackend).not.toHaveBeenCalled();

    await h.controller.setPaused({ sessionId: "session-1" }, false);
    expect(h.dependencies.flush).toHaveBeenCalledTimes(1);
    expect(h.dependencies.stopBackend).not.toHaveBeenCalled();
  });

  it.each([["flush", "flush"] as const, ["backend", "backend"] as const])(
    "releases busy after a %s failure",
    async (failurePoint, expectedStage) => {
      const h = setup([
        terminalTab({ isManualLogging: true, manualLogFilePath: "C:\\logs\\active.log" }),
      ]);
      if (failurePoint === "flush") {
        vi.mocked(h.dependencies.flush).mockRejectedValueOnce(new Error("flush failed"));
      } else {
        vi.mocked(h.dependencies.stopBackend).mockRejectedValueOnce(new Error("stop failed"));
      }

      await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toMatchObject({
        stage: expectedStage,
      });
      expect(h.controller.isBusy("session-1")).toBe(false);
    }
  );

  it("repairs start metadata without starting or overwriting the backend twice", async () => {
    const h = setup();
    vi.mocked(h.dependencies.updateMetadata).mockRejectedValueOnce(new Error("metadata failed"));

    await expect(
      h.controller.start(startTarget, { filePath: "C:\\chosen.log", writeMode: "overwrite" })
    ).rejects.toMatchObject({ stage: "metadata" });
    expect(h.controller.isBusy("session-1")).toBe(false);

    await expect(
      h.controller.start(startTarget, { filePath: "C:\\other.log", writeMode: "overwrite" })
    ).resolves.toMatchObject({ filePath: "C:\\logs\\session-1.log", alreadyActive: true });
    expect(h.dependencies.startBackend).toHaveBeenCalledTimes(1);
  });

  it("repairs stop metadata without flushing or stopping the backend twice", async () => {
    const h = setup([
      terminalTab({ isManualLogging: true, manualLogFilePath: "C:\\logs\\active.log" }),
    ]);
    vi.mocked(h.dependencies.updateMetadata).mockRejectedValueOnce(new Error("metadata failed"));

    await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toBeInstanceOf(
      ManualLogOperationError
    );
    await expect(h.controller.stop({ sessionId: "session-1" })).resolves.toEqual({
      alreadyInactive: true,
      metadataChanged: true,
    });
    expect(h.dependencies.flush).toHaveBeenCalledTimes(1);
    expect(h.dependencies.stopBackend).toHaveBeenCalledTimes(1);
  });

  it("repairs a completed stop before starting the same session again", async () => {
    const h = setup([
      terminalTab({ isManualLogging: true, manualLogFilePath: "C:\\logs\\active.log" }),
    ]);
    vi.mocked(h.dependencies.updateMetadata).mockRejectedValueOnce(new Error("metadata failed"));

    await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toMatchObject({
      stage: "metadata",
    });
    await expect(
      h.controller.start(startTarget, { filePath: "C:\\next.log", writeMode: "overwrite" })
    ).resolves.toMatchObject({ alreadyActive: false });

    expect(h.dependencies.stopBackend).toHaveBeenCalledTimes(1);
    expect(h.dependencies.startBackend).toHaveBeenCalledTimes(1);
    expect(h.dependencies.updateMetadata).toHaveBeenNthCalledWith(2, "tab-1", {
      isManualLogging: false,
      isManualLoggingPaused: false,
    });
  });
});
