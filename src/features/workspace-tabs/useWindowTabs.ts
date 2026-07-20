import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  AppTabInfo,
  ForeignTabPlacement,
  TabInfo,
  UtilityTabKind,
  ViewMode,
  WorkspaceDragPreview,
  WorkspaceSnapshot,
} from "../../types";
import {
  getRemovalFocusNeighbors,
  orderAppTabs,
  reconcileWorkspaceSnapshot,
  resolveRemovalFocusTarget,
  tabOrdersEqual,
} from "./tabStripModel";
import { workspaceClient, type WorkspaceTabMetadataPatch } from "./workspaceClient";

const FOREIGN_PLACEMENT_CLEAR_DELAY_MS = 250;

interface WindowTabsState {
  tabs: TabInfo[];
  utilityTabs: UtilityTabKind[];
  tabOrder: string[];
  appTabs: AppTabInfo[];
  activeTabId: string | null;
  closingTabIds: string[];
  dragPreview: WorkspaceDragPreview | null;
}

export interface RemovalFocusContext {
  tabId: string;
  wasActive: boolean;
  selectionEpoch: number;
  rightId: string | null;
  leftId: string | null;
}

const initialState: WindowTabsState = {
  tabs: [],
  utilityTabs: [],
  tabOrder: [],
  appTabs: [],
  activeTabId: null,
  closingTabIds: [],
  dragPreview: null,
};

