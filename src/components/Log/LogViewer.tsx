import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import { FileText, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { LogBulkDeleteResult, LogSession } from "../../types";
import "./LogViewer.css";

export default function LogViewer() {
  const { t, i18n } = useTranslation();
  const [sessions, setSessions] = useState<LogSession[]>([]);
  const [logDir, setLogDir] = useState("");
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteStatus, setDeleteStatus] = useState("");

  const loadSessions = useCallback(() => {
    invoke<LogSession[]>("logger_get_sessions")
      .then(setSessions)
      .catch(() => {});
    invoke<string>("logger_get_log_dir")
      .then(setLogDir)
      .catch(() => {});
  }, []);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleBulkDelete = async () => {
    const deleteAutoFilesLabel = t("logs.delete_dialog_include_files");
    const historyOnlyLabel = t("logs.delete_dialog_history_only");
    const dialogResult = await message(t("logs.delete_dialog_message"), {
      title: t("logs.delete_confirm_title"),
      kind: "warning",
      buttons: {
        yes: deleteAutoFilesLabel,
        no: historyOnlyLabel,
        cancel: t("logs.delete_dialog_cancel"),
      },
    });

    if (dialogResult !== deleteAutoFilesLabel && dialogResult !== historyOnlyLabel) return;

    const deleteAutoFiles = dialogResult === deleteAutoFilesLabel;

    if (deleteAutoFiles) {
      const confirmed = await message(t("logs.delete_confirm_files"), {
        title: t("logs.delete_confirm_title"),
        kind: "warning",
        buttons: {
          ok: t("logs.delete_dialog_confirm_files"),
          cancel: t("logs.delete_dialog_cancel"),
        },
      });
      if (confirmed !== t("logs.delete_dialog_confirm_files")) return;
    }

    setIsDeleting(true);
    setDeleteStatus("");
    try {
      const result = await invoke<LogBulkDeleteResult>("logger_bulk_delete_sessions", {
        deleteAutoFiles,
      });
      setDeleteStatus(
        t("logs.delete_result", {
          history: result.removed_history_count,
          files: result.removed_auto_file_count,
          active: result.skipped_active_count,
          manual: result.skipped_manual_file_count,
          missing: result.skipped_missing_file_count,
          unsafe: result.skipped_unsafe_path_count,
        })
      );
      loadSessions();
    } catch {
      setDeleteStatus(t("logs.delete_failed"));
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <div className="log-viewer">
      <div className="log-viewer__header">
        <div>
          <h2>{t("logs.title")}</h2>
          <div className="log-viewer__subtitle">{logDir || t("logs.loading")}</div>
        </div>
        <div className="log-viewer__actions">
          <button
            className="log-viewer__delete-button"
            onClick={handleBulkDelete}
            disabled={isDeleting || sessions.length === 0}
          >
            <Trash2 size={14} />
            <span>{isDeleting ? t("logs.deleting") : t("logs.delete_all")}</span>
          </button>
        </div>
      </div>
      {deleteStatus && <div className="log-viewer__status">{deleteStatus}</div>}

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
