import { describe, expect, it, vi } from "vitest";
import type { TabInfo } from "../../types";
import {
  createManualLogController,
  ManualLogOperationError,
  type ManualLogControllerDependencies,
} from "./manualLogController";

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
  const getTabBySessionId = vi.fn((sessionId: string) => {
    calls.push(`resolve:${sessionId}`);
    return tabs.find((tab) => tab.sessionId === sessionId) ?? null;
  });
  const startBackend = vi.fn(async ({ sessionId }: { sessionId: string }) => {
    calls.push(`start:${sessionId}`);
    return `C:\\logs\\${sessionId}.log`;
  });
  const stopBackend = vi.fn(async (sessionId: string) => {
    calls.push(`stop:${sessionId}`);
  });
  const getBackendStatus = vi.fn(async () => ({
    active: false,
    paused: false,
    filePath: null as string | null,
  }));
  const setBackendPaused = vi.fn(async (_sessionId: string, _paused: boolean) => {});
  const dependencies: ManualLogControllerDependencies = {
    getTabBySessionId,
    startBackend,
    stopBackend,
    getBackendStatus,
    setBackendPaused,
  };
  const controller = createManualLogController(dependencies);
  return {
    controller,
    dependencies,
    startBackend,
    stopBackend,
    getBackendStatus,
    setBackendPaused,

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
  it("routes dialog and external starts through the authoritative backend", async () => {
    const h = setup();
    const lease = h.controller.beginStart(startTarget);
    await expect(lease.commit({ filePath: null, writeMode: "overwrite" })).resolves.toMatchObject({
      alreadyActive: false,
      metadataChanged: true,
    });
    lease.release();
    expect(h.startBackend).toHaveBeenCalledWith({
      ...startTarget,
      filePath: null,
      writeMode: "overwrite",
    });
  });

  it("uses backend pause state when workspace metadata is stale", async () => {
    const h = setup();
    h.getBackendStatus.mockResolvedValue({ active: true, paused: true, filePath: "synthetic.log" });
    await expect(h.controller.setPaused({ sessionId: "session-1" }, false)).resolves.toEqual({
      changed: true,
    });
    expect(h.setBackendPaused).toHaveBeenCalledWith("session-1", false);
    await h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" });
    expect(h.startBackend).toHaveBeenCalledOnce();
  });

  it("delegates inactive stops too so stale metadata is repaired centrally", async () => {
    const h = setup([terminalTab({ isManualLogging: true })]);
    await expect(h.controller.stop({ sessionId: "session-1" })).resolves.toEqual({
      alreadyInactive: true,
      metadataChanged: true,
    });
    expect(h.stopBackend).toHaveBeenCalledWith("session-1");
  });

  it("holds a reservation across the save dialog and rejects a released lease", async () => {
    const h = setup();
    const lease = h.controller.beginStart(startTarget);
    await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toMatchObject({
      code: "operation_in_progress",
    });
    lease.release();
    await expect(lease.commit({ filePath: null, writeMode: "overwrite" })).rejects.toMatchObject({
      code: "operation_in_progress",
    });
    expect(h.startBackend).not.toHaveBeenCalled();
  });

  it("revalidates a fixed session and tab after the dialog and status lookup", async () => {
    const h = setup();
    const lease = h.controller.beginStart(startTarget);
    h.setTabs([terminalTab({ id: "replacement" })]);
    await expect(lease.commit({ filePath: null, writeMode: "overwrite" })).rejects.toMatchObject({
      code: "session_changed",
    });
    lease.release();
    h.setTabs([terminalTab()]);
    h.getBackendStatus.mockImplementation(async () => {
      h.setTabs([]);
      return { active: false, paused: false, filePath: null };
    });
    await expect(
      h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" })
    ).rejects.toMatchObject({ code: "session_not_found" });
    expect(h.startBackend).not.toHaveBeenCalled();
  });

  it("releases busy state after lookup and operation failures", async () => {
    const h = setup();
    h.getBackendStatus.mockRejectedValueOnce(new Error("lookup failed"));
    await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toBeInstanceOf(
      ManualLogOperationError
    );
    expect(h.controller.isBusy("session-1")).toBe(false);
    h.stopBackend.mockRejectedValueOnce(new Error("stop failed"));
    await expect(h.controller.stop({ sessionId: "session-1" })).rejects.toMatchObject({
      stage: "backend",
    });
    expect(h.controller.isBusy("session-1")).toBe(false);
  });

  it("rejects inactive pause and disconnected starts", async () => {
    const h = setup();
    await expect(h.controller.setPaused({ sessionId: "session-1" }, true)).rejects.toMatchObject({
      code: "logging_not_active",
    });
    h.setTabs([terminalTab({ isConnected: false })]);
    await expect(
      h.controller.start(startTarget, { filePath: null, writeMode: "overwrite" })
    ).rejects.toMatchObject({ code: "session_disconnected" });
  });
});
