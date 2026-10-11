import type { ConnectionType } from "../../types";
import type { SshCredentialPrompt } from "./connectionDialogTypes";

export type AttemptStatus =
  | "editing"
  | "preparing"
  | "credential"
  | "connecting"
  | "cancelling"
  | "finalizing"
  | "finalization_failed"
  | "complete";
export interface AttemptSnapshot {
  status: AttemptStatus;
  target: string;
  requestId: string | null;
  connectionType: ConnectionType | null;
  prompt: SshCredentialPrompt | null;
  error: string;
  cancelError: string;
}
export const initialAttemptSnapshot: AttemptSnapshot = {
  target: "",
  status: "editing",
  requestId: null,
  connectionType: null,
  prompt: null,
  error: "",
  cancelError: "",
};
export type AttemptEvent =
  | { type: "start"; target: string; requestId: string; connectionType: ConnectionType }
  | {
      type: "transition";
      requestId: string;
      status: AttemptStatus;
      prompt?: SshCredentialPrompt;
      error?: string;
      cancelError?: string;
    };

const transitions: Record<AttemptStatus, readonly AttemptStatus[]> = {
  editing: [],
  preparing: ["credential", "connecting", "editing"],
  credential: ["preparing", "editing"],
  connecting: ["cancelling", "finalizing", "editing"],
  cancelling: ["connecting", "finalizing", "editing"],
  finalizing: ["finalization_failed", "complete"],
  finalization_failed: ["finalizing"],
  complete: [],
};

export function reduceAttempt(state: AttemptSnapshot, event: AttemptEvent): AttemptSnapshot {
  if (event.type === "start")
    return state.status === "editing"
      ? {
          ...initialAttemptSnapshot,
          status: "preparing",
          target: event.target,
          requestId: event.requestId,
          connectionType: event.connectionType,
        }
      : state;
  if (state.requestId !== event.requestId || !transitions[state.status].includes(event.status))
    return state;
  if (
    event.status === "credential" &&
    (!event.prompt || event.prompt.requestId !== state.requestId)
  )
    return state;
  return {
    ...state,
    status: event.status,
    prompt: event.prompt ?? null,
    error: event.error ?? "",
    cancelError: event.cancelError ?? "",
  };
}
