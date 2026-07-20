import { describe, expect, it } from "vitest";
import type {
  AppTabInfo,
  ForeignTabPlacement,
  TabInfo,
  WorkspaceSnapshot,
  WorkspaceTabInfo,
  WorkspaceTabUpdate,
} from "../../types";
import {
  applyForeignTabPlacement,
  getRemovalFocusNeighbors,
  orderAppTabs,
  reconcileTabOrder,
  reconcileWorkspaceSnapshot,
  reorderTabIds,
  resolveRemovalFocusTarget,
  tabOrdersEqual,
  workspaceTabToTabInfo,
} from "./tabStripModel";

function terminalTab(id: string): TabInfo {
  return {
    kind: "terminal",
    id,
    title: id.toUpperCase(),
    connectionType: "ssh",
    sessionId: `session-${id}`,
    isConnected: true,
    encoding: "utf-8",
    terminalMode: "general",
  };
}

function utilityTab(kind: "settings" | "logs"): AppTabInfo {
  return { kind, id: kind };
}

function workspaceTab(id: string): WorkspaceTabInfo {
  return {
    tab_id: id,
    session_id: `session-${id}`,
    connection_type: "ssh",
    title: id.toUpperCase(),
    owner_window_id: "main",
    encoding: "utf-8",
    terminal_mode: "general",
    is_connected: true,
    is_auto_logging: false,
    is_manual_logging: false,
    is_logging_paused: false,
  };
}

function workspaceSnapshot({
  revision = 1,
  windowId = "main",
  tabs = [workspaceTab("a")],
  tabOrder = tabs.map((tab) => tab.tab_id),
  activeTabId = tabs[0]?.tab_id ?? null,
  tabUpdate = null,
}: {
  revision?: number;
  windowId?: string;
  tabs?: WorkspaceTabInfo[];
  tabOrder?: string[];
  activeTabId?: string | null;
  tabUpdate?: WorkspaceTabUpdate | null;
} = {}): WorkspaceSnapshot {
  return {
    revision,
    window_id: windowId,
    window: {
      window_id: windowId,
      label: windowId,
      tab_order: tabOrder,
      active_tab_id: activeTabId,
    },
    tabs,
    tab_update: tabUpdate,
  };
}

function reconcileSnapshot(
  snapshot: WorkspaceSnapshot,
  overrides: Partial<Parameters<typeof reconcileWorkspaceSnapshot>[0]> = {}
) {
  return reconcileWorkspaceSnapshot({
    expectedWindowId: "main",
    lastAppliedRevision: null,
    currentTabs: [terminalTab("a")],
    utilityTabs: [],
    currentTabOrder: ["a"],
    currentActiveTabId: "a",
    foreignPlacement: null,
    snapshot,
    ...overrides,
  });
}

describe("orderAppTabs", () => {
  it("keeps the requested order and appends new tabs", () => {
    const tabs = [terminalTab("a"), terminalTab("b"), utilityTab("settings")];

    expect(orderAppTabs(tabs, ["settings", "a"])).toEqual([
      utilityTab("settings"),
      terminalTab("a"),
      terminalTab("b"),
    ]);
  });

  it("drops unknown order entries", () => {
    const tabs = [terminalTab("a"), utilityTab("logs")];

    expect(orderAppTabs(tabs, ["missing", "logs", "a"])).toEqual([
      utilityTab("logs"),
      terminalTab("a"),
    ]);
  });
});

describe("reorderTabIds", () => {
  it("moves tabs before and after a target", () => {
    expect(reorderTabIds(["a", "settings", "b"], "b", "a", "before")).toEqual([
      "b",
      "a",
      "settings",
    ]);
    expect(reorderTabIds(["a", "settings", "b"], "a", "b", "after")).toEqual([
      "settings",
      "b",
      "a",
    ]);
  });

  it("preserves the existing array when either tab is unknown", () => {
    const order = ["a", "b"];

    expect(reorderTabIds(order, "missing", "b", "before")).toBe(order);
    expect(reorderTabIds(order, "a", "missing", "after")).toBe(order);
  });
});

