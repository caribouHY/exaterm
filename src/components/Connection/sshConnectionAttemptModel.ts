import { parseBackendCommandError } from "../../features/backend-errors/backendCommandError";

export type SshConnectionProgressPhase =
  | "connecting"
  | "verifying_host_key"
  | "authenticating"
  | "opening_session";

export type SshConnectionProgressTarget = "jump" | "target";

export interface SshConnectionProgressEvent {
  phase: SshConnectionProgressPhase;
  target: SshConnectionProgressTarget;
}

export type SshConnectionAttemptStatus =
  | "editing"
  | "preparing"
  | "connecting"
  | "credential"
  | "cancelling";

export interface SshConnectionAttemptState {
  status: SshConnectionAttemptStatus;
  requestId: string | null;
  progress: SshConnectionProgressEvent | null;
  cancelError: string;
}

export type SshConnectionAttemptAction =
  | { type: "begin" }
  | { type: "started"; requestId: string }
  | { type: "credential" }
  | { type: "resume" }
  | { type: "progress"; requestId: string; progress: SshConnectionProgressEvent }
  | { type: "cancel" }
  | { type: "cancel_failed"; error: string }
  | { type: "finish" };

export const initialSshConnectionAttemptState: SshConnectionAttemptState = {
  status: "editing",
  requestId: null,
  progress: null,
  cancelError: "",
};

export function sshConnectionAttemptReducer(
  state: SshConnectionAttemptState,
  action: SshConnectionAttemptAction
): SshConnectionAttemptState {
  switch (action.type) {
    case "begin":
      return state.status === "editing"
        ? { ...initialSshConnectionAttemptState, status: "preparing" }
        : state;
    case "started":
      return { ...state, status: "connecting", requestId: action.requestId, cancelError: "" };
    case "credential":
      return { ...state, status: "credential", cancelError: "" };
    case "resume":
      return { ...state, status: "connecting", cancelError: "" };
    case "progress":
      return state.requestId === action.requestId ? { ...state, progress: action.progress } : state;
    case "cancel":
      return state.status === "cancelling"
        ? state
        : { ...state, status: "cancelling", cancelError: "" };
    case "cancel_failed":
      return { ...state, status: "connecting", cancelError: action.error };
    case "finish":
      return initialSshConnectionAttemptState;
  }
}

export function isCurrentSshConnectionAttempt(
  currentRequestId: string | null,
  requestId: string
): boolean {
  return currentRequestId === requestId;
}

const SSH_CANCELLATION_CODES = new Set([
  "ssh.connect_cancelled",
  "ssh.auth_prompt_cancelled",
  "ssh.host_key_prompt_cancelled",
]);

export function isSshConnectionCancellation(error: unknown): boolean {
  const parsed = parseBackendCommandError(error);
  return parsed !== null && SSH_CANCELLATION_CODES.has(parsed.code);
}

export function consumeSshCredential(value: string, clear: () => void): string {
  clear();
  return value;
}
