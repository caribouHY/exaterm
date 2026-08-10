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
} from "../../components/Common";
import type { SshAuthenticationPromptState } from "./sshAuthenticationPromptModel";

interface SshAuthenticationPromptDialogProps {
  prompt: SshAuthenticationPromptState;
  onResponseChange: (index: number, value: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

export function SshAuthenticationPromptDialog({
  prompt,
  onResponseChange,
  onSubmit,
  onCancel,
}: SshAuthenticationPromptDialogProps) {
  const { t } = useTranslation();
  const title = prompt.name || t("connection.auth_prompt_title");
  const phase =
    prompt.phase === "jump"
      ? t("connection.auth_prompt_phase_jump")
      : t("connection.auth_prompt_phase_target");

  return (
    <div className="app-credential-overlay">
      <ModalFrame
        className="app-credential-modal ssh-auth-prompt"
        role="dialog"
        ariaModal
        ariaLabelledBy="ssh-auth-prompt-title"
        ariaDescribedBy={prompt.instructions ? "ssh-auth-prompt-instructions" : undefined}
      >
        <form
          onSubmit={(event) => {
            event.preventDefault();
            onSubmit();
          }}
        >
          <ModalHeader className="app-credential-modal__header">
            <div>
              <div className="app-credential-modal__eyebrow">{phase}</div>
              <ModalTitle
                className="app-credential-modal__title ssh-auth-prompt__title"
                id="ssh-auth-prompt-title"
              >
                {title}
              </ModalTitle>
            </div>
          </ModalHeader>
          <ModalBody className="app-credential-modal__body">
            <ModalTarget className="app-credential-modal__target">
              {prompt.username}@{prompt.host}:{prompt.port}
            </ModalTarget>
            {prompt.instructions && (
              <ModalDescription
                className="app-credential-modal__description ssh-auth-prompt__instructions"
                id="ssh-auth-prompt-instructions"
              >
                {prompt.instructions}
              </ModalDescription>
            )}
            <div className="ssh-auth-prompt__fields">
              {prompt.prompts.map((field, index) => {
                const inputId = `ssh-auth-prompt-response-${index}`;
                const response = prompt.responses.find(
                  (_value, responseIndex) => responseIndex === index
                );
                const label = field.prompt
                  ? field.prompt
                  : prompt.method === "password"
                    ? t("connection.password")
                    : t("connection.auth_prompt_response", { number: index + 1 });
                return (
                  <div className="ssh-auth-prompt__field" key={inputId}>
                    <label className="label" htmlFor={inputId}>
                      {label}
                    </label>
                    <input
                      id={inputId}
                      className="input"
                      type={field.echo ? "text" : "password"}
                      autoFocus={index === 0}
                      autoComplete="off"
                      value={response ?? ""}
                      disabled={prompt.submitting}
                      onChange={(event) => {
                        onResponseChange(index, event.target.value);
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Escape") {
                          event.preventDefault();
                          onCancel();
                        }
                      }}
                    />
                  </div>
                );
              })}
            </div>
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
                <button type="submit" className="btn btn-primary">
                  {t("connection.continue")}
                </button>
              </>
            )}
          </ModalFooter>
        </form>
      </ModalFrame>
    </div>
  );
}
