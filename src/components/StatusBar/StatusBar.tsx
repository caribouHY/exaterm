import type { TabInfo } from "../../types";
import "./StatusBar.css";

interface StatusBarProps {
  activeTab: TabInfo | null;
}

export default function StatusBar({ activeTab }: StatusBarProps) {
  return (
    <div className="statusbar">
      <div className="statusbar__left">
        <div className="statusbar__item">
          <span className={`statusbar__dot ${activeTab?.isConnected ? "statusbar__dot--connected" : "statusbar__dot--disconnected"}`} />
          <span>{activeTab?.isConnected ? "接続中" : "未接続"}</span>
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
        <div className="statusbar__item">UTF-8</div>
        <div className="statusbar__item">ExaTerm v0.1.0</div>
      </div>
    </div>
  );
}
