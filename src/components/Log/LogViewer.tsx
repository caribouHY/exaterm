import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileText } from "lucide-react";
import type { LogSession } from "../../types";
import "./LogViewer.css";

export default function LogViewer() {
  const [sessions, setSessions] = useState<LogSession[]>([]);
  const [logDir, setLogDir] = useState("");

  useEffect(() => {
    invoke<LogSession[]>("logger_get_sessions").then(setSessions).catch(() => {});
    invoke<string>("logger_get_log_dir").then(setLogDir).catch(() => {});
  }, []);

  return (
    <div className="log-viewer">
      <h2>セッションログ</h2>
      <div className="log-viewer__subtitle">
        保存先: {logDir || "読み込み中..."}
      </div>

      {sessions.length === 0 ? (
        <div className="log-viewer__empty">
          <FileText size={32} />
          <span>まだログがありません</span>
          <span>設定で自動セッションログを有効にすると、接続時にログが記録されます</span>
        </div>
      ) : (
        <table className="log-table">
          <thead>
            <tr>
              <th>種別</th>
              <th>接続先</th>
              <th>開始時刻</th>
              <th>ファイル</th>
            </tr>
          </thead>
          <tbody>
            {sessions.map((s) => (
              <tr key={s.session_id}>
                <td>
                  <span className={`log-table__type log-table__type--${s.connection_type}`}>
                    {s.connection_type}
                  </span>
                </td>
                <td>{s.target}</td>
                <td>{new Date(s.started_at).toLocaleString("ja-JP")}</td>
                <td className="log-table__path" title={s.file_path}>{s.file_path.split(/[\\/]/).pop()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