describe("tabOrdersEqual", () => {
  it("compares order and length", () => {
    expect(tabOrdersEqual(["a", "b"], ["a", "b"])).toBe(true);
    expect(tabOrdersEqual(["a", "b"], ["b", "a"])).toBe(false);
    expect(tabOrdersEqual(["a"], ["a", "b"])).toBe(false);
  });
});

describe("close focus selection", () => {
  const tabs = [terminalTab("a"), utilityTab("settings"), terminalTab("b")];

  it("records immediate right and left neighbors", () => {
    expect(getRemovalFocusNeighbors(tabs, "settings")).toEqual({ rightId: "b", leftId: "a" });
    expect(getRemovalFocusNeighbors(tabs, "missing")).toEqual({
      rightId: null,
      leftId: null,
    });
  });

  it("prefers the right neighbor, then the left neighbor", () => {
    expect(resolveRemovalFocusTarget(tabs, new Set(), "settings", "b", "a", "settings")).toBe("b");
    expect(resolveRemovalFocusTarget(tabs, new Set(["b"]), "settings", "b", "a", "b")).toBe("a");
  });

  it("falls back to the current active tab or null", () => {
    expect(resolveRemovalFocusTarget(tabs, new Set(), "settings", null, null, "a")).toBe("a");
    expect(resolveRemovalFocusTarget(tabs, new Set(["a", "b"]), "settings", null, null, "a")).toBe(
      null
    );
  });
});

describe("applyForeignTabPlacement", () => {
  const placement = (overrides: Partial<ForeignTabPlacement> = {}): ForeignTabPlacement => ({
    tabId: "moved",
    previousTabId: null,
    nextTabId: null,
    visibleSlotIndex: 1,
    ...overrides,
  });

  it("places a moved tab by next or previous visible anchors", () => {
    expect(
      applyForeignTabPlacement(
        ["a", "moved", "settings", "b"],
        ["a", "moved", "b"],
        placement({ nextTabId: "b" })
      )
    ).toEqual(["a", "settings", "moved", "b"]);
    expect(
      applyForeignTabPlacement(
        ["a", "moved", "settings", "b"],
        ["a", "moved", "b"],
        placement({ previousTabId: "a" })
      )
    ).toEqual(["a", "moved", "settings", "b"]);
  });

  it("uses the visible slot when anchors are unavailable", () => {
    expect(
      applyForeignTabPlacement(
        ["a", "moved", "settings", "b"],
        ["a", "moved", "b"],
        placement({ visibleSlotIndex: 2 })
      )
    ).toEqual(["a", "settings", "moved", "b"]);
  });

  it("rejects placements that contradict the workspace terminal order", () => {
    const order = ["a", "moved", "settings", "b"];

    expect(
      applyForeignTabPlacement(order, ["a", "moved", "b"], placement({ nextTabId: "a" }))
    ).toBe(order);
  });
});

describe("reconcileTabOrder", () => {
  it("keeps utility tabs while following the workspace terminal order", () => {
    expect(reconcileTabOrder(["a", "settings", "b"], ["a", "b"], ["b", "a"], ["settings"])).toEqual(
      ["b", "settings", "a"]
    );
  });

  it("places connected tabs at the visible end", () => {
    expect(
      reconcileTabOrder(["a", "settings"], ["a"], ["a", "b"], ["settings"], {
        kind: "connected",
        tab_id: "b",
      })
    ).toEqual(["a", "settings", "b"]);
  });
});

describe("workspaceTabToTabInfo", () => {
  it("maps workspace metadata without introducing defaults", () => {
    const source: WorkspaceTabInfo = {
      ...workspaceTab("a"),
      connection_type: "telnet",
      connection_info: { kind: "telnet", host: "example.invalid", port: 23 },
      manual_log_file_path: null,
    };

    expect(workspaceTabToTabInfo(source)).toEqual({
      id: "a",
      kind: "terminal",
      title: "A",
      connectionType: "telnet",
      sessionId: "session-a",
      isConnected: true,
      encoding: "utf-8",
      terminalMode: "general",
      connectionInfo: { kind: "telnet", host: "example.invalid", port: 23 },
      isAutoLogging: false,
      isManualLogging: false,
      isLoggingPaused: false,
      manualLogFilePath: undefined,
    });
  });
});

