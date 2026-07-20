import { useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ConnectionType,
  Encoding,
  TerminalMode,
  UtilityTabKind,
  WorkspaceConnectionInfo,
} from "../../types";
import { workspaceClient } from "./workspaceClient";
import type { WindowTabsController } from "./useWindowTabs";

interface ConnectedTerminalInput {
  connectionType: ConnectionType;
  sessionId: string;
  title: string;
  isAutoLogging: boolean;
  encoding: Encoding;
  terminalMode: TerminalMode;
  connectionInfo?: WorkspaceConnectionInfo;
}

interface UseTerminalTabLifecycleOptions {
  tabs: WindowTabsController;
  onTerminalRemoved: (tabId: string) => void;
}

const disconnectCommands: Record<ConnectionType, string> = {
  ssh: "ssh_disconnect",
  serial: "serial_disconnect",
  telnet: "telnet_disconnect",
};

export function useTerminalTabLifecycle({
  tabs,
  onTerminalRemoved,
}: UseTerminalTabLifecycleOptions) {
  const closeOperationsRef = useRef(new Map<string, Promise<boolean>>());

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
        isAutoLogging: input.isAutoLogging,
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
          await invoke(disconnectCommands[tab.connectionType], { sessionId: tab.sessionId });
          const snapshot = await workspaceClient.removeTab(tabs.windowId, tabId);
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

  return { registerTerminalTab, closeTab };
}
