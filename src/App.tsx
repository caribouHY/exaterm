import { useState, useRef, useCallback, useEffect } from "react";
import TitleBar from "./components/TitleBar/TitleBar";
import Sidebar from "./components/Sidebar/Sidebar";
import TerminalTabs from "./components/Terminal/TerminalTabs";
import TerminalView from "./components/Terminal/TerminalView";
import ConnectionDialog from "./components/Connection/ConnectionDialog";
import AIChatPanel from "./components/AI/AIChatPanel";
import StatusBar from "./components/StatusBar/StatusBar";
import SettingsPanel from "./components/Settings/SettingsPanel";
import LogViewer from "./components/Log/LogViewer";
import type { TabInfo, ViewMode, ConnectionType, Encoding, AppConfig, ChatMessage } from "./types";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

export default function App() {
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [closingTabIds, setClosingTabIds] = useState<string[]>([]);
  const [showConnection, setShowConnection] = useState(false);
  const [showAiPanel, setShowAiPanel] = useState(false);
  const [activeView, setActiveView] = useState<ViewMode>("terminal");
  const [aiPanelWidth, setAiPanelWidth] = useState(340);
  const [isDragging, setIsDragging] = useState(false);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [aiMessages, setAiMessages] = useState<ChatMessage[]>([]);
  const [aiSelectedProvider, setAiSelectedProvider] = useState("");
  const [aiSelectedModel, setAiSelectedModel] = useState("");
  const terminalBuffer = useRef("");
  const tabsRef = useRef<TabInfo[]>([]);
  const closeOperationsRef = useRef<Map<string, Promise<boolean>>>(new Map());

  const activeTab = tabs.find((t) => t.id === activeTabId) || null;

  const handleConnect = useCallback((type: ConnectionType, sessionId: string, title: string) => {
    const newTab: TabInfo = {
      id: sessionId,
      title,
      connectionType: type,
      sessionId,
      isConnected: true,
      encoding: "utf-8",
    };
    setTabs((prev) => [...prev, newTab]);
    setActiveTabId(sessionId);
    setShowConnection(false);
    setActiveView("terminal");
  }, []);

  useEffect(() => {
    tabsRef.current = tabs;
  }, [tabs]);

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
    if (activeTabId && !tabs.some((tab) => tab.id === activeTabId)) {
      setActiveTabId(tabs.length > 0 ? tabs[tabs.length - 1].id : null);
    }
  }, [activeTabId, tabs]);

  const removeTabFromState = useCallback((id: string) => {
    setTabs((prev) => prev.filter((tab) => tab.id !== id));
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
      await disconnectTab(id);
    },
    [disconnectTab]
  );

  const handleTerminalData = useCallback((data: string) => {
    // Keep last 2000 chars for AI context
    terminalBuffer.current = (terminalBuffer.current + data).slice(-2000);
  }, []);

  const handleEncodingChange = useCallback((id: string, encoding: Encoding) => {
    setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, encoding } : t)));
  }, []);

  const openConnection = () => setShowConnection(true);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        const key = e.key.toLowerCase();
        if (key === "n" || key === "t") {
          e.preventDefault();
          setShowConnection(true);
        } else if (key === ",") {
          e.preventDefault();
          setActiveView("settings");
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

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

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      // AI panel is on the right, so width is (window width - mouse X)
      const newWidth = window.innerWidth - e.clientX;
      if (newWidth > 200 && newWidth < window.innerWidth * 0.8) {
        setAiPanelWidth(newWidth);
      }
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
      <TitleBar />
      <div className="app__body">
        <Sidebar
          activeView={activeView}
          onViewChange={setActiveView}
          showAiPanel={showAiPanel}
          onToggleAiPanel={() => setShowAiPanel(!showAiPanel)}
          onOpenConnection={openConnection}
        />
        <div className="app__main">
          <div className="app__content">
            <div className={`app__terminal-area ${activeView !== "terminal" ? "app__hidden" : ""}`}>
              <TerminalTabs
                tabs={tabs}
                activeTabId={activeTabId}
                closingTabIds={closingTabIds}
                onSelectTab={setActiveTabId}
                onCloseTab={handleCloseTab}
                onAddTab={openConnection}
              />
              {tabs.length === 0 ? (
                <TerminalView
                  sessionId={null}
                  connectionType="ssh"
                  isConnected={false}
                  isActive={activeView === "terminal"}
                  onOpenConnection={openConnection}
                  onTerminalData={handleTerminalData}
                  encoding="utf-8"
                  terminalConfig={config?.terminal}
                />
              ) : (
                tabs.map((tab) => (
                  <TerminalView
                    key={tab.id}
                    sessionId={tab.sessionId || null}
                    connectionType={tab.connectionType}
                    isConnected={tab.isConnected}
                    isActive={activeView === "terminal" && tab.id === activeTabId}
                    onOpenConnection={openConnection}
                    onTerminalData={handleTerminalData}
                    encoding={tab.encoding}
                    terminalConfig={config?.terminal}
                  />
                ))
              )}
            </div>
            {activeView === "settings" && <SettingsPanel onSave={refreshConfig} />}
            {activeView === "logs" && <LogViewer />}
            {showAiPanel && (
              <>
                <div
                  className={`app__resizer ${isDragging ? "app__resizer--dragging" : ""}`}
                  onMouseDown={handleMouseDown}
                />
                <div style={{ width: aiPanelWidth, flexShrink: 0 }}>
                  <AIChatPanel
                    onClose={() => setShowAiPanel(false)}
                    terminalBuffer={terminalBuffer}
                    messages={aiMessages}
                    setMessages={setAiMessages}
                    selectedProvider={aiSelectedProvider}
                    setSelectedProvider={setAiSelectedProvider}
                    selectedModel={aiSelectedModel}
                    setSelectedModel={setAiSelectedModel}
                  />
                </div>
              </>
            )}
          </div>
          <StatusBar
            activeTab={activeTab}
            onEncodingChange={(encoding) =>
              activeTab && handleEncodingChange(activeTab.id, encoding)
            }
          />
        </div>
      </div>
      {showConnection && (
        <ConnectionDialog onClose={() => setShowConnection(false)} onConnect={handleConnect} />
      )}
    </div>
  );
}
