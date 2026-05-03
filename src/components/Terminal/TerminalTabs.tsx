import { FileText, Monitor, Network, Settings, Usb, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { AppTabInfo, ConnectionType } from "../../types";
import "./TerminalTabs.css";

interface TerminalTabsProps {
  tabs: AppTabInfo[];
  activeTabId: string | null;
  closingTabIds: string[];
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => Promise<void>;
  onAddTab: () => void;
}

export default function TerminalTabs({
  tabs,
  activeTabId,
  closingTabIds,
  onSelectTab,
  onCloseTab,
  onAddTab,
}: TerminalTabsProps) {
  const { t } = useTranslation();
  const iconFor = (connectionType: ConnectionType) => {
    if (connectionType === "ssh") return <Monitor size={13} />;
    if (connectionType === "telnet") return <Network size={13} />;
    return <Usb size={13} />;
  };

  const labelFor = (tab: AppTabInfo) => {
    switch (tab.kind) {
      case "settings":
        return t("settings.title");
      case "logs":
        return t("logs.title");
      case "terminal":
        return tab.title;
    }
  };

  return (
    <div className="terminal-tabs">
      {tabs.map((tab) => {
        const isClosing = closingTabIds.includes(tab.id);
        const isTerminalTab = tab.kind === "terminal";

        return (
          <button
            key={tab.id}
            type="button"
            className={`terminal-tab ${tab.id === activeTabId ? "terminal-tab--active" : ""}`}
            onClick={() => {
              if (!isClosing) {
                onSelectTab(tab.id);
              }
            }}
            disabled={isClosing}
            aria-busy={isClosing}
          >
            <span className="terminal-tab__icon">
              {isTerminalTab && iconFor(tab.connectionType)}
              {tab.kind === "settings" && <Settings size={13} />}
              {tab.kind === "logs" && <FileText size={13} />}
            </span>
            {isTerminalTab && (
              <span
                className={`terminal-tab__status ${
                  tab.isConnected
                    ? "terminal-tab__status--connected"
                    : "terminal-tab__status--disconnected"
                }`}
              />
            )}
            <span className="terminal-tab__label">{labelFor(tab)}</span>
            <span
              className="terminal-tab__close"
              onClick={(e) => {
                e.stopPropagation();
                if (!isClosing) {
                  void onCloseTab(tab.id);
                }
              }}
            >
              <X size={12} />
            </span>
          </button>
        );
      })}
      <button
        type="button"
        className="terminal-tabs__add"
        onClick={onAddTab}
        aria-label={t("terminal.new_tab")}
      >
        <Plus size={14} />
      </button>
    </div>
  );
}
