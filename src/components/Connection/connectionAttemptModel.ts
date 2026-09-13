import { parseBackendCommandError } from "../../features/backend-errors/backendCommandError";
import type { ConnectionType } from "../../types";

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

export type ConnectionAttemptStatus =
  | "editing"
  | "preparing"
  | "connecting"
  | "credential"
  | "cancelling"
  | "finalizing"
  | "finalization_failed";

export interface ConnectionAttemptState {
  connectionType: ConnectionType | null;
  status: ConnectionAttemptStatus;
  requestId: string | null;
  progress: SshConnectionProgressEvent | null;
  error: string;
}

export type ConnectionAttemptAction =
  | { type: "begin"; connectionType: ConnectionType; requestId: string }
  | { type: "connected"; requestId: string }
  | { type: "credential"; requestId: string }
  | { type: "resume"; requestId: string }
  | { type: "progress"; requestId: string; progress: SshConnectionProgressEvent }
  | { type: "cancel"; requestId: string }
  | { type: "cancel_failed"; requestId: string; error: string }
  | { type: "finalize"; requestId: string }
  | { type: "finalize_failed"; requestId: string; error: string }
  | { type: "finish"; requestId: string };

export const initialConnectionAttemptState: ConnectionAttemptState = {
  connectionType: null,
  status: "editing",
  requestId: null,
  progress: null,
  error: "",
};

export function connectionAttemptReducer(
  state: ConnectionAttemptState,
  action: ConnectionAttemptAction
): ConnectionAttemptState {
  if (action.type === "begin") {
    return state.status === "editing"
      ? {
          connectionType: action.connectionType,
          status: action.connectionType === "ssh" ? "preparing" : "connecting",
          requestId: action.requestId,
          progress: null,
          error: "",
        }
      : state;
  }
  if (state.requestId !== action.requestId) return state;

  switch (action.type) {
    case "connected":
      return { ...state, status: "connecting", error: "" };
    case "resume":
      return state.status === "cancelling" || state.status === "credential"
        ? { ...state, status: "connecting", error: "" }
        : state;
    case "credential":
      return { ...state, status: "credential", error: "" };
    case "progress":
      return state.status === "finalizing" || state.status === "finalization_failed"
        ? state
        : { ...state, progress: action.progress };
    case "cancel":
      return state.status === "cancelling" ||
        state.status === "finalizing" ||
        state.status === "finalization_failed"
        ? state
        : { ...state, status: "cancelling", error: "" };
    case "cancel_failed":
      return state.status === "cancelling"
        ? { ...state, status: "connecting", error: action.error }
        : state;
    case "finalize":
      return { ...state, status: "finalizing", error: "" };
    case "finalize_failed":
      return { ...state, status: "finalization_failed", error: action.error };
    case "finish":
      return initialConnectionAttemptState;
  }
}

const CONNECTION_CANCELLATION_CODES = new Set([
  "ssh.connect_cancelled",
  "ssh.auth_prompt_cancelled",
  "ssh.host_key_prompt_cancelled",
  "telnet.connect_cancelled",
  "serial.connect_cancelled",
]);

export function isConnectionCancellation(error: unknown): boolean {
  const parsed = parseBackendCommandError(error);
  return parsed !== null && CONNECTION_CANCELLATION_CODES.has(parsed.code);
}

export function isCurrentConnectionAttempt(
  attempt: { requestId: string } | null,
  requestId: string
): boolean {
  return attempt?.requestId === requestId;
}

export const createConnectionRequestId = () => globalThis.crypto.randomUUID();

export function consumeSshCredential(value: string, clear: () => void): string {
  clear();
  return value;
}
