import { Monitor, Usb, Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { TabInfo } from "../../types";
import "./TerminalTabs.css";

interface TerminalTabsProps {
  tabs: TabInfo[];
  activeTabId: string | null;
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  onAddTab: () => void;
}

export default function TerminalTabs({
  tabs, activeTabId, onSelectTab, onCloseTab, onAddTab,
}: TerminalTabsProps) {
  const { t } = useTranslation();

  return (
    <div className="terminal-tabs">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          className={`terminal-tab ${tab.id === activeTabId ? "terminal-tab--active" : ""}`}
          onClick={() => onSelectTab(tab.id)}
        >
          <span className="terminal-tab__icon">
            {tab.connectionType === "ssh" ? <Monitor size={13} /> : <Usb size={13} />}
          </span>
          <span
            className={`terminal-tab__status ${
              tab.isConnected ? "terminal-tab__status--connected" : "terminal-tab__status--disconnected"
            }`}
          />
          <span className="terminal-tab__label">{tab.title}</span>
          <span
            className="terminal-tab__close"
            onClick={(e) => { e.stopPropagation(); onCloseTab(tab.id); }}
          >
            <X size={12} />
          </span>
        </button>
      ))}
      <button className="terminal-tabs__add" onClick={onAddTab} aria-label={t("terminal.new_tab")}>
        <Plus size={14} />
      </button>
    </div>
  );
}
