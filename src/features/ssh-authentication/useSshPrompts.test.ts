import { beforeEach, describe, expect, it, vi } from "vitest";
import type { EffectCallback, SetStateAction } from "react";
import type { SshPromptState } from "./sshPromptModel";
import { useSshPrompts } from "./useSshPrompts";

interface HookWindow {
  queue: SshPromptState[];
  submissions: { current: Set<string> };
  mounted: boolean;
  cleanup?: () => void;
}

const runtime = vi.hoisted(() => ({
  current: null as HookWindow | null,
  listeners: [] as {
    event: string;
    target: { kind: string; label?: string };
    receive: (event: { payload: unknown }) => void;
    stop: ReturnType<typeof vi.fn>;
  }[],
  invoke: vi.fn(),
}));

function currentWindow(): HookWindow {
  const window = runtime.current;
  if (window === null) throw new Error("The SSH prompt hook has no current window");
  return window;
}

vi.mock("react", () => ({
  useState: () => {
    const window = currentWindow();
    return [
      window.queue,
      (update: SetStateAction<SshPromptState[]>) => {
        window.queue = typeof update === "function" ? update(window.queue) : update;
      },
    ];
  },
  useRef: () => currentWindow().submissions,
  useCallback: (callback: unknown) => callback,
  useEffect: (effect: EffectCallback) => {
    const window = currentWindow();
    if (!window.mounted) {
      const cleanup = effect();
      if (typeof cleanup === "function") window.cleanup = cleanup;
    }
  },
}));

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => runtime.invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (
    event: string,
    receive: (event: { payload: unknown }) => void,
    options?: { target: { kind: string; label?: string } }
  ) => {
    const stop = vi.fn();
    runtime.listeners.push({ event, receive, target: options?.target ?? { kind: "Any" }, stop });
    return stop;
  },
}));

function mount(windowId: string) {
  const window: HookWindow = { queue: [], submissions: { current: new Set() }, mounted: false };
  const render = () => {
    runtime.current = window;
    const result = useSshPrompts(windowId);
    window.mounted = true;
    return result;
  };
  render();
  return { window, render };
}

function emit(event: string, payload: unknown, windowId = "owner") {
  for (const listener of runtime.listeners) {
    if (
      listener.event === event &&
      (listener.target.kind === "Any" || listener.target.label === windowId)
    ) {
      listener.receive({ payload });
    }
  }
}

function show(kind: "authentication" | "host-key", requestId = "first", windowId = "owner") {
  emit(
    `ssh://${kind}-prompt`,
    {
      requestId,
      phase: "target",
      host: "synthetic.invalid",
      port: 22,
      username: "synthetic",
      method: "password",
      name: "",
      instructions: "",
      prompts: [{ prompt: "", echo: false }],
      status: "unknown",
      algorithm: "ssh-ed25519",
      fingerprint: "synthetic",
    },
    windowId
  );
}

function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  runtime.listeners = [];
  runtime.invoke.mockReset().mockResolvedValue(undefined);
});

