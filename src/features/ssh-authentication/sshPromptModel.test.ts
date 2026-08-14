import { describe, expect, it } from "vitest";
import type { SshAuthenticationPromptPayload } from "./sshAuthenticationPromptModel";
import {
  enqueueAuthenticationPrompt,
  enqueueHostKeyPrompt,
  getHostKeyPromptPresentation,
  removeSshPrompt,
  setSshPromptSubmission,
  updateSshPrompt,
  type SshHostKeyPromptPayload,
} from "./sshPromptModel";

const authenticationPrompt: SshAuthenticationPromptPayload = {
  requestId: "authentication",
  phase: "target",
  host: "router.example.test",
  port: 22,
  username: "admin",
  method: "password",
  name: "",
  instructions: "",
  prompts: [{ prompt: "", echo: false }],
};

const hostKeyPrompt: SshHostKeyPromptPayload = {
  requestId: "host-key",
  phase: "jump",
  status: "mismatch",
  host: "jump.example.test",
  port: 22,
  algorithm: "ssh-ed25519",
  fingerprint: "presented",
  known_fingerprint: "stored",
};

describe("SSH prompt queue", () => {
  it("preserves arrival order across host-key and authentication prompts", () => {
    const queue = enqueueAuthenticationPrompt(
      enqueueHostKeyPrompt([], hostKeyPrompt),
      authenticationPrompt
    );

    expect(queue.map((prompt) => prompt.kind)).toEqual(["host_key", "authentication"]);
    expect(queue.map((prompt) => prompt.value.requestId)).toEqual(["host-key", "authentication"]);
  });

  it("removes only the dismissed request", () => {
    const queue = enqueueHostKeyPrompt(
      enqueueAuthenticationPrompt([], authenticationPrompt),
      hostKeyPrompt
    );

    expect(removeSshPrompt(queue, "authentication").map((prompt) => prompt.kind)).toEqual([
      "host_key",
    ]);
  });

  it("updates the selected host-key prompt without changing queued authentication", () => {
    const queue = enqueueAuthenticationPrompt(
      enqueueHostKeyPrompt([], hostKeyPrompt),
      authenticationPrompt
    );
    const updated = updateSshPrompt(queue, "host-key", (prompt) =>
      setSshPromptSubmission(prompt, true)
    );

    expect(updated[0].value.submitting).toBe(true);
    expect(updated[1]).toEqual(queue[1]);
  });

  it("uses trust styling for unknown keys and replacement styling for mismatches", () => {
    expect(getHostKeyPromptPresentation("unknown")).toEqual({
      titleKey: "connection.host_key_unknown.title",
      actionKey: "connection.host_key_trust_connect",
      actionClassName: "btn-primary",
    });
    expect(getHostKeyPromptPresentation("mismatch")).toEqual({
      titleKey: "connection.host_key_mismatch.title",
      actionKey: "connection.host_key_replace_connect",
      actionClassName: "btn-danger",
    });
  });

  it("clears authentication responses when submission starts", () => {
    const prompt = enqueueAuthenticationPrompt([], authenticationPrompt)[0];
    if (prompt.kind !== "authentication") throw new Error("authentication prompt expected");
    const populated = {
      ...prompt,
      value: { ...prompt.value, responses: ["secret"] },
    };

    const submitting = setSshPromptSubmission(populated, true);

    expect(submitting.kind).toBe("authentication");
    if (submitting.kind === "authentication") {
      expect(submitting.value.responses).toEqual([""]);
      expect(submitting.value.submitting).toBe(true);
    }
  });
});
