import { useTranslation } from "react-i18next";
import {
  ModalBody,
  ModalDescription,
  ModalFooter,
  ModalFrame,
  ModalHeader,
  ModalTitle,
} from "../Common";

interface ProfileDeletionDialogProps {
  name: string;
  protocol: string;
  deleting: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

export function ProfileDeletionDialog({
  name,
  protocol,
  deleting,
  onCancel,
  onConfirm,
}: ProfileDeletionDialogProps) {
  const { t } = useTranslation();

  return (
    <div className="ui-overlay connection-overlay">
      <ModalFrame
        className="connection-dialog connection-dialog--profile-deletion"
        role="alertdialog"
        ariaModal
        ariaLabelledBy="profile-deletion-title"
        ariaDescribedBy="profile-deletion-description"
        focusTarget="#profile-deletion-cancel"
      >
        <ModalHeader>
          <ModalTitle id="profile-deletion-title">
            {t("connection.profile_delete_title")}
          </ModalTitle>
        </ModalHeader>
        <ModalBody className="connection-dialog__body">
          <ModalDescription id="profile-deletion-description">
            {t("connection.profile_delete_confirm", { name, protocol })}
          </ModalDescription>
        </ModalBody>
        <ModalFooter>
          <button
            id="profile-deletion-cancel"
            type="button"
            className="btn btn-secondary"
            disabled={deleting}
            onClick={onCancel}
          >
            {t("connection.cancel")}
          </button>
          <button type="button" className="btn btn-danger" disabled={deleting} onClick={onConfirm}>
            {t(deleting ? "connection.profile_deleting" : "connection.profile_delete")}
          </button>
        </ModalFooter>
      </ModalFrame>
    </div>
  );
}
