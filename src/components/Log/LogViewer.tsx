import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileText } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { LogSession } from "../../types";
import "./LogViewer.css";

export default function LogViewer() {
  const { t, i18n } = useTranslation();
  const [sessions, setSessions] = useState<LogSession[]>([]);
  const [logDir, setLogDir] = useState("");

  useEffect(() => {
    invoke<LogSession[]>("logger_get_sessions")
      .then(setSessions)
      .catch(() => {});
    invoke<string>("logger_get_log_dir")
      .then(setLogDir)
      .catch(() => {});
  }, []);

  return (
    <div className="log-viewer">
      <h2>{t("logs.title")}</h2>
      <div className="log-viewer__subtitle">{logDir || t("logs.loading")}</div>

      {sessions.length === 0 ? (
        <div className="log-viewer__empty">
          <FileText size={32} />
          <span>{t("logs.no_logs")}</span>
        </div>
      ) : (
        <table className="log-table">
          <thead>
            <tr>
              <th>{t("logs.type")}</th>
              <th>{t("logs.mode")}</th>
              <th>{t("logs.target")}</th>
              <th>{t("logs.started_at")}</th>
              <th>{t("logs.file")}</th>
            </tr>
          </thead>
          <tbody>
            {sessions.map((s) => (
              <tr key={`${s.session_id}-${s.log_mode}`}>
                <td>
                  <span className={`log-table__type log-table__type--${s.connection_type}`}>
                    {s.connection_type}
                  </span>
                </td>
                <td>{t(`logs.mode_${s.log_mode}`)}</td>
                <td>{s.target}</td>
                <td>
                  {new Date(s.started_at).toLocaleString(
                    i18n.language === "ja" ? "ja-JP" : "en-US"
                  )}
                </td>
                <td className="log-table__path" title={s.file_path}>
                  {s.file_path.split(/[\\/]/).pop()}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
