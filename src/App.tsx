import { lazy, Suspense, useState, useRef, useCallback, useEffect } from "react";
import TitleBar from "./components/TitleBar/TitleBar";
import TerminalTabs from "./components/Terminal/TerminalTabs";
import TerminalView from "./components/Terminal/TerminalView";
import type { TerminalViewHandle } from "./components/Terminal/TerminalView";
import StatusBar from "./components/StatusBar/StatusBar";
import StatusBarPalette from "./components/StatusBar/StatusBarPalette";
import type { StatusBarPaletteCloseReason } from "./components/StatusBar/StatusBarPalette";
import type { StatusBarMenuKind } from "./components/StatusBar/statusBarMenuModel";
import type {
  TabInfo,
  ViewMode,
  ConnectionType,
  Encoding,
  AppConfig,
  ChatMessage,
  TerminalMode,
  StartupCliRequest,
  SshAuthMethod,
  ManualLogWriteMode,
  WorkspaceConnectionInfo,
} from "./types";
import type { ConnectionDialogInitialValues } from "./components/Connection/connectionDialogTypes";
import type { ConnectionLogState } from "./features/terminal-logging/connectionLogModel";
import { DEFAULT_TERMINAL_MODE } from "./utils/terminalModes";
import { invoke } from "@tauri-apps/api/core";
import {
  backendCommandErrorMessage,
  translateBackendCommandError,
} from "./features/backend-errors/backendCommandError";
import { listen } from "@tauri-apps/api/event";
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
import { useWindowTabs } from "./features/workspace-tabs/useWindowTabs";
import { useTerminalTabLifecycle } from "./features/workspace-tabs/useTerminalTabLifecycle";
import { useWorkspaceTabMovement } from "./features/workspace-tabs/useWorkspaceTabMovement";
import { DEFAULT_SHORTCUT_CONFIG, findShortcutAction } from "./features/shortcuts/shortcutModel";
import type { TerminalLogShortcutAction } from "./features/shortcuts/shortcutModel";
import {
  canPauseManualLog,
  canResumeManualLog,
} from "./features/terminal-logging/terminalLoggingModel";
import { AppUpdateDialog } from "./features/app-update/AppUpdateDialog";
import { useAppUpdate } from "./features/app-update/useAppUpdate";
import { AppExitDialog } from "./features/app-exit/AppExitDialog";
import { useAppExit } from "./features/app-exit/useAppExit";
import { SshAuthenticationPromptDialog } from "./features/ssh-authentication/SshAuthenticationPromptDialog";
import { SshHostKeyPromptDialog } from "./features/ssh-authentication/SshHostKeyPromptDialog";
import { useSshPrompts } from "./features/ssh-authentication/useSshPrompts";
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
  auth_method: SshAuthMethod;
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

