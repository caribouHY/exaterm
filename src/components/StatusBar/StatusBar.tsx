import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { TabInfo, Encoding } from "../../types";
import "./StatusBar.css";

interface StatusBarProps {
  activeTab: TabInfo | null;
  onEncodingChange: (encoding: Encoding) => void;
}

export default function StatusBar({ activeTab, onEncodingChange }: StatusBarProps) {
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

  return (
    <div className="statusbar">
      <div className="statusbar__left">
        <div className="statusbar__item">
          <span className={`statusbar__dot ${activeTab?.isConnected ? "statusbar__dot--connected" : "statusbar__dot--disconnected"}`} />
          <span>{activeTab?.isConnected ? t("statusbar.connected") : t("statusbar.disconnected")}</span>
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
          <div className="statusbar__encoding-container" ref={menuRef}>
            <button 
              className="statusbar__item statusbar__item--clickable" 
              onClick={() => setShowMenu(!showMenu)}
            >
              {encodings.find(e => e.value === activeTab.encoding)?.label || activeTab.encoding.toUpperCase()}
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
        <div className="statusbar__item">ExaTerm v0.1.0</div>
      </div>
    </div>
  );
}
