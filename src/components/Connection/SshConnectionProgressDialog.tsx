import { useTranslation } from "react-i18next";
import {
  FeedbackMessage,
  ModalBody,
  ModalBusy,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTarget,
  ModalTitle,
} from "../Common";
import type { SshConnectionAttemptState } from "./sshConnectionAttemptModel";
import { SshDiagnosticsPanel } from "./SshDiagnosticsPanel";
import type { SshDiagnosticEntry } from "./connectionDialogTypes";

interface SshConnectionProgressDialogProps {
  attempt: SshConnectionAttemptState;
  target: string;
  diagnostics: {
    logs: SshDiagnosticEntry[];
    expanded: boolean;
    copied: boolean;
    onToggleExpanded: () => void;
    onCopy: () => void;
  };
  onCancel: () => void;
}

function progressLabelKey(attempt: SshConnectionAttemptState) {
  if (attempt.status === "cancelling") return "connection.ssh_progress_cancelling";
  if (attempt.status === "preparing" || attempt.progress === null) {
    return "connection.ssh_progress_preparing";
  }
  return `connection.ssh_progress_${attempt.progress.phase}`;
}

export function SshConnectionProgressDialog({
  attempt,
  target,
  diagnostics,
  onCancel,
}: SshConnectionProgressDialogProps) {
  const { t } = useTranslation();
  const isJumpHost = attempt.progress?.target === "jump";

  return (
    <div className="connection-overlay">
      <ModalFrame
        className="connection-progress-dialog"
        role="dialog"
        ariaModal
        ariaLabelledBy="ssh-connection-progress-title"
      >
        <ModalHeader className="connection-dialog__header">
          <ModalTitle className="connection-dialog__title" id="ssh-connection-progress-title">
            {t("connection.ssh_progress_title")}
          </ModalTitle>
        </ModalHeader>
        <ModalBody className="connection-progress-dialog__body">
          <ModalTarget className="connection-progress-dialog__target">{target}</ModalTarget>
          <div className="connection-progress-dialog__status" role="status" aria-live="polite">
            {isJumpHost && (
              <span className="connection-progress-dialog__role">
                {t("connection.ssh_progress_jump")}
              </span>
            )}
            <ModalBusy>{t(progressLabelKey(attempt))}</ModalBusy>
          </div>
          {attempt.cancelError && (
            <div role="alert">
              <FeedbackMessage tone="error">{attempt.cancelError}</FeedbackMessage>
            </div>
          )}
          <SshDiagnosticsPanel
            logs={diagnostics.logs}
            expanded={diagnostics.expanded}
            copied={diagnostics.copied}
            onToggleExpanded={diagnostics.onToggleExpanded}
            onCopy={diagnostics.onCopy}
          />
        </ModalBody>
        <ModalFooter className="connection-dialog__footer">
          <button
            type="button"
            className="btn btn-ghost"
            onClick={onCancel}
            disabled={attempt.status === "cancelling"}
          >
            {attempt.status === "cancelling"
              ? t("connection.ssh_progress_cancelling")
              : t("connection.cancel")}
          </button>
        </ModalFooter>
      </ModalFrame>
    </div>
  );
}
