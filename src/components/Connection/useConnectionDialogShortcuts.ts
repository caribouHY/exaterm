import { useEffect } from "react";
import type { SshCredentialPrompt } from "./connectionDialogTypes";
import type { AttemptStatus } from "./connectionAttemptState";

interface UseConnectionDialogShortcutsParams {
  attemptStatus: AttemptStatus;
  enabled: boolean;
  canConnect: boolean;
  credentialPrompt: SshCredentialPrompt | null;
  onClose: () => void;
  onCloseCredentialPrompt: () => void;
  onCancelConnection: () => void;
  onConnect: () => void;
}

export const useConnectionDialogShortcuts = ({
  attemptStatus,
  enabled,
  canConnect,
  credentialPrompt,
  onClose,
  onCloseCredentialPrompt,
  onCancelConnection,
  onConnect,
}: UseConnectionDialogShortcutsParams) => {
  useEffect(() => {
    if (!enabled) return;

    const handleEscape = (event: KeyboardEvent) => {
      event.preventDefault();
      if (event.repeat) return;
      if (credentialPrompt) {
        onCloseCredentialPrompt();
        return;
      }
      if (attemptStatus === "editing") {
        onClose();
      } else if (attemptStatus === "preparing" || attemptStatus === "connecting") {
        onCancelConnection();
      }
    };

    const handleSubmitShortcut = (event: KeyboardEvent) => {
      event.preventDefault();
      if (attemptStatus !== "editing" && !credentialPrompt) return;
      if (credentialPrompt) {
        return;
      }
      if (!canConnect) return;
      onConnect();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") {
        handleEscape(event);
        return;
      }
      if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
        handleSubmitShortcut(event);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [
    attemptStatus,
    enabled,
    canConnect,
    credentialPrompt,
    onClose,
    onCloseCredentialPrompt,
    onCancelConnection,
    onConnect,
  ]);
};
