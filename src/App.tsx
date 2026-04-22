import { useState, useRef, useCallback } from "react";
import TitleBar from "./components/TitleBar/TitleBar";
import Sidebar from "./components/Sidebar/Sidebar";
import TerminalTabs from "./components/Terminal/TerminalTabs";
import TerminalView from "./components/Terminal/TerminalView";
import ConnectionDialog from "./components/Connection/ConnectionDialog";
import AIChatPanel from "./components/AI/AIChatPanel";
import StatusBar from "./components/StatusBar/StatusBar";
import SettingsPanel from "./components/Settings/SettingsPanel";
import LogViewer from "./components/Log/LogViewer";
import type { TabInfo, ViewMode, ConnectionType } from "./types";
import "./App.css";

export default function App() {
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [showConnection, setShowConnection] = useState(false);
  const [showAiPanel, setShowAiPanel] = useState(false);
  const [activeView, setActiveView] = useState<ViewMode>("terminal");
  const terminalBuffer = useRef("");

  const activeTab = tabs.find((t) => t.id === activeTabId) || null;

  const handleConnect = useCallback(
    (type: ConnectionType, sessionId: string, title: string) => {
      const newTab: TabInfo = {
        id: sessionId,
        title,
        connectionType: type,
        sessionId,
        isConnected: true,
      };
      setTabs((prev) => [...prev, newTab]);
      setActiveTabId(sessionId);
      setShowConnection(false);
      setActiveView("terminal");
    },
    []
  );

  const handleCloseTab = useCallback(
    (id: string) => {
      setTabs((prev) => {
        const next = prev.filter((t) => t.id !== id);
        if (activeTabId === id) {
          setActiveTabId(next.length > 0 ? next[next.length - 1].id : null);
        }
        return next;
      });
    },
    [activeTabId]
  );

  const handleTerminalData = useCallback((data: string) => {
    // Keep last 2000 chars for AI context
    terminalBuffer.current = (terminalBuffer.current + data).slice(-2000);
  }, []);

  const openConnection = () => setShowConnection(true);

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
                  />
                ))
              )}
            </div>
            {activeView === "settings" && <SettingsPanel />}
            {activeView === "logs" && <LogViewer />}
            {showAiPanel && (
              <AIChatPanel
                onClose={() => setShowAiPanel(false)}
                terminalBuffer={terminalBuffer.current}
              />
            )}
          </div>
          <StatusBar activeTab={activeTab} />
        </div>
      </div>
      {showConnection && (
        <ConnectionDialog
          onClose={() => setShowConnection(false)}
          onConnect={handleConnect}
        />
      )}
    </div>
  );
}
