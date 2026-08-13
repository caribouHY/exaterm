import { useTranslation } from "react-i18next";
import { CircleDot, FileText, Pause } from "lucide-react";
import packageJson from "../../../package.json";
import type { TabInfo } from "../../types";
import { getTerminalModeOptions } from "../../utils/terminalModes";
import { STATUS_BAR_ENCODINGS, type StatusBarMenuKind } from "./statusBarMenuModel";
import "./StatusBar.css";

interface StatusBarProps {
  activeTab: TabInfo | null;
  showConnectionStatus: boolean;
  openMenu: StatusBarMenuKind | null;
  onMenuToggle: (kind: StatusBarMenuKind, pointerActivated: boolean) => void;
  onMenuTriggerPointerDown: () => void;
  manualLogBusy: boolean;
  logStatusMessage: string;
}

export default function StatusBar({
  activeTab,
  showConnectionStatus,
  openMenu,
  onMenuToggle,
  onMenuTriggerPointerDown,
  manualLogBusy,
  logStatusMessage,
}: StatusBarProps) {
  const { t } = useTranslation();

  const terminalModes = getTerminalModeOptions(t);

  const getLogLabel = () => {
    if (logStatusMessage) return t(logStatusMessage);
    if (!activeTab?.isConnected) return t("statusbar.log_unavailable");
    if (activeTab.isManualLogging && activeTab.isManualLoggingPaused) {
      return t("statusbar.log_paused");
    }
    if (activeTab.isManualLogging) return t("statusbar.log_manual");
    return t("statusbar.log_start");
  };

  const logTitle = activeTab?.isManualLoggingPaused
    ? t("statusbar.log_resume")
    : t("statusbar.log_menu_title");

  return (
    <div className="statusbar">
      <div className="statusbar__left">
        {showConnectionStatus && (
          <div className="statusbar__item">
            <span
              className={`statusbar__dot ${activeTab?.isConnected ? "statusbar__dot--connected" : "statusbar__dot--disconnected"}`}
            />
            <span>
              {activeTab?.isConnected ? t("statusbar.connected") : t("statusbar.disconnected")}
            </span>
          </div>
        )}
        {activeTab && (
          <div className="statusbar__item">
            <span>{activeTab.connectionType.toUpperCase()}</span>
            <span>—</span>
            <span>{activeTab.title}</span>
          </div>
        )}
      </div>
      <div className="statusbar__right">
        {activeTab && (
          <div className="statusbar__menu-container">
            <button
              id="statusbar-menu-trigger-log"
              className={`statusbar__item statusbar__item--clickable statusbar__log ${
                activeTab.isManualLogging ? "statusbar__log--manual" : ""
              } ${activeTab.isManualLoggingPaused ? "statusbar__log--paused" : ""}`}
              onPointerDown={onMenuTriggerPointerDown}
              onClick={(event) => {
                onMenuToggle("log", event.detail > 0);
              }}
              disabled={!activeTab.isConnected || manualLogBusy}
              title={logTitle}
              type="button"
              data-statusbar-menu-trigger
              aria-haspopup="dialog"
              aria-expanded={openMenu === "log"}
              aria-controls={openMenu === "log" ? "statusbar-command-palette" : undefined}
            >
              {activeTab.isManualLoggingPaused ? (
                <Pause size={12} />
              ) : activeTab.isManualLogging ? (
                <CircleDot size={12} />
              ) : (
                <FileText size={12} />
              )}
              <span>{manualLogBusy ? t("statusbar.log_busy") : getLogLabel()}</span>
            </button>
          </div>
        )}
        {activeTab && (
          <div className="statusbar__menu-container">
            <button
              id="statusbar-menu-trigger-terminalMode"
              className="statusbar__item statusbar__item--clickable"
              onPointerDown={onMenuTriggerPointerDown}
              onClick={(event) => {
                onMenuToggle("terminalMode", event.detail > 0);
              }}
              title={t("connection.terminal_mode")}
              type="button"
              data-statusbar-menu-trigger
              aria-haspopup="dialog"
              aria-expanded={openMenu === "terminalMode"}
              aria-controls={openMenu === "terminalMode" ? "statusbar-command-palette" : undefined}
            >
              {terminalModes.find((mode) => mode.value === activeTab.terminalMode)?.label ||
                activeTab.terminalMode}
            </button>
          </div>
        )}
        {activeTab && (
          <div className="statusbar__menu-container">
            <button
              id="statusbar-menu-trigger-encoding"
              className="statusbar__item statusbar__item--clickable"
              onPointerDown={onMenuTriggerPointerDown}
              onClick={(event) => {
                onMenuToggle("encoding", event.detail > 0);
              }}
              type="button"
              data-statusbar-menu-trigger
              aria-haspopup="dialog"
              aria-expanded={openMenu === "encoding"}
              aria-controls={openMenu === "encoding" ? "statusbar-command-palette" : undefined}
            >
              {STATUS_BAR_ENCODINGS.find((encoding) => encoding.value === activeTab.encoding)
                ?.label || activeTab.encoding.toUpperCase()}
            </button>
          </div>
        )}
        <div className="statusbar__item">ExaTerm v{packageJson.version}</div>
      </div>
    </div>
  );
}
