import { Monitor, Wifi, Bot, FileText, Settings } from "lucide-react";
import type { ViewMode } from "../../types";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();

  return (
    <nav className="sidebar">
      <div className="sidebar__top">
        <button
          className={`sidebar__btn ${activeView === "terminal" ? "sidebar__btn--active" : ""}`}
          onClick={() => onViewChange("terminal")}
        >
          <Monitor size={20} />
          <span className="sidebar__tooltip">{t("sidebar.terminal")}</span>
        </button>

        <button className="sidebar__btn" onClick={onOpenConnection}>
          <Wifi size={20} />
          <span className="sidebar__tooltip">{t("sidebar.new_connection")}</span>
        </button>

        <div className="sidebar__divider" />

        <button
          className={`sidebar__btn ${showAiPanel ? "sidebar__btn--active" : ""}`}
          onClick={onToggleAiPanel}
        >
          <Bot size={20} />
          <span className="sidebar__tooltip">{t("sidebar.ai_assistant")}</span>
        </button>

        <button
          className={`sidebar__btn ${activeView === "logs" ? "sidebar__btn--active" : ""}`}
          onClick={() => onViewChange("logs")}
        >
          <FileText size={20} />
          <span className="sidebar__tooltip">{t("sidebar.logs")}</span>
        </button>
      </div>

      <div className="sidebar__bottom">
        <button
          className={`sidebar__btn ${activeView === "settings" ? "sidebar__btn--active" : ""}`}
          onClick={() => onViewChange("settings")}
        >
          <Settings size={20} />
          <span className="sidebar__tooltip">{t("sidebar.settings")}</span>
        </button>
      </div>
    </nav>
  );
}
