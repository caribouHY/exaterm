import { parseBackendCommandError } from "../../features/backend-errors/backendCommandError";

export type SshConnectionProgressPhase =
  | "connecting"
  | "verifying_host_key"
  | "authenticating"
  | "opening_session";
export interface SshConnectionProgressEvent {
  phase: SshConnectionProgressPhase;
  target: "jump" | "target";
}
const cancellationCodes = new Set([
  "ssh.connect_cancelled",
  "ssh.auth_prompt_cancelled",
  "ssh.host_key_prompt_cancelled",
  "telnet.connect_cancelled",
  "serial.connect_cancelled",
]);
export function isConnectionCancellation(error: unknown): boolean {
  const parsed = parseBackendCommandError(error);
  return parsed !== null && cancellationCodes.has(parsed.code);
}
