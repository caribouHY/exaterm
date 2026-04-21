import { Monitor, Wifi, Bot, FileText, Settings } from "lucide-react";
import type { ViewMode } from "../../types";
import "./Sidebar.css";

interface SidebarProps {
  activeView: ViewMode;
  onViewChange: (view: ViewMode) => void;
  showAiPanel: boolean;
  onToggleAiPanel: () => void;
  onOpenConnection: () => void;
}

export default function Sidebar({
  activeView,
  onViewChange,
  showAiPanel,
  onToggleAiPanel,
  onOpenConnection,
}: SidebarProps) {
  return (
    <nav className="sidebar">
      <div className="sidebar__top">
        <button
          className={`sidebar__btn ${activeView === "terminal" ? "sidebar__btn--active" : ""}`}
          onClick={() => onViewChange("terminal")}
        >
          <Monitor size={20} />
          <span className="sidebar__tooltip">ターミナル</span>
        </button>

        <button className="sidebar__btn" onClick={onOpenConnection}>
          <Wifi size={20} />
          <span className="sidebar__tooltip">新規接続</span>
        </button>

        <div className="sidebar__divider" />

        <button
          className={`sidebar__btn ${showAiPanel ? "sidebar__btn--active" : ""}`}
          onClick={onToggleAiPanel}
        >
          <Bot size={20} />
          <span className="sidebar__tooltip">AIアシスタント</span>
        </button>

        <button
          className={`sidebar__btn ${activeView === "logs" ? "sidebar__btn--active" : ""}`}
          onClick={() => onViewChange("logs")}
        >
          <FileText size={20} />
          <span className="sidebar__tooltip">ログ</span>
        </button>
      </div>

      <div className="sidebar__bottom">
        <button
          className={`sidebar__btn ${activeView === "settings" ? "sidebar__btn--active" : ""}`}
          onClick={() => onViewChange("settings")}
        >
          <Settings size={20} />
          <span className="sidebar__tooltip">設定</span>
        </button>
      </div>
    </nav>
  );
}
