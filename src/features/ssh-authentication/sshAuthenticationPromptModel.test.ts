import { describe, expect, it } from "vitest";
import {
  createSshAuthenticationPromptState,
  enqueueSshAuthenticationPrompt,
  removeSshAuthenticationPrompt,
  updateSshAuthenticationPrompt,
  updateSshAuthenticationResponse,
  type SshAuthenticationPromptPayload,
} from "./sshAuthenticationPromptModel";

function prompt(
  requestId: string,
  prompts: SshAuthenticationPromptPayload["prompts"]
): SshAuthenticationPromptPayload {
  return {
    requestId,
    phase: "target",
    host: "switch.example.test",
    port: 22,
    username: "admin",
    method: "keyboard_interactive",
    name: "Authentication",
    instructions: "Answer each prompt.",
    prompts,
  };
}

describe("SSH authentication prompt state", () => {
  it("creates fields for multiple visible and hidden responses", () => {
    const state = createSshAuthenticationPromptState(
      prompt("request-1", [
        { prompt: "Password:", echo: false },
        { prompt: "Token:", echo: true },
      ])
    );

    expect(state.responses).toEqual(["", ""]);
    expect(state.prompts.map((field) => field.echo)).toEqual([false, true]);

    const password = updateSshAuthenticationResponse(state, 0, "secret");
    const token = updateSshAuthenticationResponse(password, 1, "123456");
    expect(token.responses).toEqual(["secret", "123456"]);
    expect(state.responses).toEqual(["", ""]);
  });

  it("represents a zero-question request without response fields", () => {
    expect(createSshAuthenticationPromptState(prompt("request-1", [])).responses).toEqual([]);
  });

  it("queues prompts in FIFO order and removes only the completed request", () => {
    const first = prompt("first", [{ prompt: "Password:", echo: false }]);
    const second = {
      ...prompt("second", [{ prompt: "OTP:", echo: false }]),
      phase: "jump" as const,
    };

    const queue = enqueueSshAuthenticationPrompt(enqueueSshAuthenticationPrompt([], first), second);
    expect(queue.map((item) => item.requestId)).toEqual(["first", "second"]);
    expect(removeSshAuthenticationPrompt(queue, "first").map((item) => item.requestId)).toEqual([
      "second",
    ]);
  });

  it("discards entered values when a completed request leaves the queue", () => {
    const state = updateSshAuthenticationResponse(
      createSshAuthenticationPromptState(
        prompt("request-1", [{ prompt: "Password:", echo: false }])
      ),
      0,
      "secret"
    );

    expect(removeSshAuthenticationPrompt([state], "request-1")).toEqual([]);
  });

  it("updates a request by id without changing a later queued prompt", () => {
    const first = createSshAuthenticationPromptState(
      prompt("first", [{ prompt: "Password:", echo: false }])
    );
    const second = createSshAuthenticationPromptState(
      prompt("second", [{ prompt: "OTP:", echo: false }])
    );

    const queue = updateSshAuthenticationPrompt([first, second], "first", (item) => ({
      ...item,
      submitting: true,
    }));
    expect(queue[0].submitting).toBe(true);
    expect(queue[1]).toEqual(second);
  });
});
