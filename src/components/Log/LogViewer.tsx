import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import { FileText, RefreshCw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { LogBulkDeleteResult, LogSession } from "../../types";
import { EmptyState, FeedbackMessage } from "../Common";
import {
  createLogViewerLoadController,
  createLogViewerLoadState,
  getLogViewerPresentation,
} from "./logViewerLoadController";
import "./LogViewer.css";

export default function LogViewer() {
  const { t, i18n } = useTranslation();
  const [loadState, setLoadState] = useState(createLogViewerLoadState);
  const controllerRef = useRef<ReturnType<typeof createLogViewerLoadController> | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [deleteStatus, setDeleteStatus] = useState("");

  useEffect(() => {
    const controller = createLogViewerLoadController(
      {
        getSessions: () => invoke<LogSession[]>("logger_get_sessions"),
        getLogDir: () => invoke<string>("logger_get_log_dir"),
      },
      setLoadState
    );
    controllerRef.current = controller;
    void controller.load();
    return () => {
      controller.dispose();
      controllerRef.current = null;
    };
  }, []);

  const presentation = getLogViewerPresentation(loadState, isDeleting);
  const { sessions } = presentation;

  const handleBulkDelete = async () => {
    const controller = controllerRef.current;
    if (!controller || presentation.deleteDisabled) return;
    setIsDeleting(true);
    try {
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

      if (controllerRef.current !== controller) return;
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

      if (controllerRef.current !== controller) return;
      setDeleteStatus("");
      const result = await invoke<LogBulkDeleteResult>("logger_bulk_delete_sessions", {
        deleteAutoFiles,
      });
      if (controllerRef.current !== controller) return;
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
      void controller.load();
    } catch {
      if (controllerRef.current === controller) setDeleteStatus(t("logs.delete_failed"));
    } finally {
      if (controllerRef.current === controller) setIsDeleting(false);
    }
  };

  return (
    <div className="log-viewer">
      <div className="log-viewer__header">
        <div>
          <h2>{t("logs.title")}</h2>
          {loadState.logDir.data !== null && (
            <div className="log-viewer__subtitle">{loadState.logDir.data}</div>
          )}
          {loadState.logDir.phase === "loading" && (
            <div className="log-viewer__subtitle" role="status">
              {t("logs.directory_loading")}
            </div>
          )}
        </div>
        <div className="log-viewer__actions">
          <button
            type="button"
            className="btn btn-ghost btn-sm log-viewer__refresh-button"
            onClick={() => void controllerRef.current?.load()}
            disabled={presentation.refreshDisabled}
          >
            <RefreshCw size={14} aria-hidden="true" />
            <span>{t(presentation.refreshLabelKey)}</span>
          </button>
          <button
            className="log-viewer__delete-button"
            onClick={handleBulkDelete}
            disabled={presentation.deleteDisabled}
          >
            <Trash2 size={14} />
            <span>{isDeleting ? t("logs.deleting") : t("logs.delete_all")}</span>
          </button>
        </div>
      </div>
      {presentation.sessionsErrorKey && (
        <div role="alert">
          <FeedbackMessage tone="error" className="log-viewer__status">
            {t(presentation.sessionsErrorKey)}
          </FeedbackMessage>
        </div>
      )}
      {presentation.logDirErrorKey && (
        <div role="alert">
          <FeedbackMessage tone="error" className="log-viewer__status">
            {t(presentation.logDirErrorKey)}
          </FeedbackMessage>
        </div>
      )}
      {deleteStatus && (
        <FeedbackMessage tone="info" className="log-viewer__status">
          {deleteStatus}
        </FeedbackMessage>
      )}

      {loadState.sessions.phase === "loading" && (
        <div role="status" className="log-viewer__status">
          {t("logs.loading")}
        </div>
      )}
      {presentation.showEmpty && (
        <EmptyState
          icon={<FileText size={32} />}
          title={t("logs.no_logs")}
          className="log-viewer__empty"
        />
      )}
      {presentation.showTable && (
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
              <tr key={`${s.session_id}-${s.log_mode}-${s.started_at}-${s.file_path}`}>
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
