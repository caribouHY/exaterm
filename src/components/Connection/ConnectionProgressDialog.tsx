import { useTranslation } from "react-i18next";
import type { ConnectionType } from "../../types";
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
import { SshDiagnosticsPanel } from "./SshDiagnosticsPanel";
import type { SshDiagnosticEntry } from "./connectionDialogTypes";

interface ConnectionProgressDialogProps {
  connectionType: ConnectionType;
  target: string;
  statusLabel: string;
  cancelling: boolean;
  cancelError: string;
  roleLabel?: string;
  diagnostics?: {
    logs: SshDiagnosticEntry[];
    expanded: boolean;
    copied: boolean;
    onToggleExpanded: () => void;
    onCopy: () => void;
  };
  onCancel: () => void;
}

export function ConnectionProgressDialog({
  connectionType,
  target,
  statusLabel,
  cancelling,
  cancelError,
  roleLabel,
  diagnostics,
  onCancel,
}: ConnectionProgressDialogProps) {
  const { t } = useTranslation();
  const titleId = `${connectionType}-connection-progress-title`;

  return (
    <div className="ui-overlay connection-overlay">
      <ModalFrame
        className="connection-progress-dialog"
        role="dialog"
        ariaModal
        ariaLabelledBy={titleId}
      >
        <ModalHeader className="connection-dialog__header">
          <ModalTitle className="connection-dialog__title" id={titleId}>
            {t(`connection.${connectionType}_progress_title`)}
          </ModalTitle>
        </ModalHeader>
        <ModalBody className="connection-progress-dialog__body">
          <ModalTarget className="connection-progress-dialog__target">{target}</ModalTarget>
          <div className="connection-progress-dialog__status" role="status" aria-live="polite">
            {roleLabel && <span className="connection-progress-dialog__role">{roleLabel}</span>}
            <ModalBusy>{statusLabel}</ModalBusy>
          </div>
          {cancelError && (
            <div role="alert">
              <FeedbackMessage tone="error">{cancelError}</FeedbackMessage>
            </div>
          )}
          {diagnostics && <SshDiagnosticsPanel {...diagnostics} />}
        </ModalBody>
        <ModalFooter className="connection-dialog__footer">
          <button type="button" className="btn btn-ghost" onClick={onCancel} disabled={cancelling}>
            {cancelling ? t("connection.progress_cancelling") : t("connection.cancel")}
          </button>
        </ModalFooter>
      </ModalFrame>
    </div>
  );
}
