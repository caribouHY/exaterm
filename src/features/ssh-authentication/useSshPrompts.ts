import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import {
  parseBackendCommandError,
  translateBackendCommandError,
} from "../backend-errors/backendCommandError";
import {
  updateSshAuthenticationResponse,
  updateSshPasswordMemory,
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

export function useSshPrompts(windowId: string) {
  const { t } = useTranslation();
  const [queue, setQueue] = useState<SshPromptState[]>([]);
  const submissions = useRef(new Set<string>());
  const activePrompt = queue.length === 0 ? null : queue[0];

  useEffect(() => {
    let acceptingEvents = true;
    const options = { target: { kind: "WebviewWindow" as const, label: windowId } };
    const unlistenAuthentication = listen<SshAuthenticationPromptPayload>(
      "ssh://authentication-prompt",
      (event) => {
        if (!acceptingEvents) return;
        setQueue((current) => enqueueAuthenticationPrompt(current, event.payload));
      },
      options
    );
    const unlistenAuthenticationDismissed = listen<SshPromptDismissedPayload>(
      "ssh://authentication-prompt-dismissed",
      (event) => {
        if (!acceptingEvents) return;
        setQueue((current) => removeSshPrompt(current, event.payload.requestId));
      },
      options
    );
    const unlistenHostKey = listen<SshHostKeyPromptPayload>(
      "ssh://host-key-prompt",
      (event) => {
        if (!acceptingEvents) return;
        setQueue((current) => enqueueHostKeyPrompt(current, event.payload));
      },
      options
    );
    const unlistenHostKeyDismissed = listen<SshPromptDismissedPayload>(
      "ssh://host-key-prompt-dismissed",
      (event) => {
        if (!acceptingEvents) return;
        setQueue((current) => removeSshPrompt(current, event.payload.requestId));
      },
      options
    );
    return () => {
      acceptingEvents = false;
      void unlistenAuthentication.then((stopListening) => stopListening());
      void unlistenAuthenticationDismissed.then((stopListening) => stopListening());
      void unlistenHostKey.then((stopListening) => stopListening());
      void unlistenHostKeyDismissed.then((stopListening) => stopListening());
    };
  }, [windowId]);

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

  const updateRememberPassword = useCallback(
    (remember: boolean) => {
      updateActivePrompt((prompt) =>
        prompt.kind === "authentication"
          ? {
              ...prompt,
              value: updateSshPasswordMemory(prompt.value, remember),
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
      if (submissions.current.has(requestId)) return;
      submissions.current.add(requestId);
      const authenticationResponses =
        activePrompt.kind === "authentication" ? [...activePrompt.value.responses] : null;
      updateActivePrompt((prompt) => setSshPromptSubmission(prompt, true));
      try {
        if (activePrompt.kind === "authentication") {
          await invoke("ssh_authentication_respond", {
            requestId,
            responses: accept ? authenticationResponses : null,
            rememberPassword:
              accept &&
              activePrompt.value.method === "password" &&
              activePrompt.value.rememberPassword,
          });
        } else {
          await invoke("ssh_host_key_respond", { requestId, accept });
        }
        setQueue((current) => removeSshPrompt(current, requestId));
      } catch (error) {
        const code = parseBackendCommandError(error)?.code;
        const prefix =
          activePrompt.kind === "authentication" ? "ssh.auth_prompt" : "ssh.host_key_prompt";
        if (code === `${prefix}_request_not_found` || code === `${prefix}_already_finished`) {
          setQueue((current) => removeSshPrompt(current, requestId));
          return;
        }
        const fallback =
          activePrompt.kind === "authentication"
            ? t("connection.auth_prompt_submit_failed")
            : t("connection.host_key_prompt_submit_failed");
        updateActivePrompt((prompt) =>
          setSshPromptSubmission(prompt, false, translateBackendCommandError(error, t, fallback))
        );
      } finally {
        submissions.current.delete(requestId);
      }
    },
    [activePrompt, t, updateActivePrompt]
  );

  return {
    activePrompt,
    updateResponse,
    updateRememberPassword,
    submit: () => resolve(true),
    cancel: () => resolve(false),
  };
}
