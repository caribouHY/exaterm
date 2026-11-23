import { useCallback, useRef, useState } from "react";
import type {
  ConnectionType,
  Encoding,
  TerminalMode,
  UtilityTabKind,
  WorkspaceConnectionInfo,
} from "../../types";
import { isClosableDisconnectedTab } from "./tabStripModel";
import { workspaceClient } from "./workspaceClient";
import type { WindowTabsController } from "./useWindowTabs";

interface ConnectedTerminalInput {
  connectionType: ConnectionType;
  sessionId: string;
  title: string;
  isManualLogging: boolean;
  manualLogFilePath: string | null;
  encoding: Encoding;
  terminalMode: TerminalMode;
  connectionInfo?: WorkspaceConnectionInfo;
}

interface UseTerminalTabLifecycleOptions {
  tabs: Pick<
    WindowTabsController,
    | "windowId"
    | "getCurrentState"
    | "applyWorkspaceSnapshot"
    | "beginClosingTab"
    | "endClosingTab"
    | "captureRemovalFocus"
    | "restoreFocusAfterRemoval"
    | "closeUtilityTab"
  >;
  onTerminalRemoved: (tabId: string) => void;
}

export function useTerminalTabLifecycle({
  tabs,
  onTerminalRemoved,
}: UseTerminalTabLifecycleOptions) {
  const closeOperationsRef = useRef(new Map<string, Promise<boolean>>());
  const batchCloseInProgress = useRef(false);
  const [isClosingDisconnectedTabs, setIsClosingDisconnectedTabs] = useState(false);

  const registerTerminalTab = useCallback(
    async (input: ConnectedTerminalInput) => {
      const snapshot = await workspaceClient.registerTab({
        windowId: tabs.windowId,
        sessionId: input.sessionId,
        connectionType: input.connectionType,
        title: input.title,
        encoding: input.encoding,
        terminalMode: input.terminalMode,
        connectionInfo: input.connectionInfo,
        isManualLogging: input.isManualLogging,
        manualLogFilePath: input.manualLogFilePath,
      });
      tabs.applyWorkspaceSnapshot(snapshot);
    },
    [tabs]
  );

  const disconnectTab = useCallback(
    (tabId: string) => {
      const existingOperation = closeOperationsRef.current.get(tabId);
      if (existingOperation) return existingOperation;

      const operation = (async () => {
        const tab = tabs.getCurrentState().tabs.find((item) => item.id === tabId);
        if (!tab) return true;

        tabs.beginClosingTab(tabId);
        if (!tab.sessionId) {
          onTerminalRemoved(tabId);
          return true;
        }

        try {
          const snapshot = await workspaceClient.closeTab(tabId);
          onTerminalRemoved(tabId);
          tabs.applyWorkspaceSnapshot(snapshot);
          return true;
        } catch (error) {
          console.error(
            `Failed to disconnect ${tab.connectionType} session ${tab.sessionId}:`,
            error
          );
          return false;
        }
      })().finally(() => {
        closeOperationsRef.current.delete(tabId);
        tabs.endClosingTab(tabId);
      });

      closeOperationsRef.current.set(tabId, operation);
      return operation;
    },
    [onTerminalRemoved, tabs]
  );

  const closeTab = useCallback(
    async (tabId: string) => {
      const focusContext = tabs.captureRemovalFocus(tabId);
      if (tabId === "settings" || tabId === "logs") {
        tabs.closeUtilityTab(tabId as UtilityTabKind);
        tabs.restoreFocusAfterRemoval(focusContext);
        return;
      }

      if (await disconnectTab(tabId)) {
        tabs.restoreFocusAfterRemoval(focusContext);
      }
    },
    [disconnectTab, tabs]
  );

  const closeDisconnectedTabs = useCallback(async () => {
    if (batchCloseInProgress.current) return;

    const { appTabs, activeTabId, closingTabIds } = tabs.getCurrentState();
    const targets = appTabs.filter((tab) => isClosableDisconnectedTab(tab, closingTabIds));
    const targetIds = targets.filter((tab) => tab.id !== activeTabId).map((tab) => tab.id);
    if (activeTabId && targets.some((tab) => tab.id === activeTabId)) {
      targetIds.push(activeTabId);
    }

    if (targetIds.length === 0) return;
    batchCloseInProgress.current = true;
    setIsClosingDisconnectedTabs(true);
    try {
      for (const tabId of targetIds) {
        const current = tabs.getCurrentState();
        const tab = current.tabs.find((item) => item.id === tabId);
        if (tab && isClosableDisconnectedTab(tab, current.closingTabIds)) {
          await closeTab(tabId);
        }
      }
    } finally {
      batchCloseInProgress.current = false;
      setIsClosingDisconnectedTabs(false);
    }
  }, [closeTab, tabs]);

  return { registerTerminalTab, closeTab, closeDisconnectedTabs, isClosingDisconnectedTabs };
}
