import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { CircleDot, FileText } from "lucide-react";
import packageJson from "../../../package.json";
import type { TabInfo, Encoding } from "../../types";
import "./StatusBar.css";

interface StatusBarProps {
  activeTab: TabInfo | null;
  onEncodingChange: (encoding: Encoding) => void;
  onStartManualLog: () => void;
  onStopManualLog: () => void;
  manualLogBusy: boolean;
  logStatusMessage: string;
}

export default function StatusBar({
  activeTab,
  onEncodingChange,
  onStartManualLog,
  onStopManualLog,
  manualLogBusy,
  logStatusMessage,
}: StatusBarProps) {
  const { t } = useTranslation();
  const [showMenu, setShowMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setShowMenu(false);
      }
    };
    if (showMenu) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showMenu]);

  const encodings: { label: string; value: Encoding }[] = [
    { label: "UTF-8", value: "utf-8" },
    { label: "Shift-JIS", value: "shift-jis" },
    { label: "EUC-JP", value: "euc-jp" },
  ];

  const getLogLabel = () => {
    if (logStatusMessage) return t(logStatusMessage);
    if (!activeTab?.isConnected) return t("statusbar.log_unavailable");
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

  const logTitle = activeTab?.isManualLogging
    ? activeTab.manualLogFilePath || t("statusbar.log_stop")
    : t("statusbar.log_manual_title");

  return (
    <div className="statusbar">
      <div className="statusbar__left">
        <div className="statusbar__item">
          <span
            className={`statusbar__dot ${activeTab?.isConnected ? "statusbar__dot--connected" : "statusbar__dot--disconnected"}`}
          />
          <span>
            {activeTab?.isConnected ? t("statusbar.connected") : t("statusbar.disconnected")}
          </span>
        </div>
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
          <button
            className={`statusbar__item statusbar__item--clickable statusbar__log ${
              activeTab.isManualLogging ? "statusbar__log--manual" : ""
            }`}
            onClick={handleLogClick}
            disabled={!activeTab.isConnected || manualLogBusy}
            title={logTitle}
          >
            {activeTab.isManualLogging ? <CircleDot size={12} /> : <FileText size={12} />}
            <span>{manualLogBusy ? t("statusbar.log_busy") : getLogLabel()}</span>
          </button>
        )}
        {activeTab && (
          <div className="statusbar__encoding-container" ref={menuRef}>
            <button
              className="statusbar__item statusbar__item--clickable"
              onClick={() => setShowMenu(!showMenu)}
            >
              {encodings.find((e) => e.value === activeTab.encoding)?.label ||
                activeTab.encoding.toUpperCase()}
            </button>
            {showMenu && (
              <div className="statusbar__menu">
                {encodings.map((e) => (
                  <button
                    key={e.value}
                    className={`statusbar__menu-item ${activeTab.encoding === e.value ? "statusbar__menu-item--active" : ""}`}
                    onClick={() => {
                      onEncodingChange(e.value);
                      setShowMenu(false);
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
