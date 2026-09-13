import { describe, expect, it } from "vitest";
import {
  connectionAttemptReducer,
  consumeSshCredential,
  initialConnectionAttemptState,
  isConnectionCancellation,
  isCurrentConnectionAttempt,
} from "./connectionAttemptModel";

describe("connectionAttemptModel", () => {
  it("starts each protocol with its appropriate preparation state", () => {
    expect(
      connectionAttemptReducer(initialConnectionAttemptState, {
        type: "begin",
        connectionType: "ssh",
        requestId: "ssh-1",
      })
    ).toMatchObject({ connectionType: "ssh", status: "preparing", requestId: "ssh-1" });

    for (const connectionType of ["telnet", "serial"] as const) {
      expect(
        connectionAttemptReducer(initialConnectionAttemptState, {
          type: "begin",
          connectionType,
          requestId: `${connectionType}-1`,
        })
      ).toMatchObject({ connectionType, status: "connecting", requestId: `${connectionType}-1` });
    }
  });

  it("keeps one active attempt through credential input and connection", () => {
    const preparing = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "ssh",
      requestId: "request-1",
    });
    const waiting = connectionAttemptReducer(preparing, {
      type: "credential",
      requestId: "request-1",
    });
    const connecting = connectionAttemptReducer(waiting, {
      type: "resume",
      requestId: "request-1",
    });

    expect(waiting.status).toBe("credential");
    expect(connecting.status).toBe("connecting");
    expect(
      connectionAttemptReducer(waiting, {
        type: "begin",
        connectionType: "ssh",
        requestId: "request-2",
      })
    ).toBe(waiting);
  });

  it("ignores every update from an old request", () => {
    const connecting = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "ssh",
      requestId: "current",
    });
    const oldActions = [
      { type: "cancel", requestId: "old" },
      { type: "finalize", requestId: "old" },
      { type: "finish", requestId: "old" },
      {
        type: "progress",
        requestId: "old",
        progress: { phase: "authenticating", target: "target" },
      },
    ] as const;

    for (const action of oldActions) {
      expect(connectionAttemptReducer(connecting, action)).toBe(connecting);
    }
  });

  it("disables cancellation while finalizing and retains retry errors", () => {
    const connecting = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "telnet",
      requestId: "request-1",
    });
    const finalizing = connectionAttemptReducer(connecting, {
      type: "finalize",
      requestId: "request-1",
    });
    const failed = connectionAttemptReducer(finalizing, {
      type: "finalize_failed",
      requestId: "request-1",
      error: "registration failed",
    });

    expect(finalizing.status).toBe("finalizing");
    expect(failed).toMatchObject({ status: "finalization_failed", error: "registration failed" });
    expect(connectionAttemptReducer(finalizing, { type: "resume", requestId: "request-1" })).toBe(
      finalizing
    );
    expect(connectionAttemptReducer(finalizing, { type: "cancel", requestId: "request-1" })).toBe(
      finalizing
    );
  });

  it("moves through cancellation and reports a rejected cancellation", () => {
    const connecting = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "serial",
      requestId: "request-1",
    });
    const cancelling = connectionAttemptReducer(connecting, {
      type: "cancel",
      requestId: "request-1",
    });
    const failed = connectionAttemptReducer(cancelling, {
      type: "cancel_failed",
      requestId: "request-1",
      error: "cancel failed",
    });

    expect(cancelling.status).toBe("cancelling");
    expect(failed).toMatchObject({ status: "connecting", error: "cancel failed" });
  });

  it("recognizes all intentional connection cancellation codes", () => {
    for (const code of [
      "ssh.connect_cancelled",
      "ssh.auth_prompt_cancelled",
      "ssh.host_key_prompt_cancelled",
      "telnet.connect_cancelled",
      "serial.connect_cancelled",
    ]) {
      expect(isConnectionCancellation({ code, message: "cancelled" })).toBe(true);
    }
    expect(isConnectionCancellation({ code: "ssh.connection_failed", message: "failed" })).toBe(
      false
    );
  });

  it("identifies the current attempt after it has been cleared", () => {
    expect(isCurrentConnectionAttempt({ requestId: "current" }, "current")).toBe(true);
    expect(isCurrentConnectionAttempt({ requestId: "old" }, "current")).toBe(false);
    expect(isCurrentConnectionAttempt(null, "current")).toBe(false);
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
