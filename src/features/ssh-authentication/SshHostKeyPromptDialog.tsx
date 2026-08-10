import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { HostKeyConfirmation } from "../../components/Connection/HostKeyConfirmation";
import {
  FeedbackMessage,
  ModalBody,
  ModalBusy,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTitle,
} from "../../components/Common";
import { getHostKeyPromptPresentation, type SshHostKeyPromptState } from "./sshPromptModel";

interface SshHostKeyPromptDialogProps {
  prompt: SshHostKeyPromptState;
  onAccept: () => void;
  onCancel: () => void;
}

export function SshHostKeyPromptDialog({
  prompt,
  onAccept,
  onCancel,
}: SshHostKeyPromptDialogProps) {
  const { t } = useTranslation();
  const presentation = getHostKeyPromptPresentation(prompt.status);
  const title = t(presentation.titleKey);
  const phase =
    prompt.phase === "jump"
      ? t("connection.auth_prompt_phase_jump")
      : t("connection.auth_prompt_phase_target");

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (prompt.submitting) return;
      if (event.key === "Escape") {
        event.preventDefault();
        onCancel();
      } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        onAccept();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onAccept, onCancel, prompt.submitting]);

  return (
    <div className="app-credential-overlay">
      <ModalFrame
        className="app-credential-modal ssh-host-key-prompt"
        role="dialog"
        ariaModal
        ariaLabelledBy="ssh-host-key-prompt-title"
      >
        <ModalHeader className="app-credential-modal__header">
          <div>
            <div className="app-credential-modal__eyebrow">{phase}</div>
            <ModalTitle className="app-credential-modal__title" id="ssh-host-key-prompt-title">
              {title}
            </ModalTitle>
          </div>
        </ModalHeader>
        <ModalBody className="app-credential-modal__body">
          <HostKeyConfirmation hostKeyCheck={prompt} />
          {prompt.error && (
            <FeedbackMessage tone="error" className="app-credential-modal__error">
              {prompt.error}
            </FeedbackMessage>
          )}
        </ModalBody>
        <ModalFooter className="app-credential-modal__footer">
          {prompt.submitting ? (
            <ModalBusy className="app-credential-modal__submitting">
              {t("connection.connecting")}
            </ModalBusy>
          ) : (
            <>
              <button type="button" className="btn btn-ghost" onClick={onCancel}>
                {t("connection.cancel")}
              </button>
              <button
                type="button"
                className={`btn ${presentation.actionClassName}`}
                onClick={onAccept}
              >
                {t(presentation.actionKey)}
              </button>
            </>
          )}
        </ModalFooter>
      </ModalFrame>
    </div>
  );
}
