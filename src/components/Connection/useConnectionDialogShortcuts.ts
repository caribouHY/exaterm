import { useEffect } from "react";
import type { SshCredentialPrompt } from "./connectionDialogTypes";

interface UseConnectionDialogShortcutsParams {
  connecting: boolean;
  credentialPrompt: SshCredentialPrompt | null;
  onClose: () => void;
  onCloseCredentialPrompt: () => void;
  onCredentialSubmit: () => void;
  onConnect: () => void;
}

export const useConnectionDialogShortcuts = ({
  connecting,
  credentialPrompt,
  onClose,
  onCloseCredentialPrompt,
  onCredentialSubmit,
  onConnect,
}: UseConnectionDialogShortcutsParams) => {
  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      event.preventDefault();
      if (connecting) return;
      if (credentialPrompt) {
        onCloseCredentialPrompt();
        return;
      }
      onClose();
    };

    const handleSubmitShortcut = (event: KeyboardEvent) => {
      event.preventDefault();
      if (connecting) return;
      if (credentialPrompt) {
        onCredentialSubmit();
        return;
      }
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
  }, [
    connecting,
    credentialPrompt,
    onClose,
    onCloseCredentialPrompt,
    onConnect,
    onCredentialSubmit,
  ]);
};