describe.each(["authentication", "host-key"] as const)("%s prompt ownership", (kind) => {
  it("delivers prompts and dismissals only to the requested window", () => {
    const other = mount("other");
    const owner = mount("owner");
    expect(runtime.listeners).toHaveLength(8);
    expect(runtime.listeners.every((listener) => listener.target.kind === "WebviewWindow")).toBe(
      true
    );
    show(kind);
    show(kind, "second", "other");
    expect(owner.render().activePrompt?.value.requestId).toBe("first");
    expect(other.render().activePrompt?.value.requestId).toBe("second");
    emit(`ssh://${kind}-prompt-dismissed`, { requestId: "first" });
    expect(owner.render().activePrompt).toBeNull();
    expect(other.render().activePrompt?.value.requestId).toBe("second");
  });

  it.each(["submit", "cancel"] as const)(
    "clears only the answered request on %s",
    async (action) => {
      const owner = mount("owner");
      show(kind);
      show(kind, "second");
      const prompt = owner.render();
      if (action === "submit") {
        await prompt.submit();
      } else {
        await prompt.cancel();
      }
      expect(owner.render().activePrompt?.value.requestId).toBe("second");
      expect(runtime.invoke).toHaveBeenCalledWith(
        kind === "authentication" ? "ssh_authentication_respond" : "ssh_host_key_respond",
        kind === "authentication"
          ? {
              requestId: "first",
              responses: action === "submit" ? [""] : null,
              rememberPassword: false,
            }
          : { requestId: "first", accept: action === "submit" }
      );
    }
  );

  it.each([true, false])("handles dismissal before invoke completion: %s", async (dismissFirst) => {
    const owner = mount("owner");
    show(kind);
    const response = deferred();
    runtime.invoke.mockReturnValueOnce(response.promise);
    const submitting = owner.render().submit();
    show(kind, "second");
    if (dismissFirst) emit(`ssh://${kind}-prompt-dismissed`, { requestId: "first" });
    response.resolve();
    await submitting;
    if (!dismissFirst) emit(`ssh://${kind}-prompt-dismissed`, { requestId: "first" });
    expect(owner.render().activePrompt?.value.requestId).toBe("second");
  });

  it.each(["request_not_found", "already_finished"])(
    "removes a stale prompt on %s",
    async (reason) => {
      const owner = mount("owner");
      show(kind);
      const prefix = kind === "authentication" ? "ssh.auth_prompt" : "ssh.host_key_prompt";
      runtime.invoke.mockRejectedValueOnce({ code: `${prefix}_${reason}`, message: "finished" });
      await owner.render().cancel();
      expect(owner.render().activePrompt).toBeNull();
    }
  );

  it("keeps retryable errors and allows another submission", async () => {
    const owner = mount("owner");
    show(kind);
    runtime.invoke.mockRejectedValueOnce({
      code: "ssh.auth_prompt_response_mismatch",
      message: "invalid",
    });
    await owner.render().submit();
    expect(owner.render().activePrompt?.value.error).toBe(
      "backend_errors.ssh_auth_prompt_response_mismatch"
    );
    expect(owner.render().activePrompt?.value.submitting).toBe(false);
    await owner.render().cancel();
    expect(owner.render().activePrompt).toBeNull();
  });

  it("does not apply late errors to the next prompt", async () => {
    const owner = mount("owner");
    show(kind);
    const response = deferred();
    runtime.invoke.mockReturnValueOnce(response.promise);
    const submitting = owner.render().submit();
    emit(`ssh://${kind}-prompt-dismissed`, { requestId: "first" });
    show(kind, "second");
    response.reject("temporary failure");
    await submitting;
    expect(owner.render().activePrompt?.value.requestId).toBe("second");
    expect(owner.render().activePrompt?.value.error).toBe("");
  });

  it("consumes rapid submit and cancel calls synchronously", async () => {
    const owner = mount("owner");
    show(kind);
    const response = deferred();
    runtime.invoke.mockReturnValue(response.promise);
    const rendered = owner.render();
    const submitting = rendered.submit();
    await rendered.cancel();
    expect(runtime.invoke).toHaveBeenCalledTimes(1);
    response.resolve();
    await submitting;
    expect(owner.render().activePrompt).toBeNull();
  });

  it("ignores callbacks after disposal and unregisters asynchronously registered listeners", async () => {
    const owner = mount("owner");
    const cleanup = owner.window.cleanup;
    if (!cleanup) throw new Error("The SSH prompt hook did not register cleanup");
    cleanup();
    show(kind);
    await Promise.resolve();
    expect(owner.render().activePrompt).toBeNull();
    expect(runtime.listeners.every((listener) => listener.stop.mock.calls.length === 1)).toBe(true);
  });
});
