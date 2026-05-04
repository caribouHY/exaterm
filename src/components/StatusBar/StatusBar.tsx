import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { CircleDot, FileText, Pause, Play } from "lucide-react";
import packageJson from "../../../package.json";
import type { TabInfo, Encoding, TerminalMode } from "../../types";
import { TERMINAL_MODE_OPTIONS } from "../../utils/terminalModes";
import "./StatusBar.css";

interface StatusBarProps {
  activeTab: TabInfo | null;
  showConnectionStatus: boolean;
  onEncodingChange: (encoding: Encoding) => void;
  onTerminalModeChange: (terminalMode: TerminalMode) => void;
  onStartManualLog: () => void;
  onStopManualLog: () => void;
  onToggleLoggingPaused: () => void;
  manualLogBusy: boolean;
  logStatusMessage: string;
}

export default function StatusBar({
  activeTab,
  showConnectionStatus,
  onEncodingChange,
  onTerminalModeChange,
  onStartManualLog,
  onStopManualLog,
  onToggleLoggingPaused,
  manualLogBusy,
  logStatusMessage,
}: StatusBarProps) {
  const { t } = useTranslation();
  const [openMenu, setOpenMenu] = useState<"encoding" | "terminalMode" | null>(null);
  const encodingMenuRef = useRef<HTMLDivElement>(null);
  const terminalModeMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        encodingMenuRef.current?.contains(target) ||
        terminalModeMenuRef.current?.contains(target)
      ) {
        return;
      }
      setOpenMenu(null);
    };
    if (openMenu) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [openMenu]);

  const encodings: { label: string; value: Encoding }[] = [
    { label: "UTF-8", value: "utf-8" },
    { label: "Shift-JIS", value: "shift-jis" },
    { label: "EUC-JP", value: "euc-jp" },
  ];

  const terminalModes = TERMINAL_MODE_OPTIONS.map((mode) => ({
    label: t(mode.labelKey),
    value: mode.value,
  }));

  const getLogLabel = () => {
    if (logStatusMessage) return t(logStatusMessage);
    if (!activeTab?.isConnected) return t("statusbar.log_unavailable");
    if (activeTab.isLoggingPaused) return t("statusbar.log_paused");
    if (activeTab.isAutoLogging && activeTab.isManualLogging) {
      return t("statusbar.log_auto_manual");
    }
    if (activeTab.isManualLogging) return t("statusbar.log_manual");
    if (activeTab.isAutoLogging) return t("statusbar.log_auto");
    return t("statusbar.log_start");
  };

  const handleLogClick = () => {
    if (!activeTab?.isConnected || manualLogBusy) return;
    if (activeTab.isManualLogging) {
      onStopManualLog();
      return;
    }
    onStartManualLog();
  };

  const hasActiveLog = Boolean(activeTab?.isAutoLogging || activeTab?.isManualLogging);
  const logTitle = activeTab?.isManualLogging
    ? activeTab.manualLogFilePath || t("statusbar.log_stop")
    : t("statusbar.log_manual_title");
  const pauseTitle = activeTab?.isLoggingPaused
    ? t("statusbar.log_resume_title")
    : t("statusbar.log_pause_title");

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
          <div
            className={`statusbar__log-group ${
              activeTab.isLoggingPaused ? "statusbar__log-group--paused" : ""
            }`}
          >
            <button
              className={`statusbar__log statusbar__log-action ${
                activeTab.isManualLogging ? "statusbar__log-action--manual" : ""
              } ${activeTab.isLoggingPaused ? "statusbar__log-action--paused" : ""}`}
              onClick={handleLogClick}
              disabled={!activeTab.isConnected || manualLogBusy}
              title={logTitle}
            >
              {activeTab.isManualLogging ? <CircleDot size={12} /> : <FileText size={12} />}
              <span>{manualLogBusy ? t("statusbar.log_busy") : getLogLabel()}</span>
            </button>
            {hasActiveLog && (
              <button
                className={`statusbar__log-toggle ${
                  activeTab.isLoggingPaused ? "statusbar__log-toggle--paused" : ""
                }`}
                onClick={onToggleLoggingPaused}
                disabled={!activeTab.isConnected}
                title={pauseTitle}
              >
                {activeTab.isLoggingPaused ? <Play size={12} /> : <Pause size={12} />}
                <span>
                  {activeTab.isLoggingPaused ? t("statusbar.log_resume") : t("statusbar.log_pause")}
                </span>
              </button>
            )}
          </div>
        )}
        {activeTab && (
          <div className="statusbar__menu-container" ref={terminalModeMenuRef}>
            <button
              className="statusbar__item statusbar__item--clickable"
              onClick={() => setOpenMenu(openMenu === "terminalMode" ? null : "terminalMode")}
              title={t("connection.terminal_mode")}
            >
              {terminalModes.find((mode) => mode.value === activeTab.terminalMode)?.label ||
                activeTab.terminalMode}
            </button>
            {openMenu === "terminalMode" && (
              <div className="statusbar__menu">
                {terminalModes.map((mode) => (
                  <button
                    key={mode.value}
                    className={`statusbar__menu-item ${
                      activeTab.terminalMode === mode.value ? "statusbar__menu-item--active" : ""
                    }`}
                    onClick={() => {
                      onTerminalModeChange(mode.value);
                      setOpenMenu(null);
                    }}
                  >
                    {mode.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
        {activeTab && (
          <div className="statusbar__menu-container" ref={encodingMenuRef}>
            <button
              className="statusbar__item statusbar__item--clickable"
              onClick={() => setOpenMenu(openMenu === "encoding" ? null : "encoding")}
            >
              {encodings.find((e) => e.value === activeTab.encoding)?.label ||
                activeTab.encoding.toUpperCase()}
            </button>
            {openMenu === "encoding" && (
              <div className="statusbar__menu">
                {encodings.map((e) => (
                  <button
                    key={e.value}
                    className={`statusbar__menu-item ${activeTab.encoding === e.value ? "statusbar__menu-item--active" : ""}`}
                    onClick={() => {
                      onEncodingChange(e.value);
                      setOpenMenu(null);
                    }}
                  >
                    {e.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
        <div className="statusbar__item">ExaTerm v{packageJson.version}</div>
      </div>
    </div>
  );
}
