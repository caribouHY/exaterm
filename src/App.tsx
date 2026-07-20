import { lazy, Suspense, useState, useRef, useCallback, useEffect, useMemo } from "react";
import TitleBar from "./components/TitleBar/TitleBar";
import TerminalTabs from "./components/Terminal/TerminalTabs";
import TerminalView from "./components/Terminal/TerminalView";
import type { TerminalViewHandle } from "./components/Terminal/TerminalView";
import StatusBar from "./components/StatusBar/StatusBar";
import type {
  AppTabInfo,
  TabInfo,
  ViewMode,
  UtilityTabKind,
  ConnectionType,
  Encoding,
  ForeignTabPlacement,
  AppConfig,
  ChatMessage,
  TerminalMode,
  StartupCliRequest,
  ManualLogWriteMode,
  WorkspaceDragDropResult,
  WorkspaceDragPreview,
  WorkspacePointerPosition,
  WorkspaceSnapshot,
  WorkspaceTabUpdate,
  WorkspaceTabInfo,
  WorkspaceWindowCreateResult,
  WorkspaceConnectionInfo,
} from "./types";
import type { ConnectionDialogInitialValues } from "./components/Connection/connectionDialogTypes";
import { DEFAULT_TERMINAL_MODE } from "./utils/terminalModes";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { save } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import {
  FeedbackMessage,
  ModalBody,
  ModalBusy,
  ModalDescription,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTarget,
  ModalTitle,
} from "./components/Common";
import "./App.css";

const loadConnectionDialog = () => import("./components/Connection/ConnectionDialog");
const loadAIChatPanel = () => import("./components/AI/AIChatPanel");
const loadSettingsPanel = () => import("./components/Settings/SettingsPanel");
const loadLogViewer = () => import("./components/Log/LogViewer");

const ConnectionDialog = lazy(loadConnectionDialog);
const AIChatPanel = lazy(loadAIChatPanel);
const SettingsPanel = lazy(loadSettingsPanel);
const LogViewer = lazy(loadLogViewer);

const AI_PANEL_DEFAULT_WIDTH = 340;
const AI_PANEL_MIN_WIDTH = 200;
const AI_PANEL_VIEWPORT_MARGIN = 40;
const FOREIGN_PLACEMENT_CLEAR_DELAY_MS = 250;

function clampAiPanelWidth(width: number, viewportWidth: number) {
  const maxWidth = Math.max(AI_PANEL_MIN_WIDTH, viewportWidth - AI_PANEL_VIEWPORT_MARGIN);
  return Math.min(Math.max(width, AI_PANEL_MIN_WIDTH), maxWidth);
}

interface McpCredentialRequestPayload {
  request_id: string;
  profile_id: string;
  host: string;
  port: number;
  username: string;
  auth_method: "password" | "public_key";
  target: string;
  title: string;
}

interface McpCredentialPromptState extends McpCredentialRequestPayload {
  value: string;
  error: string;
  submitting: boolean;
}

interface McpLogControlRequestPayload {
  request_id: string;
  session_id: string;
  connection_type: ConnectionType;
  target: string;
}

function orderAppTabs(appTabs: AppTabInfo[], tabOrder: string[]) {
  const tabsById = new Map(appTabs.map((tab) => [tab.id, tab]));
  const orderedTabs = tabOrder
    .map((id) => tabsById.get(id))
    .filter((tab): tab is AppTabInfo => Boolean(tab));
  const orderedIds = new Set(orderedTabs.map((tab) => tab.id));
  const newTabs = appTabs.filter((tab) => !orderedIds.has(tab.id));

  return [...orderedTabs, ...newTabs];
}

