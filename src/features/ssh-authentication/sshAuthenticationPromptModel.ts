export interface SshAuthenticationPromptField {
  prompt: string;
  echo: boolean;
}

export interface SshAuthenticationPromptPayload {
  requestId: string;
  phase: "jump" | "target";
  host: string;
  port: number;
  username: string;
  method: "password" | "keyboard_interactive";
  name: string;
  instructions: string;
  prompts: SshAuthenticationPromptField[];
}

export interface SshAuthenticationPromptState extends SshAuthenticationPromptPayload {
  responses: string[];
  error: string;
  submitting: boolean;
}

export function createSshAuthenticationPromptState(
  payload: SshAuthenticationPromptPayload
): SshAuthenticationPromptState {
  return {
    ...payload,
    responses: payload.prompts.map(() => ""),
    error: "",
    submitting: false,
  };
}

export function enqueueSshAuthenticationPrompt(
  queue: SshAuthenticationPromptState[],
  payload: SshAuthenticationPromptPayload
): SshAuthenticationPromptState[] {
  return [...queue, createSshAuthenticationPromptState(payload)];
}

export function removeSshAuthenticationPrompt(
  queue: SshAuthenticationPromptState[],
  requestId: string
): SshAuthenticationPromptState[] {
  return queue.filter((prompt) => prompt.requestId !== requestId);
}

export function updateSshAuthenticationPrompt(
  queue: SshAuthenticationPromptState[],
  requestId: string,
  update: (prompt: SshAuthenticationPromptState) => SshAuthenticationPromptState
): SshAuthenticationPromptState[] {
  return queue.map((prompt) => (prompt.requestId === requestId ? update(prompt) : prompt));
}

export function updateSshAuthenticationResponse(
  state: SshAuthenticationPromptState,
  index: number,
  value: string
): SshAuthenticationPromptState {
  if (index < 0 || index >= state.responses.length) return state;
  return {
    ...state,
    error: "",
    responses: state.responses.map((response, responseIndex) =>
      responseIndex === index ? value : response
    ),
  };
}
