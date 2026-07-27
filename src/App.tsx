import { lazy, Suspense, useState, useRef, useCallback, useEffect } from "react";
import TitleBar from "./components/TitleBar/TitleBar";
import TerminalTabs from "./components/Terminal/TerminalTabs";
import TerminalView from "./components/Terminal/TerminalView";
import type { TerminalViewHandle } from "./components/Terminal/TerminalView";
import StatusBar from "./components/StatusBar/StatusBar";
import type {
  TabInfo,
  ViewMode,
  ConnectionType,
  Encoding,
  AppConfig,
  ChatMessage,
  TerminalMode,
  StartupCliRequest,
  ManualLogWriteMode,
  WorkspaceConnectionInfo,
} from "./types";
import type { ConnectionDialogInitialValues } from "./components/Connection/connectionDialogTypes";
import { DEFAULT_TERMINAL_MODE } from "./utils/terminalModes";
import { invoke } from "@tauri-apps/api/core";
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

export default function App() {
  const { t } = useTranslation();
  const windowTabs = useWindowTabs();
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
  const [aiMessages, setAiMessages] = useState<ChatMessage[]>([]);
  const [aiSelectedProvider, setAiSelectedProvider] = useState("");
  const [aiSelectedModel, setAiSelectedModel] = useState("");
  const [startupCliRequest, setStartupCliRequest] = useState<StartupCliRequest | null>(null);
  const [mcpCredentialPrompts, setMcpCredentialPrompts] = useState<McpCredentialPromptState[]>([]);
  const activeTerminalBuffer = useRef("");
  const terminalBuffers = useRef<Map<string, string>>(new Map());
  const terminalViewRefs = useRef<Map<string, TerminalViewHandle>>(new Map());
  const activeMcpCredentialPrompt = mcpCredentialPrompts[0] ?? null;
  const shortcuts = config?.shortcuts ?? DEFAULT_SHORTCUT_CONFIG;

  const removeTerminalFromState = useCallback(
    (tabId: string) => {
      terminalBuffers.current.delete(tabId);
      if (getCurrentWindowTabsState().activeTabId === tabId) {
        activeTerminalBuffer.current = "";
      }
    },
    [getCurrentWindowTabsState]
  );

  const flushLogBuffersForMove = useCallback(async (tabId: string) => {
    await terminalViewRefs.current.get(tabId)?.flushLogBuffersForMove();
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
      isAutoLogging: boolean,
      encoding: Encoding = "utf-8",
      terminalMode: TerminalMode = DEFAULT_TERMINAL_MODE,
      connectionInfo?: WorkspaceConnectionInfo
    ) => {
      await terminalTabLifecycle.registerTerminalTab({
        connectionType: type,
        sessionId,
        title,
        isAutoLogging,
        encoding,
        terminalMode,
        connectionInfo,
      });
      setConnectionInitialValues(null);
      setShowConnection(false);
    },
    [terminalTabLifecycle]
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

  const openWindow = windowTabs.createWorkspaceWindow;

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
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [openConnection, openUtilityTab, openWindow, shortcuts]);

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
