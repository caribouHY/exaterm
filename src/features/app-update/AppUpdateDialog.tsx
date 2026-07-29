import { useTranslation } from "react-i18next";
import {
  FeedbackMessage,
  ModalBody,
  ModalBusy,
  ModalDescription,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTitle,
} from "../../components/Common";
import type { ReturnTypeOfUseAppUpdate } from "./appUpdateTypes";
import { updateDownloadPercent } from "./updateModel";
import "./AppUpdateDialog.css";

interface AppUpdateDialogProps {
  controller: ReturnTypeOfUseAppUpdate;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function AppUpdateDialog({ controller }: AppUpdateDialogProps) {
  const { t } = useTranslation();
  const { state } = controller;
  if (state.phase === "closed") return null;

  const busy =
    state.phase === "checking" || state.phase === "downloading" || state.phase === "installing";
  const update =
    state.phase === "available" ||
    state.phase === "confirm_active_sessions" ||
    state.phase === "downloading" ||
    state.phase === "installing"
      ? state.update
      : null;
  const percent =
    state.phase === "downloading"
      ? updateDownloadPercent(state.downloadedBytes, state.contentLength)
      : null;

  return (
    <div className="app-update-overlay">
      <ModalFrame
        className="app-update-dialog"
        role={state.phase === "confirm_active_sessions" ? "alertdialog" : "dialog"}
        ariaModal
        ariaLabelledBy="app-update-title"
        ariaDescribedBy="app-update-description"
      >
        <ModalHeader>
          <ModalTitle id="app-update-title">{t("updates.title")}</ModalTitle>
          <ModalDescription id="app-update-description">
            {state.phase === "checking" && t("updates.checking")}
            {state.phase === "up_to_date" && t("updates.up_to_date")}
            {state.phase === "available" &&
              t("updates.available_description", { version: state.update.version })}
            {state.phase === "confirm_active_sessions" &&
              t("updates.active_sessions_warning", { count: state.activeSessionCount })}
            {state.phase === "downloading" && t("updates.downloading")}
            {state.phase === "installing" && t("updates.installing")}
            {state.phase === "error" && t("updates.failed")}
          </ModalDescription>
        </ModalHeader>

        <ModalBody className="app-update-dialog__body">
          {busy && state.phase !== "downloading" && <ModalBusy>{t("updates.working")}</ModalBusy>}

          {update && (
            <div className="app-update-dialog__version">
              <span>{t("updates.current_version", { version: update.currentVersion })}</span>
              <span aria-hidden="true">→</span>
              <strong>{t("updates.new_version", { version: update.version })}</strong>
            </div>
          )}

          {state.phase === "available" && (
            <div className="app-update-dialog__notes">
              <div className="label">{t("updates.release_notes")}</div>
              <div className="app-update-dialog__notes-content">
                {state.update.notes?.trim() || t("updates.no_release_notes")}
              </div>
            </div>
          )}

          {state.phase === "confirm_active_sessions" && (
            <FeedbackMessage tone="warning">{t("updates.active_sessions_detail")}</FeedbackMessage>
          )}

          {state.phase === "downloading" && (
            <div className="app-update-dialog__progress" aria-live="polite">
              <progress
                max={100}
                value={percent ?? undefined}
                aria-label={t("updates.download_progress")}
              />
              <span>
                {percent !== null
                  ? t("updates.downloaded_percent", { percent })
                  : t("updates.downloaded_bytes", {
                      downloaded: formatBytes(state.downloadedBytes),
                    })}
              </span>
            </div>
          )}

          {state.phase === "error" && (
            <FeedbackMessage tone="error">{t("updates.failed_detail")}</FeedbackMessage>
          )}
        </ModalBody>

        {!busy && (
          <ModalFooter>
            {state.phase === "up_to_date" && (
              <button className="btn btn-primary" onClick={controller.close}>
                {t("updates.close")}
              </button>
            )}
            {state.phase === "available" && (
              <>
                <button className="btn btn-secondary" onClick={controller.close}>
                  {t("updates.later")}
                </button>
                <button
                  className="btn btn-primary"
                  onClick={() => {
                    controller.install(state.update);
                  }}
                >
                  {t("updates.download_and_install")}
                </button>
              </>
            )}
            {state.phase === "confirm_active_sessions" && (
              <>
                <button
                  className="btn btn-secondary"
                  onClick={controller.cancelActiveSessionConfirmation}
                >
                  {t("updates.cancel")}
                </button>
                <button
                  className="btn btn-danger"
                  onClick={() => {
                    controller.confirmInstallWithActiveSessions(state.update);
                  }}
                >
                  {t("updates.update_anyway")}
                </button>
              </>
            )}
            {state.phase === "error" && (
              <>
                <button className="btn btn-secondary" onClick={controller.close}>
                  {t("updates.close")}
                </button>
                <button className="btn btn-primary" onClick={controller.checkManually}>
                  {t("updates.retry")}
                </button>
              </>
            )}
          </ModalFooter>
        )}
      </ModalFrame>
    </div>
  );
}
