import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { translateBackendCommandError } from "../backend-errors/backendCommandError";
import {
  enqueueSshAuthenticationPrompt,
  removeSshAuthenticationPrompt,
  updateSshAuthenticationPrompt,
  updateSshAuthenticationResponse,
  type SshAuthenticationPromptPayload,
  type SshAuthenticationPromptState,
} from "./sshAuthenticationPromptModel";

interface SshAuthenticationPromptDismissedPayload {
  requestId: string;
}

interface SshAuthenticationPrompts {
  activePrompt: SshAuthenticationPromptState | null;
  updateResponse: (index: number, value: string) => void;
  submit: () => Promise<void>;
  cancel: () => Promise<void>;
}

export function useSshAuthenticationPrompts(): SshAuthenticationPrompts {
  const { t } = useTranslation();
  const [queue, setQueue] = useState<SshAuthenticationPromptState[]>([]);
  const activePrompt: SshAuthenticationPromptState | null = queue.length === 0 ? null : queue[0];

  useEffect(() => {
    const unlistenPrompt = listen<SshAuthenticationPromptPayload>(
      "ssh://authentication-prompt",
      (event) => {
        setQueue((current) => enqueueSshAuthenticationPrompt(current, event.payload));
      }
    );
    const unlistenDismissed = listen<SshAuthenticationPromptDismissedPayload>(
      "ssh://authentication-prompt-dismissed",
      (event) => {
        setQueue((current) => removeSshAuthenticationPrompt(current, event.payload.requestId));
      }
    );
    return () => {
      void unlistenPrompt.then((stopListening) => stopListening());
      void unlistenDismissed.then((stopListening) => stopListening());
    };
  }, []);

  const updatePrompt = useCallback(
    (
      requestId: string,
      update: (prompt: SshAuthenticationPromptState) => SshAuthenticationPromptState
    ) => {
      setQueue((current) => updateSshAuthenticationPrompt(current, requestId, update));
    },
    []
  );

  const updateResponse = useCallback(
    (index: number, value: string) => {
      if (activePrompt === null) return;
      updatePrompt(activePrompt.requestId, (prompt) =>
        updateSshAuthenticationResponse(prompt, index, value)
      );
    },
    [activePrompt, updatePrompt]
  );

  const resolve = useCallback(
    async (responses: string[] | null) => {
      if (activePrompt === null || activePrompt.submitting) return;
      const requestId = activePrompt.requestId;
      updatePrompt(requestId, (prompt) => ({ ...prompt, error: "", submitting: true }));
      try {
        await invoke("ssh_authentication_respond", {
          requestId,
          responses,
        });
        setQueue((current) => removeSshAuthenticationPrompt(current, requestId));
      } catch (error) {
        updatePrompt(requestId, (prompt) => ({
          ...prompt,
          error: translateBackendCommandError(error, t, t("connection.auth_prompt_submit_failed")),
          submitting: false,
        }));
      }
    },
    [activePrompt, t, updatePrompt]
  );

  return {
    activePrompt,
    updateResponse,
    submit: () => resolve(activePrompt === null ? [] : activePrompt.responses),
    cancel: () => resolve(null),
  };
}
