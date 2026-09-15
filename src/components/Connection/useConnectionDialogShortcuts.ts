import { useEffect } from "react";
import type { SshCredentialPrompt } from "./connectionDialogTypes";

interface UseConnectionDialogShortcutsParams {
  connecting: boolean;
  canConnect: boolean;
  credentialPrompt: SshCredentialPrompt | null;
  onClose: () => void;
  onCloseCredentialPrompt: () => void;

  onConnect: () => void;
}

export const useConnectionDialogShortcuts = ({
  connecting,
  canConnect,
  credentialPrompt,
  onClose,
  onCloseCredentialPrompt,

  onConnect,
}: UseConnectionDialogShortcutsParams) => {
  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      event.preventDefault();
      if (connecting && !credentialPrompt) return;
      if (credentialPrompt) {
        onCloseCredentialPrompt();
        return;
      }
      onClose();
    };

    const handleSubmitShortcut = (event: KeyboardEvent) => {
      event.preventDefault();
      if (connecting && !credentialPrompt) return;
      if (credentialPrompt) {
        return;
      }
      if (!canConnect) return;
      onConnect();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
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
  }, [canConnect, connecting, credentialPrompt, onClose, onCloseCredentialPrompt, onConnect]);
};
