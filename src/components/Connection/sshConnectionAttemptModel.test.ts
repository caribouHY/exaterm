import { describe, expect, it } from "vitest";
import {
  consumeSshCredential,
  initialSshConnectionAttemptState,
  isSshConnectionCancellation,
  sshConnectionAttemptReducer,
} from "./sshConnectionAttemptModel";

describe("sshConnectionAttemptModel", () => {
  it("moves from editing through preparation and connection", () => {
    const preparing = sshConnectionAttemptReducer(initialSshConnectionAttemptState, {
      type: "begin",
    });
    const connecting = sshConnectionAttemptReducer(preparing, {
      type: "started",
      requestId: "request-1",
    });

    expect(preparing.status).toBe("preparing");
    expect(connecting).toMatchObject({ status: "connecting", requestId: "request-1" });
  });

  it("ignores progress from an old request", () => {
    const connecting = sshConnectionAttemptReducer(initialSshConnectionAttemptState, {
      type: "started",
      requestId: "current",
    });
    const unchanged = sshConnectionAttemptReducer(connecting, {
      type: "progress",
      requestId: "old",
      progress: { phase: "authenticating", target: "target" },
    });

    expect(unchanged).toBe(connecting);
  });

  it("keeps repeated cancellation idempotent", () => {
    const connecting = sshConnectionAttemptReducer(initialSshConnectionAttemptState, {
      type: "started",
      requestId: "request-1",
    });
    const cancelling = sshConnectionAttemptReducer(connecting, { type: "cancel" });

    expect(sshConnectionAttemptReducer(cancelling, { type: "cancel" })).toBe(cancelling);
  });

  it("suppresses a second connection start while an attempt is active", () => {
    const preparing = sshConnectionAttemptReducer(initialSshConnectionAttemptState, {
      type: "begin",
    });

    expect(sshConnectionAttemptReducer(preparing, { type: "begin" })).toBe(preparing);
  });

  it("moves through cancellation and reports cancellation request failure", () => {
    const connecting = sshConnectionAttemptReducer(initialSshConnectionAttemptState, {
      type: "started",
      requestId: "request-1",
    });
    const cancelling = sshConnectionAttemptReducer(connecting, { type: "cancel" });
    const resumed = sshConnectionAttemptReducer(cancelling, {
      type: "cancel_failed",
      error: "cancel failed",
    });

    expect(cancelling.status).toBe("cancelling");
    expect(resumed).toMatchObject({ status: "connecting", cancelError: "cancel failed" });
  });

  it("returns to a clean editing state after completion", () => {
    const credential = sshConnectionAttemptReducer(
      { ...initialSshConnectionAttemptState, status: "credential", requestId: "request-1" },
      { type: "finish" }
    );

    expect(credential).toEqual(initialSshConnectionAttemptState);
  });

  it("recognizes every intentional SSH cancellation code", () => {
    for (const code of [
      "ssh.connect_cancelled",
      "ssh.auth_prompt_cancelled",
      "ssh.host_key_prompt_cancelled",
    ]) {
      expect(isSshConnectionCancellation({ code, message: "cancelled" })).toBe(true);
    }
    expect(isSshConnectionCancellation({ code: "ssh.connection_failed", message: "failed" })).toBe(
      false
    );
  });

  it("clears a credential before handing its local snapshot to the caller", () => {
    let storedCredential = "secret";

    const credential = consumeSshCredential(storedCredential, () => {
      storedCredential = "";
    });

    expect(credential).toBe("secret");
    expect(storedCredential).toBe("");
  });
});