export function useWindowTabs() {
  const windowIdRef = useRef(getCurrentWindow().label || "main");
  const [state, setState] = useState<WindowTabsState>(initialState);
  const stateRef = useRef(state);
  const selectionEpochRef = useRef(0);
  const lastAppliedWorkspaceRevisionRef = useRef<number | null>(null);
  const foreignTabPlacementRef = useRef<ForeignTabPlacement | null>(null);
  const activeForeignDragTabIdRef = useRef<string | null>(null);
  const foreignPlacementClearTimerRef = useRef<number | null>(null);

  const commitState = useCallback((nextState: WindowTabsState) => {
    stateRef.current = nextState;
    setState(nextState);
  }, []);

  const setSelectedTab = useCallback(
    (id: string | null, userInitiated: boolean) => {
      const current = stateRef.current;
      if (userInitiated && current.activeTabId !== id) {
        selectionEpochRef.current += 1;
      }
      if (current.activeTabId === id) return;
      commitState({ ...current, activeTabId: id });
    },
    [commitState]
  );

  const applyWorkspaceSnapshot = useCallback(
    (snapshot: WorkspaceSnapshot) => {
      const current = stateRef.current;
      const result = reconcileWorkspaceSnapshot({
        expectedWindowId: windowIdRef.current,
        lastAppliedRevision: lastAppliedWorkspaceRevisionRef.current,
        currentTabs: current.tabs,
        utilityTabs: current.utilityTabs,
        currentTabOrder: current.tabOrder,
        currentActiveTabId: current.activeTabId,
        foreignPlacement: foreignTabPlacementRef.current,
        snapshot,
      });
      if (!result.applied) return;

      lastAppliedWorkspaceRevisionRef.current = result.revision;
      if (result.consumedForeignPlacement) {
        foreignTabPlacementRef.current = null;
        if (foreignPlacementClearTimerRef.current !== null) {
          window.clearTimeout(foreignPlacementClearTimerRef.current);
          foreignPlacementClearTimerRef.current = null;
        }
      }
      commitState({
        ...stateRef.current,
        tabs: result.terminalTabs,
        tabOrder: result.tabOrder,
        appTabs: result.appTabs,
        activeTabId: result.activeTabId,
      });
    },
    [commitState]
  );

  const selectTab = useCallback(
    (id: string) => {
      const tab = stateRef.current.appTabs.find((item) => item.id === id);
      setSelectedTab(id, true);
      if (tab?.kind === "terminal") {
        workspaceClient
          .activateTab(windowIdRef.current, id)
          .then(applyWorkspaceSnapshot)
          .catch((error) => {
            console.error("Failed to activate workspace tab:", error);
          });
      }
    },
    [applyWorkspaceSnapshot, setSelectedTab]
  );

  const selectTabProgrammatically = useCallback(
    (id: string | null) => {
      if (!id) {
        setSelectedTab(null, false);
        return;
      }

      const tab = stateRef.current.appTabs.find((item) => item.id === id);
      setSelectedTab(id, false);
      if (tab?.kind === "terminal") {
        workspaceClient
          .activateTab(windowIdRef.current, id)
          .then(applyWorkspaceSnapshot)
          .catch((error) => {
            console.error("Failed to activate workspace tab:", error);
          });
      }
    },
    [applyWorkspaceSnapshot, setSelectedTab]
  );

  const openUtilityTab = useCallback(
    (kind: UtilityTabKind) => {
      const current = stateRef.current;
      const utilityTabs = current.utilityTabs.includes(kind)
        ? current.utilityTabs
        : [...current.utilityTabs, kind];
      const tabOrder = current.tabOrder.includes(kind)
        ? current.tabOrder
        : [...current.tabOrder, kind];
      const appTabs = orderAppTabs(
        [...current.tabs, ...utilityTabs.map((tabKind) => ({ kind: tabKind, id: tabKind }))],
        tabOrder
      );
      if (current.activeTabId !== kind) {
        selectionEpochRef.current += 1;
      }
      commitState({ ...current, utilityTabs, tabOrder, appTabs, activeTabId: kind });
    },
    [commitState]
  );

  const showTerminalView = useCallback(() => {
    const current = stateRef.current;
    if (current.tabs.some((tab) => tab.id === current.activeTabId)) return;
    setSelectedTab(current.tabs[current.tabs.length - 1]?.id ?? null, true);
  }, [setSelectedTab]);

  const captureRemovalFocus = useCallback((tabId: string): RemovalFocusContext => {
    const current = stateRef.current;
    const { rightId, leftId } = getRemovalFocusNeighbors(current.appTabs, tabId);
    return {
      tabId,
      wasActive: current.activeTabId === tabId,
      selectionEpoch: selectionEpochRef.current,
      rightId,
      leftId,
    };
  }, []);

  const restoreFocusAfterRemoval = useCallback(
    (context: RemovalFocusContext) => {
      if (!context.wasActive || selectionEpochRef.current !== context.selectionEpoch) return;
      const current = stateRef.current;
      selectTabProgrammatically(
        resolveRemovalFocusTarget(
          current.appTabs,
          new Set(current.closingTabIds),
          context.tabId,
          context.rightId,
          context.leftId,
          current.activeTabId
        )
      );
    },
    [selectTabProgrammatically]
  );

  const closeUtilityTab = useCallback(
    (kind: UtilityTabKind) => {
      const current = stateRef.current;
      const utilityTabs = current.utilityTabs.filter((item) => item !== kind);
      const tabOrder = current.tabOrder.filter((id) => id !== kind);
      const appTabs = orderAppTabs(
        [...current.tabs, ...utilityTabs.map((tabKind) => ({ kind: tabKind, id: tabKind }))],
        tabOrder
      );
      commitState({ ...current, utilityTabs, tabOrder, appTabs });
    },
    [commitState]
  );

  const beginClosingTab = useCallback(
    (tabId: string) => {
      const current = stateRef.current;
      if (current.closingTabIds.includes(tabId)) return;
      commitState({ ...current, closingTabIds: [...current.closingTabIds, tabId] });
    },
    [commitState]
  );

  const endClosingTab = useCallback(
    (tabId: string) => {
      const current = stateRef.current;
      if (!current.closingTabIds.includes(tabId)) return;
      commitState({
        ...current,
        closingTabIds: current.closingTabIds.filter((id) => id !== tabId),
      });
    },
    [commitState]
  );

  const setVisibleTabOrder = useCallback(
    (tabOrder: string[]) => {
      const current = stateRef.current;
      commitState({
        ...current,
        tabOrder,
        appTabs: orderAppTabs(current.appTabs, tabOrder),
      });
    },
    [commitState]
  );

  const rollbackVisibleTabOrder = useCallback(
    (expectedOrder: string[], previousOrder: string[]) => {
      const current = stateRef.current;
      if (!tabOrdersEqual(current.tabOrder, expectedOrder)) return;
      commitState({
        ...current,
        tabOrder: previousOrder,
        appTabs: orderAppTabs(current.appTabs, previousOrder),
      });
    },
    [commitState]
  );

  const setForeignTabPlacement = useCallback((placement: ForeignTabPlacement | null) => {
    foreignTabPlacementRef.current = placement;
  }, []);

  const setDragPreview = useCallback(
    (preview: WorkspaceDragPreview | null) => {
      const current = stateRef.current;
      if (current.dragPreview === preview) return;
      commitState({ ...current, dragPreview: preview });
    },
    [commitState]
  );

  const updateTabMetadata = useCallback(
    async (tabId: string, patch: WorkspaceTabMetadataPatch) => {
      const snapshot = await workspaceClient.updateTabMetadata(tabId, patch);
      applyWorkspaceSnapshot(snapshot);
    },
    [applyWorkspaceSnapshot]
  );

  const createWorkspaceWindow = useCallback(() => {
    workspaceClient.createWindow().catch((error) => {
      console.error("Failed to create workspace window:", error);
    });
  }, []);

  const getCurrentState = useCallback(() => stateRef.current, []);

  useEffect(() => {
    const registerWindow = async () => {
      try {
        applyWorkspaceSnapshot(await workspaceClient.registerWindow(windowIdRef.current));
      } catch (error) {
        console.error("Failed to register workspace window:", error);
      }
    };

    void registerWindow();

    const unlistenWorkspace = workspaceClient.listenWorkspaceUpdated(applyWorkspaceSnapshot);
    const unlistenWorkspaceDrag = workspaceClient.listenDragPreview((preview) => {
      const previewTabId = preview.active ? (preview.tab_id ?? null) : null;
      if (previewTabId) {
        if (activeForeignDragTabIdRef.current !== previewTabId) {
          foreignTabPlacementRef.current = null;
        }
        activeForeignDragTabIdRef.current = previewTabId;
        if (foreignPlacementClearTimerRef.current !== null) {
          window.clearTimeout(foreignPlacementClearTimerRef.current);
          foreignPlacementClearTimerRef.current = null;
        }
      } else if (activeForeignDragTabIdRef.current) {
        const completedDragTabId = activeForeignDragTabIdRef.current;
        activeForeignDragTabIdRef.current = null;
        const placement = foreignTabPlacementRef.current;
        if (placement?.tabId === completedDragTabId) {
          foreignPlacementClearTimerRef.current = window.setTimeout(() => {
            if (foreignTabPlacementRef.current === placement) {
              foreignTabPlacementRef.current = null;
            }
            foreignPlacementClearTimerRef.current = null;
          }, FOREIGN_PLACEMENT_CLEAR_DELAY_MS);
        }
      }
      setDragPreview(preview.active ? preview : null);
    });
    const handleFocus = () => {
      workspaceClient
        .focusWindow(windowIdRef.current)
        .then(applyWorkspaceSnapshot)
        .catch((error) => {
          console.error("Failed to focus workspace window:", error);
        });
    };

    window.addEventListener("focus", handleFocus);
    return () => {
      unlistenWorkspace.then((unlisten) => unlisten());
      unlistenWorkspaceDrag.then((unlisten) => unlisten());
      if (foreignPlacementClearTimerRef.current !== null) {
        window.clearTimeout(foreignPlacementClearTimerRef.current);
      }
      window.removeEventListener("focus", handleFocus);
    };
  }, [applyWorkspaceSnapshot, setDragPreview]);

  useEffect(() => {
    if (state.activeTabId && !state.appTabs.some((tab) => tab.id === state.activeTabId)) {
      setSelectedTab(state.appTabs[state.appTabs.length - 1]?.id ?? null, false);
    }
  }, [setSelectedTab, state.activeTabId, state.appTabs]);

  const activeAppTab = useMemo(
    () => state.appTabs.find((tab) => tab.id === state.activeTabId) ?? null,
    [state.activeTabId, state.appTabs]
  );
  const activeTab = useMemo(
    () =>
      activeAppTab?.kind === "terminal"
        ? (state.tabs.find((tab) => tab.id === activeAppTab.id) ?? null)
        : null,
    [activeAppTab, state.tabs]
  );
  const activeView: ViewMode =
    activeAppTab?.kind === "settings" || activeAppTab?.kind === "logs"
      ? activeAppTab.kind
      : "terminal";

  return {
    windowId: windowIdRef.current,
    tabs: state.tabs,
    utilityTabs: state.utilityTabs,
    tabOrder: state.tabOrder,
    appTabs: state.appTabs,
    activeTabId: state.activeTabId,
    activeTab,
    activeView,
    closingTabIds: state.closingTabIds,
    dragPreview: state.dragPreview,
    applyWorkspaceSnapshot,
    selectTab,
    selectTabProgrammatically,
    openUtilityTab,
    showTerminalView,
    captureRemovalFocus,
    restoreFocusAfterRemoval,
    closeUtilityTab,
    beginClosingTab,
    endClosingTab,
    setVisibleTabOrder,
    rollbackVisibleTabOrder,
    setForeignTabPlacement,
    setDragPreview,
    updateTabMetadata,
    createWorkspaceWindow,
    getCurrentState,
  };
}

export type WindowTabsController = ReturnType<typeof useWindowTabs>;