describe("reconcileWorkspaceSnapshot", () => {
  it("ignores snapshots for another window or an old revision", () => {
    expect(reconcileSnapshot(workspaceSnapshot({ windowId: "other" }))).toEqual({
      applied: false,
    });
    expect(
      reconcileSnapshot(workspaceSnapshot({ revision: 4 }), { lastAppliedRevision: 4 })
    ).toEqual({ applied: false });
    expect(
      reconcileSnapshot(workspaceSnapshot({ revision: 3 }), { lastAppliedRevision: 4 })
    ).toEqual({ applied: false });
  });

  it("adds and selects a newly connected tab after utility tabs", () => {
    const result = reconcileSnapshot(
      workspaceSnapshot({
        revision: 2,
        tabs: [workspaceTab("a"), workspaceTab("b")],
        tabOrder: ["a", "b"],
        activeTabId: "b",
        tabUpdate: { kind: "connected", tab_id: "b" },
      }),
      {
        utilityTabs: ["settings"],
        currentTabOrder: ["a", "settings"],
        currentActiveTabId: "settings",
      }
    );

    expect(result).toMatchObject({
      applied: true,
      revision: 2,
      tabOrder: ["a", "settings", "b"],
      activeTabId: "b",
    });
  });

  it("places and selects a tab moved into the current window", () => {
    const result = reconcileSnapshot(
      workspaceSnapshot({
        tabs: [workspaceTab("b"), workspaceTab("a")],
        tabOrder: ["b", "a"],
        activeTabId: "b",
        tabUpdate: { kind: "moved", tab_id: "b", target_index: 0 },
      }),
      { utilityTabs: ["settings"], currentTabOrder: ["a", "settings"] }
    );

    expect(result).toMatchObject({
      applied: true,
      tabOrder: ["b", "a", "settings"],
      activeTabId: "b",
    });
  });

  it("preserves an active utility tab during ordinary snapshots", () => {
    const result = reconcileSnapshot(workspaceSnapshot(), {
      utilityTabs: ["settings"],
      currentTabOrder: ["a", "settings"],
      currentActiveTabId: "settings",
    });

    expect(result).toMatchObject({ applied: true, activeTabId: "settings" });
  });

  it("falls back to the workspace active tab when the current tab disappears", () => {
    const result = reconcileSnapshot(workspaceSnapshot({ activeTabId: "a" }), {
      currentActiveTabId: "missing",
    });

    expect(result).toMatchObject({ applied: true, activeTabId: "a" });
  });

  it("clears selection when neither local nor workspace active tabs exist", () => {
    const result = reconcileSnapshot(
      workspaceSnapshot({ tabs: [], tabOrder: [], activeTabId: null }),
      { currentActiveTabId: "missing" }
    );

    expect(result).toMatchObject({ applied: true, activeTabId: null });
  });

  it("applies and consumes matching foreign placement metadata", () => {
    const result = reconcileSnapshot(
      workspaceSnapshot({
        tabs: [workspaceTab("a"), workspaceTab("moved"), workspaceTab("b")],
        tabOrder: ["a", "moved", "b"],
        activeTabId: "moved",
        tabUpdate: { kind: "moved", tab_id: "moved", target_index: 1 },
      }),
      {
        utilityTabs: ["settings"],
        currentTabOrder: ["a", "settings", "b"],
        foreignPlacement: {
          tabId: "moved",
          previousTabId: null,
          nextTabId: "b",
          visibleSlotIndex: 2,
        },
      }
    );

    expect(result).toMatchObject({
      applied: true,
      tabOrder: ["a", "settings", "moved", "b"],
      consumedForeignPlacement: true,
    });
  });
});
