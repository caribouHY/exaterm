import { useTranslation } from "react-i18next";
import {
  FeedbackMessage,
  ModalBody,
  ModalDescription,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTitle,
} from "../../components/Common";
import type { useAppExit } from "./useAppExit";
import "./AppExitDialog.css";

interface AppExitDialogProps {
  controller: ReturnType<typeof useAppExit>;
}

export function AppExitDialog({ controller }: AppExitDialogProps) {
  const { t } = useTranslation();
  if (controller.activeSessionCount === null) return null;

  return (
    <div className="ui-overlay app-exit-overlay">
      <ModalFrame
        className="app-exit-dialog"
        role="alertdialog"
        ariaModal
        ariaLabelledBy="app-exit-title"
        ariaDescribedBy="app-exit-description"
      >
        <ModalHeader>
          <ModalTitle id="app-exit-title">{t("exit.title")}</ModalTitle>
          <ModalDescription id="app-exit-description">
            {t("exit.active_sessions_warning", {
              count: controller.activeSessionCount,
            })}
          </ModalDescription>
        </ModalHeader>

        <ModalBody>
          <FeedbackMessage tone="warning">{t("exit.active_sessions_detail")}</FeedbackMessage>
        </ModalBody>

        <ModalFooter>
          <button
            className="btn btn-secondary"
            onClick={controller.cancelExit}
            disabled={controller.isExiting}
          >
            {t("exit.cancel")}
          </button>
          <button
            className="btn btn-danger"
            onClick={controller.confirmExit}
            disabled={controller.isExiting}
          >
            {t("exit.confirm")}
          </button>
        </ModalFooter>
      </ModalFrame>
    </div>
  );
}