function reorderTabIds(
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

function tabOrdersEqual(left: string[], right: string[]) {
  return left.length === right.length && left.every((id, index) => id === right[index]);
}

function applyForeignTabPlacement(
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

function getRemovalFocusNeighbors(appTabs: AppTabInfo[], removedTabId: string) {
  const removedIndex = appTabs.findIndex((tab) => tab.id === removedTabId);
  if (removedIndex < 0) return { rightId: null, leftId: null };

  return {
    rightId: appTabs[removedIndex + 1]?.id ?? null,
    leftId: appTabs[removedIndex - 1]?.id ?? null,
  };
}

function resolveRemovalFocusTarget(
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

function reconcileTabOrder(
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

function workspaceTabToTabInfo(tab: WorkspaceTabInfo): TabInfo {
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

export default function App() {
  const { t } = useTranslation();
  const windowIdRef = useRef(getCurrentWindow().label || "main");
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [utilityTabs, setUtilityTabs] = useState<UtilityTabKind[]>([]);
  const [tabOrder, setTabOrder] = useState<string[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [closingTabIds, setClosingTabIds] = useState<string[]>([]);
  const [showConnection, setShowConnection] = useState(false);
  const [connectionInitialValues, setConnectionInitialValues] =
    useState<ConnectionDialogInitialValues | null>(null);
  const [showAiPanel, setShowAiPanel] = useState(false);
  const [aiPanelWidth, setAiPanelWidth] = useState(AI_PANEL_DEFAULT_WIDTH);
  const [isDragging, setIsDragging] = useState(false);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [manualLogBusyTabId, setManualLogBusyTabId] = useState<string | null>(null);
  const [logStatusMessage, setLogStatusMessage] = useState("");
  const [aiMessages, setAiMessages] = useState<ChatMessage[]>([]);
  const [aiSelectedProvider, setAiSelectedProvider] = useState("");
  const [aiSelectedModel, setAiSelectedModel] = useState("");
  const [startupCliRequest, setStartupCliRequest] = useState<StartupCliRequest | null>(null);
  const [mcpCredentialPrompts, setMcpCredentialPrompts] = useState<McpCredentialPromptState[]>([]);
  const [workspaceDragPreview, setWorkspaceDragPreview] = useState<WorkspaceDragPreview | null>(
    null
  );
  const activeTerminalBuffer = useRef("");
  const terminalBuffers = useRef<Map<string, string>>(new Map());
  const terminalViewRefs = useRef<Map<string, TerminalViewHandle>>(new Map());
  const tabsRef = useRef<TabInfo[]>([]);
  const utilityTabsRef = useRef<UtilityTabKind[]>([]);
  const tabOrderRef = useRef<string[]>([]);
  const activeTabIdRef = useRef<string | null>(null);
  const appTabsRef = useRef<AppTabInfo[]>([]);
  const closingTabIdsRef = useRef<Set<string>>(new Set());
  const selectionEpochRef = useRef(0);
  const lastAppliedWorkspaceRevisionRef = useRef<number | null>(null);
  const closeOperationsRef = useRef<Map<string, Promise<boolean>>>(new Map());
  const foreignTabPlacementRef = useRef<ForeignTabPlacement | null>(null);
  const activeForeignDragTabIdRef = useRef<string | null>(null);
  const foreignPlacementClearTimerRef = useRef<number | null>(null);

  const appTabs: AppTabInfo[] = useMemo(
    () =>
      orderAppTabs(
        [
          ...tabs,
          ...utilityTabs.map((kind) => ({
            kind,
            id: kind,
          })),
        ],
        tabOrder
      ),
    [tabs, utilityTabs, tabOrder]
  );
  const activeAppTab = appTabs.find((tab) => tab.id === activeTabId) || null;
  const activeTab =
    activeAppTab?.kind === "terminal" ? tabs.find((t) => t.id === activeAppTab.id) || null : null;
  const activeView: ViewMode =
    activeAppTab?.kind === "settings" || activeAppTab?.kind === "logs"
      ? activeAppTab.kind
      : "terminal";
  const activeMcpCredentialPrompt = mcpCredentialPrompts[0] ?? null;

  const setSelectedTab = useCallback((id: string | null, userInitiated: boolean) => {
    if (userInitiated && activeTabIdRef.current !== id) {
      selectionEpochRef.current += 1;
    }
    activeTabIdRef.current = id;
    setActiveTabId(id);
  }, []);

  const applyWorkspaceSnapshot = useCallback((snapshot: WorkspaceSnapshot) => {
    if (snapshot.window_id !== windowIdRef.current) return;
    const lastAppliedRevision = lastAppliedWorkspaceRevisionRef.current;
    if (lastAppliedRevision !== null && snapshot.revision <= lastAppliedRevision) return;
    lastAppliedWorkspaceRevisionRef.current = snapshot.revision;
    const terminalTabs = snapshot.tabs.map(workspaceTabToTabInfo);
    const currentTerminalIds = tabsRef.current.map((tab) => tab.id);
    tabsRef.current = terminalTabs;
    setTabs(terminalTabs);
    let nextTabOrder = reconcileTabOrder(
      tabOrderRef.current,
      currentTerminalIds,
      snapshot.window.tab_order,
      utilityTabsRef.current,
      snapshot.tab_update
    );
    const foreignPlacement = foreignTabPlacementRef.current;
    if (
      snapshot.tab_update?.kind === "moved" &&
      foreignPlacement?.tabId === snapshot.tab_update.tab_id
    ) {
      nextTabOrder = applyForeignTabPlacement(
        nextTabOrder,
        snapshot.window.tab_order,
        foreignPlacement
      );
      foreignTabPlacementRef.current = null;
      if (foreignPlacementClearTimerRef.current !== null) {
        window.clearTimeout(foreignPlacementClearTimerRef.current);
        foreignPlacementClearTimerRef.current = null;
      }
    }
    tabOrderRef.current = nextTabOrder;
    setTabOrder(nextTabOrder);
    appTabsRef.current = orderAppTabs(
      [...terminalTabs, ...utilityTabsRef.current.map((kind) => ({ kind, id: kind }))],
      nextTabOrder
    );

    let nextActiveTabId = activeTabIdRef.current;
    if (
      snapshot.tab_update?.kind === "connected" &&
      snapshot.tabs.some((tab) => tab.tab_id === snapshot.tab_update?.tab_id)
    ) {
      nextActiveTabId = snapshot.tab_update.tab_id;
    } else if (
      snapshot.tab_update?.kind === "moved" &&
      snapshot.window.active_tab_id === snapshot.tab_update.tab_id &&
      snapshot.tabs.some((tab) => tab.tab_id === snapshot.tab_update?.tab_id)
    ) {
      nextActiveTabId = snapshot.tab_update.tab_id;
    } else {
      const currentTabExists =
        snapshot.tabs.some((tab) => tab.tab_id === nextActiveTabId) ||
        ((nextActiveTabId === "settings" || nextActiveTabId === "logs") &&
          utilityTabsRef.current.includes(nextActiveTabId));
      if (!currentTabExists) {
        const workspaceActiveTabId = snapshot.window.active_tab_id;
        nextActiveTabId =
          workspaceActiveTabId && snapshot.tabs.some((tab) => tab.tab_id === workspaceActiveTabId)
            ? workspaceActiveTabId
            : null;
      }
    }
    activeTabIdRef.current = nextActiveTabId;
    setActiveTabId(nextActiveTabId);
  }, []);

  const updateWorkspaceTabMetadata = useCallback(
    async (tabId: string, patch: Record<string, unknown>) => {
      const snapshot = await invoke<WorkspaceSnapshot>("workspace_tab_update_metadata", {
        tabId,
        patch,
      });
      applyWorkspaceSnapshot(snapshot);
    },
    [applyWorkspaceSnapshot]
  );

  const handleConnect = useCallback(
    async (
      type: ConnectionType,
      sessionId: string,
      title: string,
      isAutoLogging: boolean,
      encoding: Encoding = "utf-8",
      terminalMode: TerminalMode = DEFAULT_TERMINAL_MODE,
      connectionInfo?: WorkspaceConnectionInfo
    ) => {
      const snapshot = await invoke<WorkspaceSnapshot>("workspace_tab_register", {
        windowId: windowIdRef.current,
        sessionId,
        connectionType: type,
        title,
        encoding,
        terminalMode,
        connectionInfo,
        isAutoLogging,
      });
      applyWorkspaceSnapshot(snapshot);
      setConnectionInitialValues(null);
      setShowConnection(false);
    },
    [applyWorkspaceSnapshot]
  );

  const openUtilityTab = useCallback(
    (kind: UtilityTabKind) => {
      const nextUtilityTabs = utilityTabsRef.current.includes(kind)
        ? utilityTabsRef.current
        : [...utilityTabsRef.current, kind];
      utilityTabsRef.current = nextUtilityTabs;
      setUtilityTabs(nextUtilityTabs);
      const nextTabOrder = tabOrderRef.current.includes(kind)
        ? tabOrderRef.current
        : [...tabOrderRef.current, kind];
      tabOrderRef.current = nextTabOrder;
      setTabOrder(nextTabOrder);
      appTabsRef.current = orderAppTabs(
        [...tabsRef.current, ...nextUtilityTabs.map((tabKind) => ({ kind: tabKind, id: tabKind }))],
        nextTabOrder
      );
      setSelectedTab(kind, true);
    },
    [setSelectedTab]
  );

  const handleViewChange = useCallback(
    (view: ViewMode) => {
      if (view === "settings" || view === "logs") {
        void (view === "settings" ? loadSettingsPanel() : loadLogViewer());
        openUtilityTab(view);
        return;
      }

      const currentIsTerminal = tabsRef.current.some((tab) => tab.id === activeTabIdRef.current);
      if (currentIsTerminal) return;
      setSelectedTab(
        tabsRef.current.length > 0 ? tabsRef.current[tabsRef.current.length - 1].id : null,
        true
      );
    },
    [openUtilityTab, setSelectedTab]
  );

  const handleSelectTab = useCallback(
    (id: string) => {
      const tab = appTabsRef.current.find((item) => item.id === id);
      setSelectedTab(id, true);
      if (tab?.kind === "terminal") {
        invoke<WorkspaceSnapshot>("workspace_tab_activate", {
          windowId: windowIdRef.current,
          tabId: id,
        })
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

      const tab = appTabsRef.current.find((item) => item.id === id);
      setSelectedTab(id, false);
      if (tab?.kind === "terminal") {
        invoke<WorkspaceSnapshot>("workspace_tab_activate", {
          windowId: windowIdRef.current,
          tabId: id,
        })
          .then(applyWorkspaceSnapshot)
          .catch((error) => {
            console.error("Failed to activate workspace tab:", error);
          });
      }
    },
    [applyWorkspaceSnapshot, setSelectedTab]
  );

  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);

  useEffect(() => {
    utilityTabsRef.current = utilityTabs;
  }, [utilityTabs]);

  useEffect(() => {
    tabOrderRef.current = tabOrder;
  }, [tabOrder]);

  useEffect(() => {
    appTabsRef.current = appTabs;
  }, [appTabs]);

  useEffect(() => {
    activeTabIdRef.current = activeTabId;
    activeTerminalBuffer.current = activeTabId
      ? terminalBuffers.current.get(activeTabId) || ""
      : "";
  }, [activeTabId]);

  useEffect(() => {
    const registerWindow = async () => {
      try {
        const snapshot = await invoke<WorkspaceSnapshot>("workspace_window_register", {
          windowId: windowIdRef.current,
          label: windowIdRef.current,
          focused: true,
        });
        applyWorkspaceSnapshot(snapshot);
      } catch (error) {
        console.error("Failed to register workspace window:", error);
      }
    };

    void registerWindow();

    const unlistenWorkspace = listen<WorkspaceSnapshot>("workspace://updated", (event) => {
      applyWorkspaceSnapshot(event.payload);
    });
    const unlistenWorkspaceDrag = listen<WorkspaceDragPreview>(
      "workspace://drag-preview",
      (event) => {
        const previewTabId = event.payload.active ? (event.payload.tab_id ?? null) : null;
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
        setWorkspaceDragPreview(event.payload.active ? event.payload : null);
      }
    );
    const handleFocus = () => {
      invoke<WorkspaceSnapshot>("workspace_window_focus", {
        windowId: windowIdRef.current,
      })
        .then(applyWorkspaceSnapshot)
        .catch((error) => {
          console.error("Failed to focus workspace window:", error);
        });
    };

    window.addEventListener("focus", handleFocus);
    return () => {
      unlistenWorkspace.then((fn) => fn());
      unlistenWorkspaceDrag.then((fn) => fn());
      if (foreignPlacementClearTimerRef.current !== null) {
        window.clearTimeout(foreignPlacementClearTimerRef.current);
      }
      window.removeEventListener("focus", handleFocus);
    };
  }, [applyWorkspaceSnapshot]);

  useEffect(() => {
    const unlistenCredential = listen<McpCredentialRequestPayload>(
      "external-control://credential-request",
      (event) => {
        setMcpCredentialPrompts((prev) => [
          ...prev,
          {
            ...event.payload,
            value: "",
            error: "",
            submitting: false,
          },
        ]);
      }
    );

    return () => {
      unlistenCredential.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const waitForUiUpdate = () =>
      new Promise<void>((resolve) => {
        window.requestAnimationFrame(() => {
          window.setTimeout(() => {
            resolve();
          }, 0);
        });
      });

    const submitLogControl = async (
      requestId: string,
      filePath: string | null,
      error: string | null
    ) => {
      try {
        await invoke("external_control_log_control_submit", {
          requestId,
          filePath,
          error,
        });
      } catch (submitError) {
        console.error("Failed to submit MCP log control response:", submitError);
      }
    };

    const unlistenStart = listen<McpLogControlRequestPayload>(
      "external-control://log-start-request",
      async (event) => {
        const payload = event.payload;
        const tab = tabsRef.current.find((item) => item.sessionId === payload.session_id);
        if (!tab) {
          await submitLogControl(payload.request_id, null, "セッションが見つかりません");
          return;
        }
        if (!tab.isConnected) {
          await submitLogControl(payload.request_id, null, "セッションは切断済みです");
          return;
        }
        if (tab.isManualLogging && tab.manualLogFilePath) {
          await submitLogControl(payload.request_id, tab.manualLogFilePath, null);
          return;
        }

        try {
          const filePath = await invoke<string>("logger_start_manual", {
            sessionId: payload.session_id,
            connectionType: payload.connection_type,
            target: payload.target,
            filePath: null,
            writeMode: "overwrite",
          });
          await updateWorkspaceTabMetadata(payload.session_id, {
            isManualLogging: true,
            isLoggingPaused: false,
            manualLogFilePath: filePath,
          });
          await waitForUiUpdate();
          await submitLogControl(payload.request_id, filePath, null);
        } catch (error) {
          console.error("Failed to start MCP manual log:", error);
          await submitLogControl(
            payload.request_id,
            null,
            typeof error === "string" ? error : "MCPログ開始に失敗しました"
          );
        }
      }
    );

    const unlistenStop = listen<McpLogControlRequestPayload>(
      "external-control://log-stop-request",
      async (event) => {
        const payload = event.payload;
        const tab = tabsRef.current.find((item) => item.sessionId === payload.session_id);
        if (!tab) {
          await submitLogControl(payload.request_id, null, "セッションが見つかりません");
          return;
        }
        if (!tab.isManualLogging) {
          await submitLogControl(payload.request_id, null, null);
          return;
        }

        try {
          await terminalViewRefs.current.get(tab.id)?.flushManualLogBuffer();
          await invoke("logger_stop_manual", { sessionId: payload.session_id });
          await updateWorkspaceTabMetadata(payload.session_id, {
            isManualLogging: false,
            isLoggingPaused: tab.isAutoLogging ? tab.isLoggingPaused : false,
          });
          await waitForUiUpdate();
          await submitLogControl(payload.request_id, null, null);
        } catch (error) {
          console.error("Failed to stop MCP manual log:", error);
          await submitLogControl(
            payload.request_id,
            null,
            typeof error === "string" ? error : "MCPログ停止に失敗しました"
          );
        }
      }
    );

    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStop.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (activeTabId && !appTabs.some((tab) => tab.id === activeTabId)) {
      setSelectedTab(appTabs.length > 0 ? appTabs[appTabs.length - 1].id : null, false);
    }
  }, [activeTabId, appTabs, setSelectedTab]);

  const removeTabFromState = useCallback((id: string) => {
    terminalBuffers.current.delete(id);
    if (activeTabIdRef.current === id) {
      activeTerminalBuffer.current = "";
    }
  }, []);

  const disconnectTab = useCallback(
    (id: string) => {
      const existingOperation = closeOperationsRef.current.get(id);
      if (existingOperation) {
        return existingOperation;
      }

      const operation = (async () => {
        const tab = tabsRef.current.find((item) => item.id === id);
        if (!tab) {
          return true;
        }

        closingTabIdsRef.current = new Set(closingTabIdsRef.current).add(id);
        setClosingTabIds(Array.from(closingTabIdsRef.current));

        if (!tab.sessionId) {
          removeTabFromState(id);
          return true;
        }

        const disconnectCommands: Record<ConnectionType, string> = {
          ssh: "ssh_disconnect",
          serial: "serial_disconnect",
          telnet: "telnet_disconnect",
        };
        const disconnectCommand = disconnectCommands[tab.connectionType];

        try {
          await invoke(disconnectCommand, { sessionId: tab.sessionId });
          const snapshot = await invoke<WorkspaceSnapshot>("workspace_tab_remove", {
            windowId: windowIdRef.current,
            tabId: id,
          });
          removeTabFromState(id);
          applyWorkspaceSnapshot(snapshot);
          return true;
        } catch (error) {
          console.error(
            `Failed to disconnect ${tab.connectionType} session ${tab.sessionId}:`,
            error
          );
          return false;
        } finally {
          closeOperationsRef.current.delete(id);
          const nextClosingTabIds = new Set(closingTabIdsRef.current);
          nextClosingTabIds.delete(id);
          closingTabIdsRef.current = nextClosingTabIds;
          setClosingTabIds(Array.from(nextClosingTabIds));
        }
      })();

      closeOperationsRef.current.set(id, operation);
      return operation;
    },
    [applyWorkspaceSnapshot, removeTabFromState]
  );

  const handleCloseTab = useCallback(
    async (id: string) => {
      const wasActive = activeTabIdRef.current === id;
      const selectionEpoch = selectionEpochRef.current;
      const { rightId, leftId } = getRemovalFocusNeighbors(appTabsRef.current, id);

      const focusAfterClose = () => {
        if (!wasActive || selectionEpochRef.current !== selectionEpoch) return;

        const focusTargetId = resolveRemovalFocusTarget(
          appTabsRef.current,
          closingTabIdsRef.current,
          id,
          rightId,
          leftId,
          activeTabIdRef.current
        );
        selectTabProgrammatically(focusTargetId);
      };

      if (id === "settings" || id === "logs") {
        const nextUtilityTabs = utilityTabsRef.current.filter((kind) => kind !== id);
        utilityTabsRef.current = nextUtilityTabs;
        setUtilityTabs(nextUtilityTabs);
        const nextTabOrder = tabOrderRef.current.filter((tabId) => tabId !== id);
        tabOrderRef.current = nextTabOrder;
        setTabOrder(nextTabOrder);
        appTabsRef.current = orderAppTabs(
          [...tabsRef.current, ...nextUtilityTabs.map((kind) => ({ kind, id: kind }))],
          nextTabOrder
        );
        focusAfterClose();
        return;
      }

      const closed = await disconnectTab(id);
      if (!closed) return;
      focusAfterClose();
    },
    [disconnectTab, selectTabProgrammatically]
  );

  const handleReorderTabs = useCallback(
    (draggedId: string, targetId: string, dropSide: "before" | "after") => {
      if (draggedId === targetId) return;

      const currentAppTabs = appTabsRef.current;
      const draggedTab = currentAppTabs.find((tab) => tab.id === draggedId);
      const targetTab = currentAppTabs.find((tab) => tab.id === targetId);
      if (!draggedTab || !targetTab) return;

      const visibleOrder = currentAppTabs.map((tab) => tab.id);
      const nextVisibleOrder = reorderTabIds(visibleOrder, draggedId, targetId, dropSide);
      if (tabOrdersEqual(visibleOrder, nextVisibleOrder)) return;

      const terminalTabIds = new Set(
        currentAppTabs.filter((tab) => tab.kind === "terminal").map((tab) => tab.id)
      );
      const currentTerminalOrder = visibleOrder.filter((id) => terminalTabIds.has(id));
      const nextTerminalOrder = nextVisibleOrder.filter((id) => terminalTabIds.has(id));

      if (!tabOrdersEqual(currentTerminalOrder, nextTerminalOrder)) {
        if (draggedTab.kind !== "terminal" || targetTab.kind !== "terminal") return;

        tabOrderRef.current = nextVisibleOrder;
        setTabOrder(nextVisibleOrder);
        appTabsRef.current = orderAppTabs(currentAppTabs, nextVisibleOrder);

        invoke<WorkspaceSnapshot>("workspace_tab_reorder", {
          windowId: windowIdRef.current,
          draggedTabId: draggedId,
          targetTabId: targetId,
          dropSide,
        })
          .then(applyWorkspaceSnapshot)
          .catch((error) => {
            console.error("Failed to reorder workspace tab:", error);
            if (!tabOrdersEqual(tabOrderRef.current, nextVisibleOrder)) return;

            tabOrderRef.current = visibleOrder;
            setTabOrder(visibleOrder);
            appTabsRef.current = orderAppTabs(currentAppTabs, visibleOrder);
          });
        return;
      }

      tabOrderRef.current = nextVisibleOrder;
      setTabOrder(nextVisibleOrder);
      appTabsRef.current = orderAppTabs(currentAppTabs, nextVisibleOrder);
    },
    [applyWorkspaceSnapshot]
  );

  const handleCrossWindowDragStart = useCallback(
    (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => {
      invoke<WorkspaceDragPreview>("workspace_tab_drag_start", {
        windowId: windowIdRef.current,
        tabId,
        pointerScreenPosition,
      })
        .then((preview) => {
          setWorkspaceDragPreview(preview.active ? preview : null);
        })
        .catch((error) => {
          console.error("Failed to start workspace tab drag:", error);
        });
    },
    []
  );

  const handleCrossWindowDragUpdate = useCallback(
    (pointerScreenPosition: WorkspacePointerPosition) => {
      invoke<WorkspaceDragPreview>("workspace_tab_drag_update", {
        pointerScreenPosition,
      }).catch((error) => {
        console.error("Failed to update workspace tab drag:", error);
      });
    },
    []
  );

  const handleCrossWindowDragHover = useCallback(
    (targetIndex: number | null, placement: ForeignTabPlacement | null) => {
      foreignTabPlacementRef.current = placement;
      invoke<WorkspaceDragPreview>("workspace_tab_drag_hover", {
        windowId: windowIdRef.current,
        targetIndex,
      }).catch((error) => {
        console.error("Failed to update workspace tab drag hover:", error);
      });
    },
    []
  );

  const handleCrossWindowDragCancel = useCallback(() => {
    invoke<WorkspaceDragPreview>("workspace_tab_drag_cancel")
      .then(() => {
        setWorkspaceDragPreview(null);
      })
      .catch((error) => {
        console.error("Failed to cancel workspace tab drag:", error);
      });
  }, []);

  const handleCrossWindowDragDrop = useCallback(
    async (tabId: string, pointerScreenPosition: WorkspacePointerPosition) => {
      const wasActive = activeTabIdRef.current === tabId;
      const selectionEpoch = selectionEpochRef.current;
      const { rightId, leftId } = getRemovalFocusNeighbors(appTabsRef.current, tabId);

      if (tabId) {
        try {
          await terminalViewRefs.current.get(tabId)?.flushLogBuffersForMove();
        } catch (error) {
          console.warn("Failed to flush terminal log buffers before tab move:", error);
        }
      }

      try {
        const result = await invoke<WorkspaceDragDropResult>("workspace_tab_drag_drop", {
          pointerScreenPosition,
        });
        result.snapshots.forEach(applyWorkspaceSnapshot);
        setWorkspaceDragPreview(null);
        const sourceSnapshot = result.snapshots.find(
          (snapshot) => snapshot.window_id === windowIdRef.current
        );
        const movedFromCurrentWindow =
          result.source_window_id === windowIdRef.current &&
          result.target_window_id !== windowIdRef.current;
        if (movedFromCurrentWindow && wasActive && selectionEpochRef.current === selectionEpoch) {
          selectTabProgrammatically(
            resolveRemovalFocusTarget(
              appTabsRef.current,
              closingTabIdsRef.current,
              tabId,
              rightId,
              leftId,
              activeTabIdRef.current
            )
          );
        }
        if (
          movedFromCurrentWindow &&
          sourceSnapshot?.window.tab_order.length === 0 &&
          utilityTabsRef.current.length === 0
        ) {
          await getCurrentWindow().close();
        }
      } catch (error) {
        console.error("Failed to drop workspace tab drag:", error);
        handleCrossWindowDragCancel();
      }
    },
    [applyWorkspaceSnapshot, handleCrossWindowDragCancel, selectTabProgrammatically]
  );

  const handleMoveTabToNewWindow = useCallback(
    async (tabId: string) => {
      const wasActive = activeTabIdRef.current === tabId;
      const selectionEpoch = selectionEpochRef.current;
      const { rightId, leftId } = getRemovalFocusNeighbors(appTabsRef.current, tabId);

      try {
        await terminalViewRefs.current.get(tabId)?.flushLogBuffersForMove();
      } catch (error) {
        console.warn("Failed to flush terminal log buffers before tab move:", error);
      }

      try {
        const result = await invoke<WorkspaceDragDropResult>("workspace_tab_detach_to_new_window", {
          tabId,
          fromWindowId: windowIdRef.current,
        });
        result.snapshots.forEach(applyWorkspaceSnapshot);
        const sourceSnapshot = result.snapshots.find(
          (snapshot) => snapshot.window_id === windowIdRef.current
        );
        const movedFromCurrentWindow =
          result.source_window_id === windowIdRef.current &&
          result.target_window_id !== windowIdRef.current;
        if (movedFromCurrentWindow && wasActive && selectionEpochRef.current === selectionEpoch) {
          selectTabProgrammatically(
            resolveRemovalFocusTarget(
              appTabsRef.current,
              closingTabIdsRef.current,
              tabId,
              rightId,
              leftId,
              activeTabIdRef.current
            )
          );
        }
        if (
          movedFromCurrentWindow &&
          sourceSnapshot?.window.tab_order.length === 0 &&
          utilityTabsRef.current.length === 0
        ) {
          await getCurrentWindow().close();
        }
      } catch (error) {
        console.error("Failed to move workspace tab to a new window:", error);
      }
    },
    [applyWorkspaceSnapshot, selectTabProgrammatically]
  );

  const handleTerminalData = useCallback((tabId: string, data: string) => {
    // Keep last 2000 chars per tab for AI context.
    const nextBuffer = ((terminalBuffers.current.get(tabId) || "") + data).slice(-2000);
    terminalBuffers.current.set(tabId, nextBuffer);
    if (activeTabIdRef.current === tabId) {
      activeTerminalBuffer.current = nextBuffer;
    }
  }, []);

  const updateActiveMcpCredentialPrompt = useCallback(
    (patch: Partial<McpCredentialPromptState>) => {
      setMcpCredentialPrompts((prev) => {
        if (prev.length === 0) return prev;
        return [{ ...prev[0], ...patch }, ...prev.slice(1)];
      });
    },
    []
  );

  const resolveMcpCredentialPrompt = useCallback(
    async (credential: string | null) => {
      if (!activeMcpCredentialPrompt || activeMcpCredentialPrompt.submitting) return;

      updateActiveMcpCredentialPrompt({ error: "", submitting: true });
      try {
        await invoke("external_control_credential_submit", {
          requestId: activeMcpCredentialPrompt.request_id,
          credential,
        });
        setMcpCredentialPrompts((prev) => prev.slice(1));
      } catch (error) {
        updateActiveMcpCredentialPrompt({
          error: typeof error === "string" ? error : t("mcp.credential_submit_failed"),
          submitting: false,
        });
      }
    },
    [activeMcpCredentialPrompt, t, updateActiveMcpCredentialPrompt]
  );

  const handleInsertCommand = useCallback(
    (command: string) => {
      if (!activeTab || !activeTab.isConnected) return;
      terminalViewRefs.current.get(activeTab.id)?.insertText(command);
    },
    [activeTab]
  );

  const handleEncodingChange = useCallback(
    (id: string, encoding: Encoding) => {
      invoke("terminal_encoding_set", { sessionId: id, encoding }).catch(console.error);
      updateWorkspaceTabMetadata(id, { encoding }).catch(console.error);
    },
    [updateWorkspaceTabMetadata]
  );

  const handleTerminalModeChange = useCallback(
    (id: string, terminalMode: TerminalMode) => {
      updateWorkspaceTabMetadata(id, { terminalMode }).catch(console.error);
    },
    [updateWorkspaceTabMetadata]
  );

  const buildManualLogFileName = useCallback((tab: TabInfo) => {
    const now = new Date();
    const stamp = [
      now.getFullYear(),
      String(now.getMonth() + 1).padStart(2, "0"),
      String(now.getDate()).padStart(2, "0"),
      "_",
      String(now.getHours()).padStart(2, "0"),
      String(now.getMinutes()).padStart(2, "0"),
      String(now.getSeconds()).padStart(2, "0"),
    ].join("");
    const sessionPrefix = (tab.sessionId ?? tab.id).slice(0, 8);
    return `exaterm_${stamp}_${sessionPrefix}.log`;
  }, []);

  const showTemporaryLogStatus = useCallback((message: string) => {
    setLogStatusMessage(message);
    window.setTimeout(() => {
      setLogStatusMessage("");
    }, 3000);
  }, []);

  const handleStartManualLog = useCallback(
    async (writeMode: ManualLogWriteMode) => {
      if (!activeTab?.sessionId || !activeTab.isConnected || activeTab.isManualLogging) return;

      setManualLogBusyTabId(activeTab.id);
      try {
        const selectedPath = await save({
          title: "Save ExaTerm Log",
          defaultPath: buildManualLogFileName(activeTab),
          filters: [{ name: "Log", extensions: ["log", "txt"] }],
        });
        if (!selectedPath) return;

        const filePath = await invoke<string>("logger_start_manual", {
          sessionId: activeTab.sessionId,
          connectionType: activeTab.connectionType,
          target: activeTab.title,
          filePath: selectedPath,
          writeMode,
        });
        await updateWorkspaceTabMetadata(activeTab.id, {
          isManualLogging: true,
          isLoggingPaused: false,
          manualLogFilePath: filePath,
        });
      } catch (error) {
        console.error("Failed to start manual log:", error);
        showTemporaryLogStatus("statusbar.log_start_failed");
      } finally {
        setManualLogBusyTabId(null);
      }
    },
    [activeTab, buildManualLogFileName, showTemporaryLogStatus, updateWorkspaceTabMetadata]
  );

  const handleStopManualLog = useCallback(async () => {
    if (!activeTab?.sessionId || !activeTab.isManualLogging) return;

    setManualLogBusyTabId(activeTab.id);
    try {
      await terminalViewRefs.current.get(activeTab.id)?.flushManualLogBuffer();
      await invoke("logger_stop_manual", { sessionId: activeTab.sessionId });
      await updateWorkspaceTabMetadata(activeTab.id, {
        isManualLogging: false,
        isLoggingPaused: activeTab.isAutoLogging ? activeTab.isLoggingPaused : false,
      });
    } catch (error) {
      console.error("Failed to stop manual log:", error);
      showTemporaryLogStatus("statusbar.log_stop_failed");
    } finally {
      setManualLogBusyTabId(null);
    }
  }, [activeTab, showTemporaryLogStatus, updateWorkspaceTabMetadata]);

  const handleSetLoggingPaused = useCallback(
    (paused: boolean) => {
      if (!activeTab?.isConnected || !(activeTab.isAutoLogging || activeTab.isManualLogging))
        return;
      updateWorkspaceTabMetadata(activeTab.id, { isLoggingPaused: paused }).catch(console.error);
    },
    [activeTab, updateWorkspaceTabMetadata]
  );

  const openConnection = useCallback(() => {
    void loadConnectionDialog();
    setConnectionInitialValues(null);
    setShowConnection(true);
  }, []);

  const openSameDestination = useCallback((tab: TabInfo) => {
    if (!tab.connectionInfo || tab.connectionType === "serial") return;
    void loadConnectionDialog();
    setConnectionInitialValues({
      connectionInfo: tab.connectionInfo,
      encoding: tab.encoding,
      terminalMode: tab.terminalMode,
    });
    setShowConnection(true);
  }, []);

  const openWindow = useCallback(() => {
    invoke<WorkspaceWindowCreateResult>("workspace_window_create").catch((error) => {
      console.error("Failed to create workspace window:", error);
    });
  }, []);

  const toggleAiPanel = useCallback(() => {
    setShowAiPanel((current) => {
      if (!current) {
        void loadAIChatPanel();
      }
      return !current;
    });
  }, []);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        const key = e.key.toLowerCase();
        if (key === "n" && e.shiftKey) {
          e.preventDefault();
          openWindow();
        } else if (key === "n" || key === "t") {
          e.preventDefault();
          openConnection();
        } else if (key === ",") {
          e.preventDefault();
          void loadSettingsPanel();
          openUtilityTab("settings");
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [openConnection, openUtilityTab, openWindow]);

  const refreshConfig = useCallback(async () => {
    try {
      const cfg = await invoke<AppConfig>("config_load");
      setConfig(cfg);
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }, []);

  useEffect(() => {
    void refreshConfig();
  }, [refreshConfig]);

  useEffect(() => {
    invoke<StartupCliRequest | null>("startup_cli_request_get")
      .then((request) => {
        if (!request) return;
        setStartupCliRequest(request);
        void loadConnectionDialog();
        setShowConnection(true);
      })
      .catch((error) => {
        console.error("Failed to load startup CLI request:", error);
      });
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  useEffect(() => {
    const handleResize = () => {
      setAiPanelWidth((width) => clampAiPanelWidth(width, window.innerWidth));
    };

    handleResize();
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
    };
  }, []);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      // AI panel is on the right, so width is (window width - mouse X)
      const newWidth = window.innerWidth - e.clientX;
      setAiPanelWidth(clampAiPanelWidth(newWidth, window.innerWidth));
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);
    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging]);

  return (
    <div className="app">
      <TitleBar
        activeView={activeView}
        showAiPanel={showAiPanel}
        onViewChange={handleViewChange}
        onOpenConnection={openConnection}
        onOpenWindow={openWindow}
        onToggleAiPanel={toggleAiPanel}
      />
      <div className="app__body">
        <div className="app__main">
          <div className="app__content">
            <div className="app__workspace">
              <TerminalTabs
                tabs={appTabs}
                activeTabId={activeTabId}
                closingTabIds={closingTabIds}
                onSelectTab={handleSelectTab}
                onCloseTab={handleCloseTab}
                onMoveTabToNewWindow={handleMoveTabToNewWindow}
                onOpenSameDestination={openSameDestination}
                onAddTab={openConnection}
                onReorderTabs={handleReorderTabs}
                windowId={windowIdRef.current}
                dragPreview={workspaceDragPreview}
                onCrossWindowDragStart={handleCrossWindowDragStart}
                onCrossWindowDragUpdate={handleCrossWindowDragUpdate}
                onCrossWindowDragDrop={handleCrossWindowDragDrop}
                onCrossWindowDragCancel={handleCrossWindowDragCancel}
                onCrossWindowDragHover={handleCrossWindowDragHover}
              />
              <div
                className={`app__terminal-area ${activeView !== "terminal" ? "app__hidden" : ""}`}
              >
                {tabs.length === 0 ? (
                  <TerminalView
                    sessionId={null}
                    connectionType="ssh"
                    isConnected={false}
                    isActive={activeView === "terminal"}
                    isAutoLogging={false}
                    isManualLogging={false}
                    isLoggingPaused={false}
                    onOpenConnection={openConnection}
                    onTerminalData={() => {}}
                    encoding="utf-8"
                    terminalConfig={config?.terminal}
                    terminalMode={DEFAULT_TERMINAL_MODE}
                  />
                ) : (
                  tabs.map((tab) => (
                    <TerminalView
                      key={tab.id}
                      ref={(handle) => {
                        if (handle) {
                          terminalViewRefs.current.set(tab.id, handle);
                        } else {
                          terminalViewRefs.current.delete(tab.id);
                        }
                      }}
                      sessionId={tab.sessionId || null}
                      connectionType={tab.connectionType}
                      isConnected={tab.isConnected}
                      isActive={activeView === "terminal" && tab.id === activeTabId}
                      isAutoLogging={Boolean(tab.isAutoLogging)}
                      isManualLogging={Boolean(tab.isManualLogging)}
                      isLoggingPaused={Boolean(tab.isLoggingPaused)}
                      onOpenConnection={openConnection}
                      onTerminalData={(data) => {
                        handleTerminalData(tab.id, data);
                      }}
                      encoding={tab.encoding}
                      terminalMode={tab.terminalMode}
                      terminalConfig={config?.terminal}
                    />
                  ))
                )}
              </div>
              {activeView === "settings" && (
                <Suspense fallback={<div aria-hidden="true" />}>
                  <SettingsPanel onSave={refreshConfig} />
                </Suspense>
              )}
              {activeView === "logs" && (
                <Suspense fallback={<div aria-hidden="true" />}>
                  <LogViewer />
                </Suspense>
              )}
            </div>
            {showAiPanel && (
              <>
                <div
                  className={`app__resizer ${isDragging ? "app__resizer--dragging" : ""}`}
                  onMouseDown={handleMouseDown}
                />
                <div
                  className="app__ai-panel"
                  style={{ width: clampAiPanelWidth(aiPanelWidth, window.innerWidth) }}
                >
                  <Suspense fallback={<div aria-hidden="true" />}>
                    <AIChatPanel
                      onClose={() => {
                        setShowAiPanel(false);
                      }}
                      config={config}
                      terminalBuffer={activeTerminalBuffer}
                      messages={aiMessages}
                      setMessages={setAiMessages}
                      selectedProvider={aiSelectedProvider}
                      setSelectedProvider={setAiSelectedProvider}
                      selectedModel={aiSelectedModel}
                      setSelectedModel={setAiSelectedModel}
                      onInsertCommand={handleInsertCommand}
                      canInsertCommand={Boolean(activeTab?.isConnected)}
                      activeTerminalMode={activeTab?.terminalMode ?? DEFAULT_TERMINAL_MODE}
                    />
                  </Suspense>
                </div>
              </>
            )}
          </div>
          <StatusBar
            activeTab={activeTab}
            showConnectionStatus={activeView === "terminal"}
            onEncodingChange={(encoding) =>
              activeTab && handleEncodingChange(activeTab.id, encoding)
            }
            onTerminalModeChange={(terminalMode) =>
              activeTab && handleTerminalModeChange(activeTab.id, terminalMode)
            }
            onStartManualLog={handleStartManualLog}
            onStopManualLog={handleStopManualLog}
            onSetLoggingPaused={handleSetLoggingPaused}
            manualLogBusy={Boolean(activeTab && manualLogBusyTabId === activeTab.id)}
            logStatusMessage={logStatusMessage}
          />
        </div>
      </div>
      {showConnection && (
        <Suspense fallback={<div aria-hidden="true" />}>
          <ConnectionDialog
            initialValues={connectionInitialValues}
            startupRequest={startupCliRequest}
            onStartupRequestHandled={() => {
              setStartupCliRequest(null);
            }}
            onClose={() => {
              setConnectionInitialValues(null);
              setShowConnection(false);
            }}
            onConnect={handleConnect}
          />
        </Suspense>
      )}
      {activeMcpCredentialPrompt && (
        <div className="app-credential-overlay">
          <ModalFrame
            className="app-credential-modal"
            role="dialog"
            ariaModal
            ariaLabelledBy="mcp-credential-title"
            ariaDescribedBy="mcp-credential-description"
          >
            <ModalHeader className="app-credential-modal__header">
              <div>
                <div className="app-credential-modal__eyebrow">
                  {t("mcp.credential_request_title")}
                </div>
                <ModalTitle className="app-credential-modal__title" id="mcp-credential-title">
                  {activeMcpCredentialPrompt.auth_method === "public_key"
                    ? t("connection.key_passphrase_prompt_title")
                    : t("connection.password_prompt_title")}
                </ModalTitle>
              </div>
            </ModalHeader>
            <ModalBody className="app-credential-modal__body">
              <ModalTarget className="app-credential-modal__target">
                {activeMcpCredentialPrompt.target}
              </ModalTarget>
              <ModalDescription
                className="app-credential-modal__description"
                id="mcp-credential-description"
              >
                {t("mcp.credential_request_desc")}
              </ModalDescription>
              <label className="label" htmlFor="mcp-credential-input">
                {activeMcpCredentialPrompt.auth_method === "public_key"
                  ? t("connection.key_passphrase")
                  : t("connection.password")}
              </label>
              <input
                id="mcp-credential-input"
                className="input"
                type="password"
                autoFocus
                value={activeMcpCredentialPrompt.value}
                disabled={activeMcpCredentialPrompt.submitting}
                onChange={(event) => {
                  updateActiveMcpCredentialPrompt({ value: event.target.value });
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    void resolveMcpCredentialPrompt(activeMcpCredentialPrompt.value);
                  } else if (event.key === "Escape") {
                    void resolveMcpCredentialPrompt(null);
                  }
                }}
              />
              {activeMcpCredentialPrompt.error && (
                <FeedbackMessage tone="error" className="app-credential-modal__error">
                  {activeMcpCredentialPrompt.error}
                </FeedbackMessage>
              )}
            </ModalBody>
            <ModalFooter className="app-credential-modal__footer">
              {activeMcpCredentialPrompt.submitting ? (
                <ModalBusy className="app-credential-modal__submitting">
                  {t("connection.connecting")}
                </ModalBusy>
              ) : (
                <>
                  <button
                    className="btn btn-ghost"
                    onClick={() => void resolveMcpCredentialPrompt(null)}
                  >
                    {t("connection.cancel")}
                  </button>
                  <button
                    className="btn btn-primary"
                    onClick={() => void resolveMcpCredentialPrompt(activeMcpCredentialPrompt.value)}
                  >
                    {t("connection.connect")}
                  </button>
                </>
              )}
            </ModalFooter>
          </ModalFrame>
        </div>
      )}
    </div>
  );
}
