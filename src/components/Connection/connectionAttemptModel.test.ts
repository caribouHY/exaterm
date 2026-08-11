import { describe, expect, it } from "vitest";
import {
  connectionAttemptReducer,
  initialConnectionAttemptState,
  isConnectionCancellation,
  isCurrentConnectionAttempt,
} from "./connectionAttemptModel";

describe("connectionAttemptModel", () => {
  it("starts Telnet and Serial attempts", () => {
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

  it("moves through cancellation and resumes after a rejected cancellation", () => {
    const connecting = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "telnet",
      requestId: "request-1",
    });
    const cancelling = connectionAttemptReducer(connecting, {
      type: "cancel",
      requestId: "request-1",
    });
    const resumed = connectionAttemptReducer(cancelling, {
      type: "resume",
      requestId: "request-1",
    });

    expect(cancelling.status).toBe("cancelling");
    expect(resumed.status).toBe("connecting");
  });

  it("reports cancellation command failures without leaving the progress view", () => {
    const connecting = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "serial",
      requestId: "request-1",
    });
    const failed = connectionAttemptReducer(connecting, {
      type: "cancel_failed",
      requestId: "request-1",
      error: "cancel failed",
    });

    expect(failed).toMatchObject({ status: "connecting", cancelError: "cancel failed" });
  });

  it("ignores completion and cancellation updates from an old request", () => {
    const connecting = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "telnet",
      requestId: "current",
    });

    expect(connectionAttemptReducer(connecting, { type: "finish", requestId: "old" })).toBe(
      connecting
    );
    expect(connectionAttemptReducer(connecting, { type: "cancel", requestId: "old" })).toBe(
      connecting
    );
  });

  it("returns to a clean editing state after completion", () => {
    const connecting = connectionAttemptReducer(initialConnectionAttemptState, {
      type: "begin",
      connectionType: "serial",
      requestId: "request-1",
    });

    expect(
      connectionAttemptReducer(connecting, { type: "finish", requestId: "request-1" })
    ).toEqual(initialConnectionAttemptState);
  });

  it("recognizes Telnet and Serial cancellation codes", () => {
    for (const code of ["telnet.connect_cancelled", "serial.connect_cancelled"]) {
      expect(isConnectionCancellation({ code, message: "cancelled" })).toBe(true);
    }
    expect(isConnectionCancellation({ code: "telnet.connect_failed", message: "failed" })).toBe(
      false
    );
  });

  it("identifies the current attempt without treating a cleared attempt as current", () => {
    expect(isCurrentConnectionAttempt({ requestId: "current" }, "current")).toBe(true);
    expect(isCurrentConnectionAttempt({ requestId: "old" }, "current")).toBe(false);
    expect(isCurrentConnectionAttempt(null, "current")).toBe(false);
  });
});
