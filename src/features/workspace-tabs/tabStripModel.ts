import type {
  AppTabInfo,
  ForeignTabPlacement,
  TabInfo,
  UtilityTabKind,
  WorkspaceSnapshot,
  WorkspaceTabInfo,
  WorkspaceTabUpdate,
} from "../../types";

export function orderAppTabs(appTabs: AppTabInfo[], tabOrder: string[]) {
  const tabsById = new Map(appTabs.map((tab) => [tab.id, tab]));
  const orderedTabs = tabOrder
    .map((id) => tabsById.get(id))
    .filter((tab): tab is AppTabInfo => Boolean(tab));
  const orderedIds = new Set(orderedTabs.map((tab) => tab.id));
  const newTabs = appTabs.filter((tab) => !orderedIds.has(tab.id));

  return [...orderedTabs, ...newTabs];
}

export function reorderTabIds(
  currentOrder: string[],
  draggedId: string,
  targetId: string,
  dropSide: "before" | "after"
) {
  const draggedIndex = currentOrder.indexOf(draggedId);
  const targetIndex = currentOrder.indexOf(targetId);
  if (draggedIndex < 0 || targetIndex < 0) return currentOrder;

  const nextOrder = [...currentOrder];
  const [draggedTabId] = nextOrder.splice(draggedIndex, 1);
  const targetIndexAfterRemoval = nextOrder.indexOf(targetId);
  const insertIndex = dropSide === "after" ? targetIndexAfterRemoval + 1 : targetIndexAfterRemoval;
  nextOrder.splice(insertIndex, 0, draggedTabId);
  return nextOrder;
}

export function tabOrdersEqual(left: string[], right: string[]) {
  if (left.length !== right.length) return false;

  const rightIds = right.values();
  return left.every((id) => id === rightIds.next().value);
}

export function applyForeignTabPlacement(
  order: string[],
  terminalOrder: string[],
  placement: ForeignTabPlacement
) {
  const withoutMovedTab = order.filter((id) => id !== placement.tabId);
  let insertIndex: number;
  const nextAnchorIndex = placement.nextTabId ? withoutMovedTab.indexOf(placement.nextTabId) : -1;
  const previousAnchorIndex = placement.previousTabId
    ? withoutMovedTab.indexOf(placement.previousTabId)
    : -1;

  if (nextAnchorIndex >= 0) {
    insertIndex = nextAnchorIndex;
  } else if (previousAnchorIndex >= 0) {
    insertIndex = previousAnchorIndex + 1;
  } else {
    insertIndex = Math.min(placement.visibleSlotIndex, withoutMovedTab.length);
  }

  const placedOrder = [
    ...withoutMovedTab.slice(0, insertIndex),
    placement.tabId,
    ...withoutMovedTab.slice(insertIndex),
  ];
  const terminalIds = new Set(terminalOrder);
  const placedTerminalOrder = placedOrder.filter((id) => terminalIds.has(id));
  return tabOrdersEqual(placedTerminalOrder, terminalOrder) ? placedOrder : order;
}

export function getRemovalFocusNeighbors(appTabs: AppTabInfo[], removedTabId: string) {
  const removedIndex = appTabs.findIndex((tab) => tab.id === removedTabId);
  if (removedIndex < 0) return { rightId: null, leftId: null };

  return {
    rightId: appTabs[removedIndex + 1]?.id ?? null,
    leftId: appTabs[removedIndex - 1]?.id ?? null,
  };
}

export function resolveRemovalFocusTarget(
  appTabs: AppTabInfo[],
  closingTabIds: Set<string>,
  removedTabId: string,
  rightId: string | null,
  leftId: string | null,
  activeTabId: string | null
) {
  const availableTabIds = new Set(
    appTabs
      .filter((tab) => tab.id !== removedTabId && !closingTabIds.has(tab.id))
      .map((tab) => tab.id)
  );

  if (rightId && availableTabIds.has(rightId)) return rightId;
  if (leftId && availableTabIds.has(leftId)) return leftId;

  return activeTabId && availableTabIds.has(activeTabId) ? activeTabId : null;
}

function insertTerminalAtWorkspaceIndex(
  order: string[],
  tabId: string,
  terminalOrder: string[],
  targetIndex: number
) {
  const withoutTab = order.filter((id) => id !== tabId);
  const followingTerminal = terminalOrder
    .slice(targetIndex + 1)
    .find((id) => withoutTab.includes(id));
  if (followingTerminal) {
    const insertIndex = withoutTab.indexOf(followingTerminal);
    return [...withoutTab.slice(0, insertIndex), tabId, ...withoutTab.slice(insertIndex)];
  }

  const precedingTerminal = terminalOrder
    .slice(0, targetIndex)
    .reverse()
    .find((id) => withoutTab.includes(id));
  if (precedingTerminal) {
    const insertIndex = withoutTab.indexOf(precedingTerminal) + 1;
    return [...withoutTab.slice(0, insertIndex), tabId, ...withoutTab.slice(insertIndex)];
  }

  return [...withoutTab, tabId];
}

