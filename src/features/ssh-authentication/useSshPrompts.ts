import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { translateBackendCommandError } from "../backend-errors/backendCommandError";
import {
  updateSshAuthenticationResponse,
  type SshAuthenticationPromptPayload,
} from "./sshAuthenticationPromptModel";
import {
  enqueueAuthenticationPrompt,
  enqueueHostKeyPrompt,
  removeSshPrompt,
  setSshPromptSubmission,
  updateSshPrompt,
  type SshHostKeyPromptPayload,
  type SshPromptState,
} from "./sshPromptModel";

interface SshPromptDismissedPayload {
  requestId: string;
}

export function useSshPrompts() {
  const { t } = useTranslation();
  const [queue, setQueue] = useState<SshPromptState[]>([]);
  const activePrompt = queue.length === 0 ? null : queue[0];

  useEffect(() => {
    const unlistenAuthentication = listen<SshAuthenticationPromptPayload>(
      "ssh://authentication-prompt",
      (event) => {
        setQueue((current) => enqueueAuthenticationPrompt(current, event.payload));
      }
    );
    const unlistenAuthenticationDismissed = listen<SshPromptDismissedPayload>(
      "ssh://authentication-prompt-dismissed",
      (event) => {
        setQueue((current) => removeSshPrompt(current, event.payload.requestId));
      }
    );
    const unlistenHostKey = listen<SshHostKeyPromptPayload>("ssh://host-key-prompt", (event) => {
      setQueue((current) => enqueueHostKeyPrompt(current, event.payload));
    });
    const unlistenHostKeyDismissed = listen<SshPromptDismissedPayload>(
      "ssh://host-key-prompt-dismissed",
      (event) => {
        setQueue((current) => removeSshPrompt(current, event.payload.requestId));
      }
    );
    return () => {
      void unlistenAuthentication.then((stopListening) => stopListening());
      void unlistenAuthenticationDismissed.then((stopListening) => stopListening());
      void unlistenHostKey.then((stopListening) => stopListening());
      void unlistenHostKeyDismissed.then((stopListening) => stopListening());
    };
  }, []);

  const updateActivePrompt = useCallback(
    (update: (prompt: SshPromptState) => SshPromptState) => {
      if (activePrompt === null) return;
      setQueue((current) => updateSshPrompt(current, activePrompt.value.requestId, update));
    },
    [activePrompt]
  );

  const updateResponse = useCallback(
    (index: number, value: string) => {
      updateActivePrompt((prompt) =>
        prompt.kind === "authentication"
          ? {
              ...prompt,
              value: updateSshAuthenticationResponse(prompt.value, index, value),
            }
          : prompt
      );
    },
    [updateActivePrompt]
  );

  const resolve = useCallback(
    async (accept: boolean) => {
      if (activePrompt === null || activePrompt.value.submitting) return;
      const requestId = activePrompt.value.requestId;
      const authenticationResponses =
        activePrompt.kind === "authentication" ? [...activePrompt.value.responses] : null;
      updateActivePrompt((prompt) => setSshPromptSubmission(prompt, true));
      try {
        if (activePrompt.kind === "authentication") {
          await invoke("ssh_authentication_respond", {
            requestId,
            responses: accept ? authenticationResponses : null,
          });
        } else {
          await invoke("ssh_host_key_respond", { requestId, accept });
        }
        setQueue((current) => removeSshPrompt(current, requestId));
      } catch (error) {
        const fallback =
          activePrompt.kind === "authentication"
            ? t("connection.auth_prompt_submit_failed")
            : t("connection.host_key_prompt_submit_failed");
        updateActivePrompt((prompt) =>
          setSshPromptSubmission(prompt, false, translateBackendCommandError(error, t, fallback))
        );
      }
    },
    [activePrompt, t, updateActivePrompt]
  );

  return {
    activePrompt,
    updateResponse,
    submit: () => resolve(true),
    cancel: () => resolve(false),
  };
}
