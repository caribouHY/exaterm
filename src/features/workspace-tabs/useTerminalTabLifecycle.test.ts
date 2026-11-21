import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, expect, it, vi } from "vitest";
import type { TabInfo, WorkspaceSnapshot } from "../../types";
import { useTerminalTabLifecycle } from "./useTerminalTabLifecycle";
import { workspaceClient } from "./workspaceClient";

function terminalTab(id: string, isConnected = false): TabInfo {
  return {
    kind: "terminal",
    id,
    sessionId: id,
    title: id,
    connectionType: "telnet",
    isConnected,
    encoding: "utf-8",
    terminalMode: "general",
  };
}

afterEach(() => vi.restoreAllMocks());

it("continues past a failed close, rechecks queued targets, and closes the initial selection last", async () => {
  const activeTab = terminalTab("active");
  const firstTab = terminalTab("first");
  const movedTab = terminalTab("moved");
  const closingTab = terminalTab("closing");
  const lastTab = terminalTab("last");
  const connectedTab = terminalTab("connected", true);
  let tabs = [activeTab, firstTab, movedTab, closingTab, lastTab, connectedTab];
  let closingTabIds: string[] = [];
  const captureRemovalFocus = vi.fn((tabId: string) => ({
    tabId,
    wasActive: tabId === activeTab.id,
    selectionEpoch: 0,
    leftId: null,
    rightId: null,
  }));
  const windowTabs = {
    windowId: "main",
    getCurrentState: () => ({
      tabs,
      appTabs: tabs,
      activeTabId: activeTab.id,
      closingTabIds,
      utilityTabs: [],
      tabOrder: tabs.map((tab) => tab.id),
      dragPreview: null,
    }),
    captureRemovalFocus,
    restoreFocusAfterRemoval: vi.fn(),
    beginClosingTab: vi.fn(),
    endClosingTab: vi.fn(),
    applyWorkspaceSnapshot: vi.fn(),
    closeUtilityTab: vi.fn(),
  };
  const snapshot: WorkspaceSnapshot = {
    revision: 1,
    window_id: "main",
    window: { window_id: "main", label: "main", tab_order: [] },
    tabs: [],
  };
  let rejectFirstClose!: (reason: Error) => void;
  const firstClose = new Promise<WorkspaceSnapshot>((_, reject) => {
    rejectFirstClose = reject;
  });
  const closeTab = vi
    .spyOn(workspaceClient, "closeTab")
    .mockReturnValueOnce(firstClose)
    .mockResolvedValue(snapshot);
  vi.spyOn(console, "error").mockImplementation(() => {});
  const onTerminalRemoved = vi.fn();
  let lifecycle!: ReturnType<typeof useTerminalTabLifecycle>;
  function Harness() {
    lifecycle = useTerminalTabLifecycle({ tabs: windowTabs, onTerminalRemoved });
    return null;
  }
  renderToStaticMarkup(createElement(Harness));

  const batchClose = lifecycle.closeDisconnectedTabs();
  await lifecycle.closeDisconnectedTabs();
  expect(closeTab.mock.calls).toEqual([["first"]]);

  tabs = [activeTab, firstTab, closingTab, lastTab, { ...connectedTab, isConnected: false }];
  closingTabIds = [closingTab.id];
  rejectFirstClose(new Error("synthetic close failure"));
  await batchClose;

  expect(closeTab.mock.calls).toEqual([["first"], ["last"], ["active"]]);
  expect(onTerminalRemoved.mock.calls).toEqual([["last"], ["active"]]);
});
