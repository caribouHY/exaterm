import { parseBackendCommandError } from "../../features/backend-errors/backendCommandError";
import type { ConnectionType } from "../../types";

export type BasicConnectionType = Exclude<ConnectionType, "ssh">;
export type ConnectionAttemptStatus = "editing" | "connecting" | "cancelling";

export interface ConnectionAttemptState {
  connectionType: BasicConnectionType | null;
  status: ConnectionAttemptStatus;
  requestId: string | null;
  cancelError: string;
}

export type ConnectionAttemptAction =
  | { type: "begin"; connectionType: BasicConnectionType; requestId: string }
  | { type: "cancel"; requestId: string }
  | { type: "resume"; requestId: string }
  | { type: "cancel_failed"; requestId: string; error: string }
  | { type: "finish"; requestId: string };

export const initialConnectionAttemptState: ConnectionAttemptState = {
  connectionType: null,
  status: "editing",
  requestId: null,
  cancelError: "",
};

export function connectionAttemptReducer(
  state: ConnectionAttemptState,
  action: ConnectionAttemptAction
): ConnectionAttemptState {
  if (action.type === "begin") {
    return state.status === "editing"
      ? {
          connectionType: action.connectionType,
          status: "connecting",
          requestId: action.requestId,
          cancelError: "",
        }
      : state;
  }
  if (state.requestId !== action.requestId) return state;

  switch (action.type) {
    case "cancel":
      return state.status === "cancelling"
        ? state
        : { ...state, status: "cancelling", cancelError: "" };
    case "resume":
      return { ...state, status: "connecting", cancelError: "" };
    case "cancel_failed":
      return { ...state, status: "connecting", cancelError: action.error };
    case "finish":
      return initialConnectionAttemptState;
  }
}

const CONNECTION_CANCELLATION_CODES = new Set([
  "telnet.connect_cancelled",
  "serial.connect_cancelled",
]);

export function isConnectionCancellation(error: unknown): boolean {
  const parsed = parseBackendCommandError(error);
  return parsed !== null && CONNECTION_CANCELLATION_CODES.has(parsed.code);
}

export const createConnectionRequestId = () => globalThis.crypto.randomUUID();
