import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { CircleDot, FilePlus, FileText, Pause, Play } from "lucide-react";
import packageJson from "../../../package.json";
import type { TabInfo, Encoding, TerminalMode, ManualLogWriteMode } from "../../types";
import { TERMINAL_MODE_OPTIONS } from "../../utils/terminalModes";
import "./StatusBar.css";

interface StatusBarProps {
  activeTab: TabInfo | null;
  showConnectionStatus: boolean;
  onEncodingChange: (encoding: Encoding) => void;
  onTerminalModeChange: (terminalMode: TerminalMode) => void;
  onStartManualLog: (writeMode: ManualLogWriteMode) => void;
  onStopManualLog: () => void;
  onSetLoggingPaused: (paused: boolean) => void;
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
  onSetLoggingPaused,
  manualLogBusy,
  logStatusMessage,
}: StatusBarProps) {
  const { t } = useTranslation();
  const [openMenu, setOpenMenu] = useState<"encoding" | "terminalMode" | "log" | null>(null);
  const encodingMenuRef = useRef<HTMLDivElement>(null);
  const terminalModeMenuRef = useRef<HTMLDivElement>(null);
  const logMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        encodingMenuRef.current?.contains(target) ||
        terminalModeMenuRef.current?.contains(target) ||
        logMenuRef.current?.contains(target)
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
    if (activeTab.isLoggingPaused && (activeTab.isAutoLogging || activeTab.isManualLogging)) {
      return t("statusbar.log_paused");
    }
    if (activeTab.isAutoLogging && activeTab.isManualLogging) {
      return t("statusbar.log_auto_manual");
    }
    if (activeTab.isManualLogging) return t("statusbar.log_manual");
    if (activeTab.isAutoLogging) return t("statusbar.log_auto");
    return t("statusbar.log_start");
  };

  const isLoggingActive = Boolean(activeTab?.isAutoLogging || activeTab?.isManualLogging);
  const logTitle = activeTab?.isLoggingPaused
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
          <div className="statusbar__menu-container" ref={logMenuRef}>
            <button
              className={`statusbar__item statusbar__item--clickable statusbar__log ${
                activeTab.isManualLogging ? "statusbar__log--manual" : ""
              } ${activeTab.isLoggingPaused ? "statusbar__log--paused" : ""}`}
              onClick={() => setOpenMenu(openMenu === "log" ? null : "log")}
              disabled={!activeTab.isConnected || manualLogBusy}
              title={logTitle}
            >
              {activeTab.isLoggingPaused ? (
                <Pause size={12} />
              ) : activeTab.isManualLogging ? (
                <CircleDot size={12} />
              ) : (
                <FileText size={12} />
              )}
              <span>{manualLogBusy ? t("statusbar.log_busy") : getLogLabel()}</span>
            </button>
            {openMenu === "log" && (
              <div className="statusbar__menu statusbar__menu--log">
                <button
                  className="statusbar__menu-item"
                  disabled={!activeTab.isConnected || activeTab.isManualLogging}
                  onClick={() => {
                    onStartManualLog("overwrite");
                    setOpenMenu(null);
                  }}
                >
                  <FileText size={12} />
                  <span>{t("statusbar.log_start_manual_overwrite")}</span>
                </button>
                <button
                  className="statusbar__menu-item"
                  disabled={!activeTab.isConnected || activeTab.isManualLogging}
                  onClick={() => {
                    onStartManualLog("append");
                    setOpenMenu(null);
                  }}
                >
                  <FilePlus size={12} />
                  <span>{t("statusbar.log_start_manual_append")}</span>
                </button>
                <button
                  className="statusbar__menu-item"
                  disabled={!activeTab.isManualLogging}
                  onClick={() => {
                    onStopManualLog();
                    setOpenMenu(null);
                  }}
                >
                  <CircleDot size={12} />
                  <span>{t("statusbar.log_stop_manual")}</span>
                </button>
                <button
                  className="statusbar__menu-item"
                  disabled={!isLoggingActive || activeTab.isLoggingPaused}
                  onClick={() => {
                    onSetLoggingPaused(true);
                    setOpenMenu(null);
                  }}
                >
                  <Pause size={12} />
                  <span>{t("statusbar.log_pause")}</span>
                </button>
                <button
                  className="statusbar__menu-item"
                  disabled={!isLoggingActive || !activeTab.isLoggingPaused}
                  onClick={() => {
                    onSetLoggingPaused(false);
                    setOpenMenu(null);
                  }}
                >
                  <Play size={12} />
                  <span>{t("statusbar.log_resume")}</span>
                </button>
              </div>
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
