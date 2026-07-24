import { useEffect } from "react";
import type { SshCredentialPrompt, SshHostKeyCheck } from "./connectionDialogTypes";

interface UseConnectionDialogShortcutsParams {
  connecting: boolean;
  credentialPrompt: SshCredentialPrompt | null;
  hostKeyCheck: SshHostKeyCheck | null;
  onClose: () => void;
  onCloseCredentialPrompt: () => void;
  onCancelHostKeyCheck: () => void;
  onCredentialSubmit: () => void;
  onTrustAndConnect: (replace: boolean) => void;
  onConnect: () => void;
}

export const useConnectionDialogShortcuts = ({
  connecting,
  credentialPrompt,
  hostKeyCheck,
  onClose,
  onCloseCredentialPrompt,
  onCancelHostKeyCheck,
  onCredentialSubmit,
  onTrustAndConnect,
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
      if (hostKeyCheck) {
        onCancelHostKeyCheck();
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
      if (hostKeyCheck) {
        onTrustAndConnect(hostKeyCheck.status === "mismatch");
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
    hostKeyCheck,
    onCancelHostKeyCheck,
    onClose,
    onCloseCredentialPrompt,
    onConnect,
    onCredentialSubmit,
    onTrustAndConnect,
  ]);
};