export default function App() {
  const { t } = useTranslation();
  const windowTabs = useWindowTabs();
  const sshPrompts = useSshPrompts();
  const {
    tabs,
    appTabs,
    activeTabId,
    activeTab,
    activeView,
    closingTabIds,
    dragPreview: workspaceDragPreview,
  } = windowTabs;
  const getCurrentWindowTabsState = windowTabs.getCurrentState;
  const openUtilityTab = windowTabs.openUtilityTab;
  const updateWorkspaceTabMetadata = windowTabs.updateTabMetadata;
  const [showConnection, setShowConnection] = useState(false);
  const [connectionInitialValues, setConnectionInitialValues] =
    useState<ConnectionDialogInitialValues | null>(null);
  const [showAiPanel, setShowAiPanel] = useState(false);
  const [aiPanelWidth, setAiPanelWidth] = useState(AI_PANEL_DEFAULT_WIDTH);
  const [isDragging, setIsDragging] = useState(false);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [manualLogBusyTabId, setManualLogBusyTabId] = useState<string | null>(null);
  const [logStatusMessage, setLogStatusMessage] = useState("");
  const showTemporaryLogStatus = useCallback((message: string) => {
    setLogStatusMessage(message);
    window.setTimeout(() => {
      setLogStatusMessage("");
    }, 3000);
  }, []);
  const [openStatusBarMenu, setOpenStatusBarMenu] = useState<StatusBarMenuKind | null>(null);
  const [aiMessages, setAiMessages] = useState<ChatMessage[]>([]);
  const [aiSelectedProvider, setAiSelectedProvider] = useState("");
  const [aiSelectedModel, setAiSelectedModel] = useState("");
  const [startupCliRequest, setStartupCliRequest] = useState<StartupCliRequest | null>(null);
  const [mcpCredentialPrompts, setMcpCredentialPrompts] = useState<McpCredentialPromptState[]>([]);
  const [terminalSelectionByTab, setTerminalSelectionByTab] = useState<
    ReadonlyMap<string, boolean>
  >(new Map());
  const activeTerminalBuffer = useRef("");
  const terminalBuffers = useRef<Map<string, string>>(new Map());
  const terminalViewRefs = useRef<Map<string, TerminalViewHandle>>(new Map());
  const restoreTerminalFocusAfterPaletteRef = useRef(false);
  const activeMcpCredentialPrompt = mcpCredentialPrompts[0] ?? null;
  const shortcuts = config?.shortcuts ?? DEFAULT_SHORTCUT_CONFIG;
  const appUpdate = useAppUpdate({
    windowId: windowTabs.windowId,
    checkOnStartup: config ? config.updates.check_on_startup : null,
  });
  const appExit = useAppExit();

  const handleStatusBarMenuToggle = useCallback(
    (kind: StatusBarMenuKind, pointerActivated: boolean) => {
      if (!pointerActivated) {
        restoreTerminalFocusAfterPaletteRef.current = false;
      }
      setOpenStatusBarMenu((current) => {
        if (current === kind) {
          restoreTerminalFocusAfterPaletteRef.current = false;
          return null;
        }
        return kind;
      });
    },
    []
  );

  const handleStatusBarMenuTriggerPointerDown = useCallback(() => {
    restoreTerminalFocusAfterPaletteRef.current = Boolean(
      activeTabId && terminalViewRefs.current.get(activeTabId)?.isFocused()
    );
  }, [activeTabId]);

  const handleStatusBarPaletteClose = useCallback(
    (reason: StatusBarPaletteCloseReason) => {
      const menuToRestore = openStatusBarMenu;
      const shouldRestoreTerminalFocus = restoreTerminalFocusAfterPaletteRef.current;
      restoreTerminalFocusAfterPaletteRef.current = false;
      if (reason === "tab" && menuToRestore) {
        document.getElementById(`statusbar-menu-trigger-${menuToRestore}`)?.focus();
      }
      setOpenStatusBarMenu(null);
      if (reason === "confirm" || reason === "escape") {
        window.requestAnimationFrame(() => {
          if (shouldRestoreTerminalFocus && activeTabId) {
            terminalViewRefs.current.get(activeTabId)?.focus();
          } else if (menuToRestore) {
            document.getElementById(`statusbar-menu-trigger-${menuToRestore}`)?.focus();
          }
        });
      } else if (reason === "action" && menuToRestore) {
        window.requestAnimationFrame(() => {
          document.getElementById(`statusbar-menu-trigger-${menuToRestore}`)?.focus();
        });
      }
    },
    [activeTabId, openStatusBarMenu]
  );

  useEffect(() => {
    restoreTerminalFocusAfterPaletteRef.current = false;
    setOpenStatusBarMenu(null);
  }, [activeTabId, activeView]);

  useEffect(() => {
    if (
      openStatusBarMenu === "log" &&
      (!activeTab?.isConnected || manualLogBusyTabId === activeTab.id)
    ) {
      setOpenStatusBarMenu(null);
    }
  }, [activeTab, manualLogBusyTabId, openStatusBarMenu]);

  const removeTerminalFromState = useCallback(
    (tabId: string) => {
      terminalBuffers.current.delete(tabId);
      setTerminalSelectionByTab((current) => {
        if (!current.has(tabId)) return current;
        const next = new Map(current);
        next.delete(tabId);
        return next;
      });
      if (getCurrentWindowTabsState().activeTabId === tabId) {
        activeTerminalBuffer.current = "";
      }
    },
    [getCurrentWindowTabsState]
  );

  const flushLogBuffersForMove = useCallback(async (tabId: string) => {
    await terminalViewRefs.current.get(tabId)?.flushLogBuffersForMove();
  }, []);

  const clearActiveTerminalViewport = useCallback(() => {
    if (!activeTabId) return;
    terminalViewRefs.current.get(activeTabId)?.clearViewport();
  }, [activeTabId]);

  const clearActiveTerminalBuffer = useCallback(() => {
    if (!activeTabId) return;
    terminalViewRefs.current.get(activeTabId)?.clearBuffer();
  }, [activeTabId]);

  const selectAllActiveTerminal = useCallback(() => {
    if (!activeTabId) return;
    terminalViewRefs.current.get(activeTabId)?.selectAll();
  }, [activeTabId]);

  const copyActiveTerminalSelection = useCallback(() => {
    if (!activeTabId) return;
    terminalViewRefs.current.get(activeTabId)?.copySelection();
  }, [activeTabId]);

  const pasteIntoActiveTerminal = useCallback(() => {
    if (!activeTabId) return;
    terminalViewRefs.current.get(activeTabId)?.paste();
  }, [activeTabId]);

  const handleTerminalSelectionChange = useCallback((tabId: string, hasSelection: boolean) => {
    setTerminalSelectionByTab((current) => {
      if (Boolean(current.get(tabId)) === hasSelection) return current;
      const next = new Map(current);
      if (hasSelection) {
        next.set(tabId, true);
      } else {
        next.delete(tabId);
      }
      return next;
    });
  }, []);

  const terminalTabLifecycle = useTerminalTabLifecycle({
    tabs: windowTabs,
    onTerminalRemoved: removeTerminalFromState,
  });
  const workspaceTabMovement = useWorkspaceTabMovement({
    tabs: windowTabs,
    flushLogBuffersForMove,
  });

  const handleConnect = useCallback(
    async (
      type: ConnectionType,
      sessionId: string,
      title: string,
      logState: ConnectionLogState,
      encoding: Encoding = "utf-8",
      terminalMode: TerminalMode = DEFAULT_TERMINAL_MODE,
      connectionInfo?: WorkspaceConnectionInfo
    ) => {
      await terminalTabLifecycle.registerTerminalTab({
        connectionType: type,
        sessionId,
        title,
        isManualLogging: logState.isLogging,
        manualLogFilePath: logState.filePath,
        encoding,
        terminalMode,
        connectionInfo,
      });
      setConnectionInitialValues(null);
      setShowConnection(false);
      if (logState.startFailed) {
        showTemporaryLogStatus("statusbar.log_start_failed");
      }
    },
    [showTemporaryLogStatus, terminalTabLifecycle]
  );

  const handleViewChange = useCallback(
    (view: ViewMode) => {
      if (view === "settings" || view === "logs") {
        void (view === "settings" ? loadSettingsPanel() : loadLogViewer());
        windowTabs.openUtilityTab(view);
        return;
      }
      windowTabs.showTerminalView();
    },
    [windowTabs]
  );

  useEffect(() => {
    activeTerminalBuffer.current = activeTabId
      ? terminalBuffers.current.get(activeTabId) || ""
      : "";
  }, [activeTabId]);

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
        const tab = getCurrentWindowTabsState().tabs.find(
          (item) => item.sessionId === payload.session_id
        );
        if (!tab) {
          await submitLogControl(payload.request_id, null, "セッションが見つかりません");
          return;
        }
        if (!tab.isConnected) {
          await submitLogControl(payload.request_id, null, "セッションは切断済みです");
          return;
        }
        if (tab.isManualLogging && tab.manualLogFilePath) {
          if (tab.isManualLoggingPaused) {
            await updateWorkspaceTabMetadata(tab.id, { isManualLoggingPaused: false });
            await waitForUiUpdate();
          }
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
            isManualLoggingPaused: false,
            manualLogFilePath: filePath,
          });
          await waitForUiUpdate();
          await submitLogControl(payload.request_id, filePath, null);
        } catch (error) {
          console.error("Failed to start the MCP log:", error);
          await submitLogControl(
            payload.request_id,
            null,
            backendCommandErrorMessage(error, "Failed to start the external control log.")
          );
        }
      }
    );

    const unlistenStop = listen<McpLogControlRequestPayload>(
      "external-control://log-stop-request",
      async (event) => {
        const payload = event.payload;
        const tab = getCurrentWindowTabsState().tabs.find(
          (item) => item.sessionId === payload.session_id
        );
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
            isManualLoggingPaused: false,
          });
          await waitForUiUpdate();
          await submitLogControl(payload.request_id, null, null);
        } catch (error) {
          console.error("Failed to stop the MCP log:", error);
          await submitLogControl(
            payload.request_id,
            null,
            backendCommandErrorMessage(error, "Failed to stop the external control log.")
          );
        }
      }
    );

    return () => {
      unlistenStart.then((fn) => fn());
      unlistenStop.then((fn) => fn());
    };
  }, [getCurrentWindowTabsState, updateWorkspaceTabMetadata]);

  const handleTerminalData = useCallback(
    (tabId: string, data: string) => {
      // Keep last 2000 chars per tab for AI context.
      const nextBuffer = ((terminalBuffers.current.get(tabId) || "") + data).slice(-2000);
      terminalBuffers.current.set(tabId, nextBuffer);
      if (getCurrentWindowTabsState().activeTabId === tabId) {
        activeTerminalBuffer.current = nextBuffer;
      }
    },
    [getCurrentWindowTabsState]
  );

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
          error: translateBackendCommandError(error, t, t("mcp.credential_submit_failed")),
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
          isManualLoggingPaused: false,
          manualLogFilePath: filePath,
        });
      } catch (error) {
        console.error("Failed to start the log:", error);
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
        isManualLoggingPaused: false,
      });
    } catch (error) {
      console.error("Failed to stop the log:", error);
      showTemporaryLogStatus("statusbar.log_stop_failed");
    } finally {
      setManualLogBusyTabId(null);
    }
  }, [activeTab, showTemporaryLogStatus, updateWorkspaceTabMetadata]);

  const handleSetManualLoggingPaused = useCallback(
    (paused: boolean) => {
      if (!activeTab?.isConnected || !activeTab.isManualLogging) return;
      updateWorkspaceTabMetadata(activeTab.id, { isManualLoggingPaused: paused }).catch(
        console.error
      );
    },
    [activeTab, updateWorkspaceTabMetadata]
  );

  const handleTerminalLogShortcut = useCallback(
    (tabId: string, action: TerminalLogShortcutAction) => {
      if (
        !activeTab ||
        activeTab.id !== tabId ||
        !activeTab.isConnected ||
        manualLogBusyTabId === tabId
      ) {
        return;
      }

      switch (action) {
        case "terminal_log_start_overwrite":
          if (!activeTab.isManualLogging) {
            void handleStartManualLog("overwrite");
          }
          break;
        case "terminal_log_start_append":
          if (!activeTab.isManualLogging) {
            void handleStartManualLog("append");
          }
          break;
        case "terminal_log_stop":
          if (activeTab.isManualLogging) {
            void handleStopManualLog();
          }
          break;
        case "terminal_log_pause":
          if (
            canPauseManualLog(
              Boolean(activeTab.isManualLogging),
              Boolean(activeTab.isManualLoggingPaused)
            )
          ) {
            handleSetManualLoggingPaused(true);
          }
          break;
        case "terminal_log_resume":
          if (
            canResumeManualLog(
              Boolean(activeTab.isManualLogging),
              Boolean(activeTab.isManualLoggingPaused)
            )
          ) {
            handleSetManualLoggingPaused(false);
          }
          break;
      }
    },
    [
      activeTab,
      handleSetManualLoggingPaused,
      handleStartManualLog,
      handleStopManualLog,
      manualLogBusyTabId,
    ]
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

  const openWindow = windowTabs.createWorkspaceWindow;

  const activeTerminalTab =
    activeView === "terminal" && activeTab?.kind === "terminal" ? activeTab : null;
  const canAccessActiveTerminal = Boolean(activeTerminalTab?.sessionId);
  const canCopyActiveTerminal =
    canAccessActiveTerminal && Boolean(activeTabId && terminalSelectionByTab.get(activeTabId));
  const canPasteActiveTerminal = Boolean(
    activeTerminalTab?.sessionId && activeTerminalTab.isConnected
  );

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
      if (e.target instanceof Element && e.target.closest("[data-shortcut-recorder='true']")) {
        return;
      }

      const action = findShortcutAction(shortcuts, e, "application");
      if (!action) return;

      e.preventDefault();
      switch (action) {
        case "new_window":
          openWindow();
          break;
        case "new_connection":
          openConnection();
          break;
        case "open_settings":
          void loadSettingsPanel();
          openUtilityTab("settings");
          break;
        case "exit":
          appExit.requestExit();
          break;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [appExit.requestExit, openConnection, openUtilityTab, openWindow, shortcuts]);

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
    const unlisten = listen("config://updated", () => {
      void refreshConfig();
    });
    return () => {
      void unlisten.then((stopListening) => {
        stopListening();
      });
    };
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
        shortcuts={shortcuts}
        onViewChange={handleViewChange}
        onOpenConnection={openConnection}
        onOpenWindow={openWindow}
        canAccessTerminal={canAccessActiveTerminal}
        canCopyTerminal={canCopyActiveTerminal}
        canPasteTerminal={canPasteActiveTerminal}
        onSelectAllTerminal={selectAllActiveTerminal}
        onCopyTerminal={copyActiveTerminalSelection}
        onPasteTerminal={pasteIntoActiveTerminal}
        onClearTerminalViewport={clearActiveTerminalViewport}
        onClearTerminalBuffer={clearActiveTerminalBuffer}
        onToggleAiPanel={toggleAiPanel}
        onCheckForUpdates={appUpdate.checkManually}
        onExit={appExit.requestExit}
      />
      <AppUpdateDialog controller={appUpdate} />
      <AppExitDialog controller={appExit} />
      <div className="app__body">
        <div className="app__main">
          <div className="app__content">
            <div className="app__workspace">
              <TerminalTabs
                tabs={appTabs}
                activeTabId={activeTabId}
                closingTabIds={closingTabIds}
                onSelectTab={windowTabs.selectTab}
                onCloseTab={terminalTabLifecycle.closeTab}
                onMoveTabToNewWindow={workspaceTabMovement.moveTabToNewWindow}
                onOpenSameDestination={openSameDestination}
                onAddTab={openConnection}
                onReorderTabs={workspaceTabMovement.reorderTabs}
                windowId={windowTabs.windowId}
                dragPreview={workspaceDragPreview}
                onCrossWindowDragStart={workspaceTabMovement.startCrossWindowDrag}
                onCrossWindowDragUpdate={workspaceTabMovement.updateCrossWindowDrag}
                onCrossWindowDragDrop={(tabId, pointerScreenPosition) => {
                  void workspaceTabMovement.dropCrossWindowDrag(tabId, pointerScreenPosition);
                }}
                onCrossWindowDragCancel={workspaceTabMovement.cancelCrossWindowDrag}
                onCrossWindowDragHover={workspaceTabMovement.hoverCrossWindowDrag}
              />
              <div
                className={`app__terminal-area ${activeView !== "terminal" ? "app__hidden" : ""}`}
              >
                {openStatusBarMenu && activeTab && (
                  <StatusBarPalette
                    key={openStatusBarMenu}
                    kind={openStatusBarMenu}
                    activeTab={activeTab}
                    shortcuts={shortcuts}
                    onEncodingChange={(encoding) => {
                      handleEncodingChange(activeTab.id, encoding);
                    }}
                    onTerminalModeChange={(terminalMode) => {
                      handleTerminalModeChange(activeTab.id, terminalMode);
                    }}
                    onStartManualLog={(writeMode) => {
                      void handleStartManualLog(writeMode);
                    }}
                    onStopManualLog={() => {
                      void handleStopManualLog();
                    }}
                    onSetManualLoggingPaused={handleSetManualLoggingPaused}
                    onClose={handleStatusBarPaletteClose}
                  />
                )}
                {tabs.length === 0 ? (
                  <TerminalView
                    sessionId={null}
                    connectionType="ssh"
                    isConnected={false}
                    isActive={activeView === "terminal"}
                    isManualLogging={false}
                    isManualLoggingPaused={false}
                    onOpenConnection={openConnection}
                    onTerminalData={() => {}}
                    encoding="utf-8"
                    terminalConfig={config?.terminal}
                    shortcuts={shortcuts}
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
                      isManualLogging={Boolean(tab.isManualLogging)}
                      isManualLoggingPaused={Boolean(tab.isManualLoggingPaused)}
                      onOpenConnection={openConnection}
                      onTerminalData={(data) => {
                        handleTerminalData(tab.id, data);
                      }}
                      onTerminalLogShortcut={(action) => {
                        handleTerminalLogShortcut(tab.id, action);
                      }}
                      onTerminalSelectionChange={(hasSelection) => {
                        handleTerminalSelectionChange(tab.id, hasSelection);
                      }}
                      encoding={tab.encoding}
                      terminalMode={tab.terminalMode}
                      terminalConfig={config?.terminal}
                      shortcuts={shortcuts}
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
            openMenu={openStatusBarMenu}
            onMenuToggle={handleStatusBarMenuToggle}
            onMenuTriggerPointerDown={handleStatusBarMenuTriggerPointerDown}
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
      {sshPrompts.activePrompt?.kind === "authentication" && (
        <SshAuthenticationPromptDialog
          prompt={sshPrompts.activePrompt.value}
          onResponseChange={sshPrompts.updateResponse}
          onSubmit={() => {
            void sshPrompts.submit();
          }}
          onCancel={() => {
            void sshPrompts.cancel();
          }}
        />
      )}
      {sshPrompts.activePrompt?.kind === "host_key" && (
        <SshHostKeyPromptDialog
          prompt={sshPrompts.activePrompt.value}
          onAccept={() => {
            void sshPrompts.submit();
          }}
          onCancel={() => {
            void sshPrompts.cancel();
          }}
        />
      )}
      {activeMcpCredentialPrompt && (
        <div className="ui-overlay app-credential-overlay">
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
                  {activeMcpCredentialPrompt.auth_method === "public_key" ||
                  activeMcpCredentialPrompt.auth_method === "auto"
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
                {activeMcpCredentialPrompt.auth_method === "public_key" ||
                activeMcpCredentialPrompt.auth_method === "auto"
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
