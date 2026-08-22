import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  FeedbackMessage,
  ModalBody,
  ModalBusy,
  ModalDescription,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTarget,
  ModalTitle,
} from "../Common";
import type { SshCredentialPrompt } from "./connectionDialogTypes";
import { SshDiagnosticsPanel } from "./SshDiagnosticsPanel";
import type { SshDiagnosticEntry } from "./connectionDialogTypes";

interface CredentialPromptModalProps {
  credentialPrompt: SshCredentialPrompt;
  connecting: boolean;
  diagnostics: {
    logs: SshDiagnosticEntry[];
    expanded: boolean;
    copied: boolean;
    onToggleExpanded: () => void;
    onCopy: () => void;
  };
  onClose: () => void;
  onSubmit: () => void;
  onValueChange: (value: string) => void;
}

export function CredentialPromptModal({
  credentialPrompt,
  connecting,
  diagnostics,
  onClose,
  onSubmit,
  onValueChange,
}: CredentialPromptModalProps) {
  const { t } = useTranslation();
  const promptsForKeyPassphrase =
    credentialPrompt.authMethod === "auto" || credentialPrompt.authMethod === "public_key";
  const credentialTitle = promptsForKeyPassphrase
    ? t("connection.key_passphrase_prompt_title")
    : t("connection.password_prompt_title");
  const credentialLabel = promptsForKeyPassphrase
    ? t("connection.key_passphrase")
    : t("connection.password");
  const credentialDescription = promptsForKeyPassphrase
    ? t("connection.key_passphrase_prompt_desc")
    : t("connection.password_prompt_desc");

  return (
    <div
      className="ui-overlay connection-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <ModalFrame
        className="connection-credential-modal"
        onClick={(e) => {
          e.stopPropagation();
        }}
      >
        <ModalHeader className="connection-credential-modal__header">
          <ModalTitle className="connection-dialog__title">{credentialTitle}</ModalTitle>
          <button
            className="btn-icon"
            onClick={onClose}
            disabled={connecting}
            aria-label={t("connection.cancel")}
          >
            <X size={16} />
          </button>
        </ModalHeader>
        <ModalBody className="connection-credential-modal__body">
          <ModalTarget className="connection-credential-modal__target">
            {credentialPrompt.username}@{credentialPrompt.host}:{credentialPrompt.port}
          </ModalTarget>
          <ModalDescription className="connection-credential-modal__description">
            {credentialDescription}
          </ModalDescription>
          <div>
            <label className="label">{credentialLabel}</label>
            <input
              className="input"
              type="password"
              autoFocus
              autoComplete="off"
              disabled={connecting}
              value={credentialPrompt.value}
              onChange={(event) => {
                onValueChange(event.target.value);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  onSubmit();
                }
              }}
            />
          </div>
          {credentialPrompt.error && (
            <FeedbackMessage tone="error" className="connection-dialog__error">
              {credentialPrompt.error}
            </FeedbackMessage>
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
          {connecting ? (
            <ModalBusy className="connection-dialog__connecting">
              {t("connection.connecting")}
            </ModalBusy>
          ) : (
            <>
              <button className="btn btn-ghost" onClick={onClose}>
                {t("connection.cancel")}
              </button>
              <button className="btn btn-primary" onClick={onSubmit}>
                {t("connection.connect")}
              </button>
            </>
          )}
        </ModalFooter>
      </ModalFrame>
    </div>
  );
}
