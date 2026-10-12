import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useConnectionDialogShortcuts } from "./useConnectionDialogShortcuts";
import { createConnectionAttemptController } from "./connectionAttemptController";
import type { AttemptStatus } from "./connectionAttemptState";

const effects = vi.hoisted(() => ({ cleanups: [] as (() => void)[] }));
vi.mock("react", () => ({
  useEffect: (effect: () => void | (() => void)) => {
    const cleanup = effect();
    if (cleanup) effects.cleanups.push(cleanup);
  },
}));

function mount(overrides: Partial<Parameters<typeof useConnectionDialogShortcuts>[0]> = {}) {
  const params = {
    attemptStatus: "editing" as AttemptStatus,
    enabled: true,
    canConnect: true,
    credentialPrompt: null,
    onClose: vi.fn(),
    onCloseCredentialPrompt: vi.fn(),
    onCancelConnection: vi.fn(),
    onConnect: vi.fn(),
    ...overrides,
  };
  useConnectionDialogShortcuts(params);
  return params;
}

function keydown(
  key = "Escape",
  options: { repeat?: boolean; ctrlKey?: boolean; consumed?: boolean } = {}
) {
  const event = new Event("keydown", { cancelable: true });
  Object.assign(event, { key, repeat: false, ctrlKey: false, metaKey: false, ...options });
  if (options.consumed) event.preventDefault();
  window.dispatchEvent(event);
  return event;
}

beforeEach(() => {
  vi.stubGlobal("window", new EventTarget());
});

afterEach(() => {
  for (const cleanup of effects.cleanups.splice(0)) cleanup();
  vi.unstubAllGlobals();
});

describe("connection dialog shortcuts", () => {
  it("closes the editing dialog without cancelling a connection", () => {
    const params = mount();
    expect(keydown().defaultPrevented).toBe(true);
    expect(params.onClose).toHaveBeenCalledOnce();
    expect(params.onCancelConnection).not.toHaveBeenCalled();
  });

  it.each(["preparing", "connecting"] as const)(
    "cancels %s without closing the dialog",
    (attemptStatus) => {
      const params = mount({ attemptStatus });
      keydown();
      expect(params.onCancelConnection).toHaveBeenCalledOnce();
      expect(params.onClose).not.toHaveBeenCalled();
    }
  );

  it("prioritizes the pre-connection credential prompt", () => {
    const params = mount({
      attemptStatus: "credential",
      credentialPrompt: {
        phase: "target",
        host: "example.invalid",
        port: 22,
        username: "test",
        authMethod: "password",
        privateKeyPath: "",
        requestId: "request",
        promptId: "prompt",
      },
    });
    keydown();
    expect(params.onCloseCredentialPrompt).toHaveBeenCalledOnce();
    expect(params.onCancelConnection).not.toHaveBeenCalled();
    expect(params.onClose).not.toHaveBeenCalled();
  });

  it.each(["cancelling", "finalizing", "finalization_failed", "complete"] as const)(
    "ignores Escape during %s",
    (attemptStatus) => {
      const params = mount({ attemptStatus });
      keydown();
      expect(params.onClose).not.toHaveBeenCalled();
      expect(params.onCancelConnection).not.toHaveBeenCalled();
    }
  );

  it.each(["editing", "preparing", "connecting"] as const)(
    "ignores held Escape during %s",
    (attemptStatus) => {
      const params = mount({ attemptStatus });
      keydown("Escape", { repeat: true });
      expect(params.onClose).not.toHaveBeenCalled();
      expect(params.onCancelConnection).not.toHaveBeenCalled();
    }
  );

  it("leaves Escape and submit to the foreground SSH prompt", () => {
    const params = mount({ enabled: false });
    expect(keydown().defaultPrevented).toBe(false);
    expect(keydown("Enter", { ctrlKey: true }).defaultPrevented).toBe(false);
    expect(params.onClose).not.toHaveBeenCalled();
    expect(params.onCancelConnection).not.toHaveBeenCalled();
    expect(params.onConnect).not.toHaveBeenCalled();
  });

  it("ignores a key already consumed by another control", () => {
    const params = mount();
    keydown("Escape", { consumed: true });
    expect(params.onClose).not.toHaveBeenCalled();
  });

  it("preserves Ctrl+Enter submission only for a connectable editing form", () => {
    const params = mount();
    keydown("Enter", { ctrlKey: true });
    expect(params.onConnect).toHaveBeenCalledOnce();
  });

  it.each(["editing", "connecting"] as const)(
    "does not submit an unavailable form during %s",
    (attemptStatus) => {
      const params = mount({ attemptStatus, canConnect: false });
      keydown("Enter", { ctrlKey: true });
      expect(params.onConnect).not.toHaveBeenCalled();
    }
  );

  it("removes the listener on disposal", () => {
    const params = mount();
    for (const cleanup of effects.cleanups.splice(0)) cleanup();
    keydown();
    expect(params.onClose).not.toHaveBeenCalled();
  });

  it("routes rapid Escape presses through the real controller once before React updates", async () => {
    let rejectConnection!: (error: unknown) => void;
    const connection = new Promise<string>((_resolve, reject) => {
      rejectConnection = reject;
    });
    const cancel = vi.fn(async () => true);
    const register = vi.fn(async () => {});
    const disconnect = vi.fn(async () => {});
    const controller = createConnectionAttemptController({
      createRequestId: () => "request",
      prepare: async () => ({
        type: "ssh" as const,
        connect: () => connection,
        startLog: async () => ({ isLogging: false, filePath: null, startFailed: false }),
        register,
        recordHistory: async () => {},
      }),
      cancel,
      disconnect,
      stopDiagnostics: () => {},
      errorMessage: () => "safe error",
      isCancellation: () => true,
    });
    const pending = controller.start("ssh", {});
    await Promise.resolve();
    expect(controller.getSnapshot().status).toBe("connecting");
    const params = mount({
      attemptStatus: "connecting",
      onCancelConnection: () => {
        void controller.cancel();
      },
    });
    keydown();
    keydown();
    expect(cancel).toHaveBeenCalledOnce();
    expect(controller.getSnapshot().status).toBe("cancelling");
    rejectConnection(new Error("cancelled"));
    await pending;
    expect(controller.getSnapshot().status).toBe("editing");
    expect(params.onClose).not.toHaveBeenCalled();
    expect(register).not.toHaveBeenCalled();
    expect(disconnect).not.toHaveBeenCalled();
  });
});
