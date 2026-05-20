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
  AppConfig,
  ChatMessage,
  TerminalMode,
  StartupCliRequest,
  ManualLogWriteMode,
} from "./types";
import { DEFAULT_TERMINAL_MODE } from "./utils/terminalModes";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
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

interface TerminalCreatedPayload {
  session_id: string;
  connection_type: ConnectionType;
  target: string;
  title: string;
  encoding?: Encoding;
  terminal_mode?: TerminalMode;
  auto_logging: boolean;
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

export default function App() {
  const { t } = useTranslation();
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [utilityTabs, setUtilityTabs] = useState<UtilityTabKind[]>([]);
  const [tabOrder, setTabOrder] = useState<string[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [closingTabIds, setClosingTabIds] = useState<string[]>([]);
  const [showConnection, setShowConnection] = useState(false);
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
  const tabsRef = useRef<TabInfo[]>([]);
  const activeTabIdRef = useRef<string | null>(null);
  const closeOperationsRef = useRef<Map<string, Promise<boolean>>>(new Map());

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

  const addTerminalTab = useCallback(
    (
      type: ConnectionType,
      sessionId: string,
      title: string,
      isAutoLogging: boolean,
      encoding: Encoding = "utf-8",
      terminalMode: TerminalMode = DEFAULT_TERMINAL_MODE
    ) => {
      const newTab: TabInfo = {
        id: sessionId,
        kind: "terminal",
        title,
        connectionType: type,
        sessionId,
        isConnected: true,
        encoding,
        terminalMode,
        isAutoLogging,
        isManualLogging: false,
        isLoggingPaused: false,
      };
      setTabs((prev) =>
        prev.some((tab) => tab.sessionId === sessionId) ? prev : [...prev, newTab]
      );
      setTabOrder((prev) => (prev.includes(sessionId) ? prev : [...prev, sessionId]));
      setActiveTabId(sessionId);
    },
    []
  );

  const handleConnect = useCallback(
    (
      type: ConnectionType,
      sessionId: string,
      title: string,
      isAutoLogging: boolean,
      encoding: Encoding = "utf-8",
      terminalMode: TerminalMode = DEFAULT_TERMINAL_MODE
    ) => {
      addTerminalTab(type, sessionId, title, isAutoLogging, encoding, terminalMode);
      setShowConnection(false);
    },
    [addTerminalTab]
  );

  const openUtilityTab = useCallback((kind: UtilityTabKind) => {
    setUtilityTabs((prev) => (prev.includes(kind) ? prev : [...prev, kind]));
    setTabOrder((prev) => (prev.includes(kind) ? prev : [...prev, kind]));
    setActiveTabId(kind);
  }, []);

  const handleViewChange = useCallback(
    (view: ViewMode) => {
      if (view === "settings" || view === "logs") {
        void (view === "settings" ? loadSettingsPanel() : loadLogViewer());
        openUtilityTab(view);
        return;
      }

      setActiveTabId((current) => {
        const currentIsTerminal = tabsRef.current.some((tab) => tab.id === current);
        if (currentIsTerminal) return current;
        return tabsRef.current.length > 0 ? tabsRef.current[tabsRef.current.length - 1].id : null;
      });
    },
    [openUtilityTab]
  );

  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);

  useEffect(() => {
    activeTabIdRef.current = activeTabId;
    activeTerminalBuffer.current = activeTabId
      ? terminalBuffers.current.get(activeTabId) || ""
      : "";
  }, [activeTabId]);

  useEffect(() => {
    const markDisconnected = (sessionId: string) => {
      setTabs((prev) =>
        prev.map((tab) => (tab.sessionId === sessionId ? { ...tab, isConnected: false } : tab))
      );
    };

    const unlistenSsh = listen<string>("ssh://disconnected", (event) => {
      markDisconnected(event.payload);
    });
    const unlistenSerial = listen<string>("serial://disconnected", (event) => {
      markDisconnected(event.payload);
    });
    const unlistenTelnet = listen<string>("telnet://disconnected", (event) => {
      markDisconnected(event.payload);
    });

    return () => {
      unlistenSsh.then((fn) => fn());
      unlistenSerial.then((fn) => fn());
      unlistenTelnet.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlistenCreated = listen<TerminalCreatedPayload>("terminal://created", (event) => {
      const payload = event.payload;
      if (
        payload.connection_type !== "ssh" &&
        payload.connection_type !== "telnet" &&
        payload.connection_type !== "serial"
      )
        return;
      addTerminalTab(
        payload.connection_type,
        payload.session_id,
        payload.title || payload.target,
        Boolean(payload.auto_logging),
        payload.encoding ?? "utf-8",
        payload.terminal_mode ?? DEFAULT_TERMINAL_MODE
      );
    });

    return () => {
      unlistenCreated.then((fn) => fn());
    };
  }, [addTerminalTab]);

  useEffect(() => {
    const unlistenCredential = listen<McpCredentialRequestPayload>(
      "mcp://credential-request",
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
          window.setTimeout(() => resolve(), 0);
        });
      });

    const submitLogControl = async (
      requestId: string,
      filePath: string | null,
      error: string | null
    ) => {
      try {
        await invoke("mcp_log_control_submit", {
          requestId,
          filePath,
          error,
        });
      } catch (submitError) {
        console.error("Failed to submit MCP log control response:", submitError);
      }
    };

    const unlistenStart = listen<McpLogControlRequestPayload>(
      "mcp://log-start-request",
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
          setTabs((prev) =>
            prev.map((item) =>
              item.sessionId === payload.session_id
                ? {
                    ...item,
                    isManualLogging: true,
                    isLoggingPaused: false,
                    manualLogFilePath: filePath,
                  }
                : item
            )
          );
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
      "mcp://log-stop-request",
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
          setTabs((prev) =>
            prev.map((item) =>
              item.sessionId === payload.session_id
                ? {
                    ...item,
                    isManualLogging: false,
                    isLoggingPaused: item.isAutoLogging ? item.isLoggingPaused : false,
                  }
                : item
            )
          );
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
      setActiveTabId(appTabs.length > 0 ? appTabs[appTabs.length - 1].id : null);
    }
  }, [activeTabId, appTabs]);

  const removeTabFromState = useCallback((id: string) => {
    terminalBuffers.current.delete(id);
    if (activeTabIdRef.current === id) {
      activeTerminalBuffer.current = "";
    }
    setTabs((prev) => prev.filter((tab) => tab.id !== id));
    setTabOrder((prev) => prev.filter((tabId) => tabId !== id));
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

        setClosingTabIds((prev) => (prev.includes(id) ? prev : [...prev, id]));

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
          removeTabFromState(id);
          return true;
        } catch (error) {
          console.error(
            `Failed to disconnect ${tab.connectionType} session ${tab.sessionId}:`,
            error
          );
          return false;
        } finally {
          closeOperationsRef.current.delete(id);
          setClosingTabIds((prev) => prev.filter((tabId) => tabId !== id));
        }
      })();

      closeOperationsRef.current.set(id, operation);
      return operation;
    },
    [removeTabFromState]
  );

  const handleCloseTab = useCallback(
    async (id: string) => {
      if (id === "settings" || id === "logs") {
        setUtilityTabs((prev) => prev.filter((kind) => kind !== id));
        setTabOrder((prev) => prev.filter((tabId) => tabId !== id));
        return;
      }

      await disconnectTab(id);
    },
    [disconnectTab]
  );

  const handleReorderTabs = useCallback(
    (draggedId: string, targetId: string, dropSide: "before" | "after") => {
      if (draggedId === targetId) return;

      const visibleOrder = appTabs.map((tab) => tab.id);
      const draggedIndex = visibleOrder.indexOf(draggedId);
      const targetIndex = visibleOrder.indexOf(targetId);
      if (draggedIndex < 0 || targetIndex < 0) return;

      const nextOrder = [...visibleOrder];
      const [draggedTabId] = nextOrder.splice(draggedIndex, 1);
      const targetIndexAfterRemoval = nextOrder.indexOf(targetId);
      const insertIndex =
        dropSide === "after" ? targetIndexAfterRemoval + 1 : targetIndexAfterRemoval;
      nextOrder.splice(insertIndex, 0, draggedTabId);
      setTabOrder(nextOrder);
    },
    [appTabs]
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
        await invoke("mcp_credential_submit", {
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

  const handleEncodingChange = useCallback((id: string, encoding: Encoding) => {
    setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, encoding } : t)));
  }, []);

  const handleTerminalModeChange = useCallback((id: string, terminalMode: TerminalMode) => {
    setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, terminalMode } : t)));
  }, []);

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
    window.setTimeout(() => setLogStatusMessage(""), 3000);
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
        setTabs((prev) =>
          prev.map((tab) =>
            tab.id === activeTab.id
              ? {
                  ...tab,
                  isManualLogging: true,
                  isLoggingPaused: false,
                  manualLogFilePath: filePath,
                }
              : tab
          )
        );
      } catch (error) {
        console.error("Failed to start manual log:", error);
        showTemporaryLogStatus("statusbar.log_start_failed");
      } finally {
        setManualLogBusyTabId(null);
      }
    },
    [activeTab, buildManualLogFileName, showTemporaryLogStatus]
  );

  const handleStopManualLog = useCallback(async () => {
    if (!activeTab?.sessionId || !activeTab.isManualLogging) return;

    setManualLogBusyTabId(activeTab.id);
    try {
      await terminalViewRefs.current.get(activeTab.id)?.flushManualLogBuffer();
      await invoke("logger_stop_manual", { sessionId: activeTab.sessionId });
      setTabs((prev) =>
        prev.map((tab) =>
          tab.id === activeTab.id
            ? {
                ...tab,
                isManualLogging: false,
                isLoggingPaused: tab.isAutoLogging ? tab.isLoggingPaused : false,
              }
            : tab
        )
      );
    } catch (error) {
      console.error("Failed to stop manual log:", error);
      showTemporaryLogStatus("statusbar.log_stop_failed");
    } finally {
      setManualLogBusyTabId(null);
    }
  }, [activeTab, showTemporaryLogStatus]);

  const handleSetLoggingPaused = useCallback(
    (paused: boolean) => {
      if (!activeTab?.isConnected || !(activeTab.isAutoLogging || activeTab.isManualLogging))
        return;
      setTabs((prev) =>
        prev.map((tab) => (tab.id === activeTab.id ? { ...tab, isLoggingPaused: paused } : tab))
      );
    },
    [activeTab]
  );

  const openConnection = useCallback(() => {
    void loadConnectionDialog();
    setShowConnection(true);
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
        if (key === "n" || key === "t") {
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
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [openConnection, openUtilityTab]);

  const refreshConfig = useCallback(async () => {
    try {
      const cfg = await invoke<AppConfig>("config_load");
      setConfig(cfg);
    } catch (e) {
      console.error("Failed to load config:", e);
    }
  }, []);

  useEffect(() => {
    refreshConfig();
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
    return () => window.removeEventListener("resize", handleResize);
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
                onSelectTab={setActiveTabId}
                onCloseTab={handleCloseTab}
                onAddTab={openConnection}
                onReorderTabs={handleReorderTabs}
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
                      onTerminalData={(data) => handleTerminalData(tab.id, data)}
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
                      onClose={() => setShowAiPanel(false)}
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
            startupRequest={startupCliRequest}
            onStartupRequestHandled={() => setStartupCliRequest(null)}
            onClose={() => setShowConnection(false)}
            onConnect={handleConnect}
          />
        </Suspense>
      )}
      {activeMcpCredentialPrompt && (
        <div className="app-credential-overlay">
          <div
            className="app-credential-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="mcp-credential-title"
            aria-describedby="mcp-credential-description"
          >
            <div className="app-credential-modal__header">
              <div>
                <div className="app-credential-modal__eyebrow">
                  {t("mcp.credential_request_title")}
                </div>
                <div className="app-credential-modal__title" id="mcp-credential-title">
                  {activeMcpCredentialPrompt.auth_method === "public_key"
                    ? t("connection.key_passphrase_prompt_title")
                    : t("connection.password_prompt_title")}
                </div>
              </div>
            </div>
            <div className="app-credential-modal__body">
              <div className="app-credential-modal__target">{activeMcpCredentialPrompt.target}</div>
              <p className="app-credential-modal__description" id="mcp-credential-description">
                {t("mcp.credential_request_desc")}
              </p>
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
                onChange={(event) => updateActiveMcpCredentialPrompt({ value: event.target.value })}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    void resolveMcpCredentialPrompt(activeMcpCredentialPrompt.value);
                  } else if (event.key === "Escape") {
                    void resolveMcpCredentialPrompt(null);
                  }
                }}
              />
              {activeMcpCredentialPrompt.error && (
                <div className="app-credential-modal__error">{activeMcpCredentialPrompt.error}</div>
              )}
            </div>
            <div className="app-credential-modal__footer">
              {activeMcpCredentialPrompt.submitting ? (
                <div className="app-credential-modal__submitting">
                  <div className="app-credential-modal__spinner" />
                  {t("connection.connecting")}
                </div>
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
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
