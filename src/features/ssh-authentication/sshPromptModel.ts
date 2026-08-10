import type { HostKeyCheckResult, HostKeyCheckStatus } from "../../types";
import {
  createSshAuthenticationPromptState,
  type SshAuthenticationPromptPayload,
  type SshAuthenticationPromptState,
} from "./sshAuthenticationPromptModel";

export interface SshHostKeyPromptPayload extends HostKeyCheckResult {
  requestId: string;
  phase: "jump" | "target";
  status: Exclude<HostKeyCheckStatus, "trusted">;
}

export interface SshHostKeyPromptState extends SshHostKeyPromptPayload {
  error: string;
  submitting: boolean;
}

export type SshPromptState =
  | { kind: "authentication"; value: SshAuthenticationPromptState }
  | { kind: "host_key"; value: SshHostKeyPromptState };

export function getHostKeyPromptPresentation(status: SshHostKeyPromptPayload["status"]) {
  return status === "mismatch"
    ? {
        titleKey: "connection.host_key_mismatch.title",
        actionKey: "connection.host_key_replace_connect",
        actionClassName: "btn-danger",
      }
    : {
        titleKey: "connection.host_key_unknown.title",
        actionKey: "connection.host_key_trust_connect",
        actionClassName: "btn-primary",
      };
}

export function enqueueAuthenticationPrompt(
  queue: SshPromptState[],
  payload: SshAuthenticationPromptPayload
): SshPromptState[] {
  return [...queue, { kind: "authentication", value: createSshAuthenticationPromptState(payload) }];
}

export function enqueueHostKeyPrompt(
  queue: SshPromptState[],
  payload: SshHostKeyPromptPayload
): SshPromptState[] {
  return [...queue, { kind: "host_key", value: { ...payload, error: "", submitting: false } }];
}

export function removeSshPrompt(queue: SshPromptState[], requestId: string): SshPromptState[] {
  return queue.filter((prompt) => prompt.value.requestId !== requestId);
}

export function updateSshPrompt(
  queue: SshPromptState[],
  requestId: string,
  update: (prompt: SshPromptState) => SshPromptState
): SshPromptState[] {
  return queue.map((prompt) => (prompt.value.requestId === requestId ? update(prompt) : prompt));
}

export function setSshPromptSubmission(
  prompt: SshPromptState,
  submitting: boolean,
  error = ""
): SshPromptState {
  if (prompt.kind === "authentication") {
    return { ...prompt, value: { ...prompt.value, error, submitting } };
  }
  return { ...prompt, value: { ...prompt.value, error, submitting } };
}