export function reconcileTabOrder(
  currentOrder: string[],
  currentTerminalIds: string[],
  nextTerminalOrder: string[],
  utilityTabs: UtilityTabKind[],
  tabUpdate?: WorkspaceTabUpdate | null
) {
  const currentTerminalSet = new Set(currentTerminalIds);
  const nextTerminalSet = new Set(nextTerminalOrder);
  const utilityTabSet = new Set<string>(utilityTabs);
  let nextOrder = currentOrder.filter((id) => nextTerminalSet.has(id) || utilityTabSet.has(id));

  for (const utilityTab of utilityTabs) {
    if (!nextOrder.includes(utilityTab)) {
      nextOrder.push(utilityTab);
    }
  }

  const retainedTerminals = nextTerminalOrder.filter(
    (id) => currentTerminalSet.has(id) && nextOrder.includes(id)
  );
  nextOrder = nextOrder.map((id) => {
    if (!currentTerminalSet.has(id) || !nextTerminalSet.has(id)) return id;
    const replacement = retainedTerminals.shift();
    return replacement ?? id;
  });

  for (const tabId of nextTerminalOrder) {
    if (nextOrder.includes(tabId) || tabId === tabUpdate?.tab_id) continue;
    const targetIndex = nextTerminalOrder.indexOf(tabId);
    nextOrder = insertTerminalAtWorkspaceIndex(nextOrder, tabId, nextTerminalOrder, targetIndex);
  }

  if (tabUpdate?.kind === "connected" && nextTerminalSet.has(tabUpdate.tab_id)) {
    return [...nextOrder.filter((id) => id !== tabUpdate.tab_id), tabUpdate.tab_id];
  }

  if (tabUpdate?.kind === "moved" && nextTerminalSet.has(tabUpdate.tab_id)) {
    return insertTerminalAtWorkspaceIndex(
      nextOrder,
      tabUpdate.tab_id,
      nextTerminalOrder,
      tabUpdate.target_index
    );
  }

  return nextOrder;
}

export function workspaceTabToTabInfo(tab: WorkspaceTabInfo): TabInfo {
  return {
    id: tab.tab_id,
    kind: "terminal",
    title: tab.title,
    connectionType: tab.connection_type,
    sessionId: tab.session_id,
    isConnected: tab.is_connected,
    encoding: tab.encoding,
    terminalMode: tab.terminal_mode,
    connectionInfo: tab.connection_info ?? undefined,
    isAutoLogging: tab.is_auto_logging,
    isManualLogging: tab.is_manual_logging,
    isLoggingPaused: tab.is_logging_paused,
    manualLogFilePath: tab.manual_log_file_path ?? undefined,
  };
}

interface ReconcileWorkspaceSnapshotInput {
  expectedWindowId: string;
  lastAppliedRevision: number | null;
  currentTabs: TabInfo[];
  utilityTabs: UtilityTabKind[];
  currentTabOrder: string[];
  currentActiveTabId: string | null;
  foreignPlacement: ForeignTabPlacement | null;
  snapshot: WorkspaceSnapshot;
}

export type ReconcileWorkspaceSnapshotResult =
  | { applied: false }
  | {
      applied: true;
      revision: number;
      terminalTabs: TabInfo[];
      tabOrder: string[];
      appTabs: AppTabInfo[];
      activeTabId: string | null;
      consumedForeignPlacement: boolean;
    };

export function reconcileWorkspaceSnapshot({
  expectedWindowId,
  lastAppliedRevision,
  currentTabs,
  utilityTabs,
  currentTabOrder,
  currentActiveTabId,
  foreignPlacement,
  snapshot,
}: ReconcileWorkspaceSnapshotInput): ReconcileWorkspaceSnapshotResult {
  if (snapshot.window_id !== expectedWindowId) return { applied: false };
  if (lastAppliedRevision !== null && snapshot.revision <= lastAppliedRevision) {
    return { applied: false };
  }

  const terminalTabs = snapshot.tabs.map(workspaceTabToTabInfo);
  let tabOrder = reconcileTabOrder(
    currentTabOrder,
    currentTabs.map((tab) => tab.id),
    snapshot.window.tab_order,
    utilityTabs,
    snapshot.tab_update
  );
  const consumedForeignPlacement =
    snapshot.tab_update?.kind === "moved" && foreignPlacement?.tabId === snapshot.tab_update.tab_id;

  if (consumedForeignPlacement) {
    tabOrder = applyForeignTabPlacement(tabOrder, snapshot.window.tab_order, foreignPlacement);
  }

  const appTabs = orderAppTabs(
    [...terminalTabs, ...utilityTabs.map((kind) => ({ kind, id: kind }))],
    tabOrder
  );
  let activeTabId = currentActiveTabId;
  if (
    snapshot.tab_update?.kind === "connected" &&
    snapshot.tabs.some((tab) => tab.tab_id === snapshot.tab_update?.tab_id)
  ) {
    activeTabId = snapshot.tab_update.tab_id;
  } else if (
    snapshot.tab_update?.kind === "moved" &&
    snapshot.window.active_tab_id === snapshot.tab_update.tab_id &&
    snapshot.tabs.some((tab) => tab.tab_id === snapshot.tab_update?.tab_id)
  ) {
    activeTabId = snapshot.tab_update.tab_id;
  } else {
    const currentTabExists =
      snapshot.tabs.some((tab) => tab.tab_id === activeTabId) ||
      ((activeTabId === "settings" || activeTabId === "logs") && utilityTabs.includes(activeTabId));
    if (!currentTabExists) {
      const workspaceActiveTabId = snapshot.window.active_tab_id;
      activeTabId =
        workspaceActiveTabId && snapshot.tabs.some((tab) => tab.tab_id === workspaceActiveTabId)
          ? workspaceActiveTabId
          : null;
    }
  }

  return {
    applied: true,
    revision: snapshot.revision,
    terminalTabs,
    tabOrder,
    appTabs,
    activeTabId,
    consumedForeignPlacement,
  };
}
