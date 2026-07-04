import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
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
  const credentialTitle =
    credentialPrompt.authMethod === "public_key"
      ? t("connection.key_passphrase_prompt_title")
      : t("connection.password_prompt_title");
  const credentialLabel =
    credentialPrompt.authMethod === "public_key"
      ? t("connection.key_passphrase")
      : t("connection.password");
  const credentialDescription =
    credentialPrompt.authMethod === "public_key"
      ? t("connection.key_passphrase_prompt_desc")
      : t("connection.password_prompt_desc");

  return (
    <div
      className="connection-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="connection-credential-modal"
        onClick={(e) => {
          e.stopPropagation();
        }}
      >
        <div className="connection-credential-modal__header">
          <span className="connection-dialog__title">{credentialTitle}</span>
          <button className="btn-icon" onClick={onClose} disabled={connecting}>
            <X size={16} />
          </button>
        </div>
        <div className="connection-credential-modal__body">
          <div className="connection-credential-modal__target">
            {credentialPrompt.username}@{credentialPrompt.host}:{credentialPrompt.port}
          </div>
          <div className="connection-credential-modal__description">{credentialDescription}</div>
          <div>
            <label className="label">{credentialLabel}</label>
            <input
              className="input"
              type="password"
              autoFocus
              value={credentialPrompt.value}
              onChange={(e) => onValueChange(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  onSubmit();
                }
              }}
            />
          </div>
          {credentialPrompt.error && (
            <div className="connection-dialog__error">{credentialPrompt.error}</div>
          )}
          <SshDiagnosticsPanel
            logs={diagnostics.logs}
            expanded={diagnostics.expanded}
            copied={diagnostics.copied}
            onToggleExpanded={diagnostics.onToggleExpanded}
            onCopy={diagnostics.onCopy}
          />
        </div>
        <div className="connection-dialog__footer">
          {connecting ? (
            <div className="connection-dialog__connecting">
              <div className="connection-dialog__spinner" />
              {t("connection.connecting")}
            </div>
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
        </div>
      </div>
    </div>
  );
}
