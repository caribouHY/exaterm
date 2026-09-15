import { beforeEach, describe, expect, it, vi } from "vitest";
import { createConnectionAttemptController } from "./connectionAttemptController";
import { createConnectionDependencies, type ConnectionInput } from "./connectionAttemptServices";
import type { ConnectionDialogProps } from "./connectionDialogTypes";
import type { ConnectionInvoke } from "./sshConnectionPreparation";
import { connectionHistoryClient } from "../../features/connection-history/connectionHistoryClient";

vi.mock("../../features/connection-history/connectionHistoryClient", () => ({
  connectionHistoryClient: { record: vi.fn(async () => {}) },
}));
const tick = async () => {
  for (let i = 0; i < 24; i++) await Promise.resolve();
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function input(): ConnectionInput {
  return {
    tab: "ssh",
    selectedProfileIds: { ssh: "", telnet: "" },
    sshProfiles: [],
    ssh: {
      host: "target.invalid",
      port: "22",
      username: "test",
      authMethod: "public_key",
      privateKeyPath: "target-key",
      defaultPrivateKeyPath: "",
      jumpProfileId: "",
      encoding: "utf-8",
      terminalMode: "general",
    },
    telnet: { host: "telnet.invalid", port: "23", encoding: "utf-8", terminalMode: "general" },
    serial: {
      selectedPort: "test-port",
      baudRate: "9600",
      dataBits: "8",
      parity: "none",
      stopBits: "1",
      terminalMode: "general",
    },
  };
}
function setup() {
  const connection = deferred<{ session_id: string }>();
  const call = vi.fn(async (command: string, _args?: Record<string, unknown>): Promise<unknown> => {
    if (command === "config_load") return { terminal: { auto_session_log: true } };
    if (command === "ssh_private_key_requires_passphrase") return true;
    if (command === "ssh_connect") return connection.promise;
    if (command === "logger_start_on_connection") return "test-log";
    if (command === "telnet_connect" || command === "serial_connect") return "session";
    if (command.endsWith("_disconnect")) return;
    throw new Error(`Unexpected test command: ${command}`);
  });
  const onConnect = vi.fn<ConnectionDialogProps["onConnect"]>(async () => {});
  const diagnostics = { start: vi.fn(async (_requestId: string) => {}), stop: vi.fn() };
  const controller = createConnectionAttemptController(
    createConnectionDependencies({
      invoke: call as ConnectionInvoke,
      onConnect,
      diagnostics,
      t: (key) => key,
    })
  );
  return { connection, call, onConnect, diagnostics, controller };
}
const commandCalls = (h: ReturnType<typeof setup>, command: string) =>
  h.call.mock.calls.filter(([name]) => name === command);

beforeEach(() => vi.clearAllMocks());
describe("production connection services with the real controller", () => {
  it("fixes repeated SSH credential submission without losing the successful terminal", async () => {
    const h = setup();
    const pending = h.controller.start("ssh", input());
    await tick();
    const prompt = h.controller.getSnapshot().prompt!;
    h.controller.submitCredential(prompt.requestId, prompt.promptId, "passphrase");
    h.controller.submitCredential(prompt.requestId, prompt.promptId, "duplicate");
    await tick();
    expect(commandCalls(h, "ssh_connect")).toHaveLength(1);
    const options = commandCalls(h, "ssh_connect")[0][1]?.options;
    expect(options).toEqual({
      host: "target.invalid",
      port: 22,
      username: "test",
      password: "",
      authMethod: "public_key",
      privateKeyPath: "target-key",
      keyPassphrase: "passphrase",
      jumpProfileId: null,
      jumpPassword: "",
      jumpKeyPassphrase: "",
      cols: 120,
      rows: 30,
      encoding: "utf-8",
      requestId: prompt.requestId,
    });
    h.connection.resolve({ session_id: "session" });
    await pending;
    expect(h.onConnect).toHaveBeenCalledTimes(1);
    expect(h.onConnect).toHaveBeenCalledWith(
      "ssh",
      "session",
      "test@target.invalid",
      { isLogging: true, filePath: "test-log", startFailed: false },
      "utf-8",
      "general",
      {
        kind: "ssh",
        host: "target.invalid",
        port: 22,
        username: "test",
        auth_method: "public_key",
        private_key_path: "target-key",
        jump_profile_id: null,
      }
    );
    expect(vi.mocked(connectionHistoryClient.record)).toHaveBeenCalledTimes(1);
  });
  it("fixes target and jump credential preparation to the input at start", async () => {
    const h = setup();
    const data = input();
    data.ssh.jumpProfileId = "jump";
    data.sshProfiles = [
      {
        id: "jump",
        connection_type: "ssh",
        host: "jump.invalid",
        username: "jump-user",
        auth_method: "public_key",
        private_key_path: "jump-key",
      },
    ];
    const pending = h.controller.start("ssh", data);
    data.ssh.host = "changed.invalid";
    data.sshProfiles[0].host = "changed-jump.invalid";
    data.sshProfiles[0].auth_method = "password";
    await tick();
    const jump = h.controller.getSnapshot().prompt!;
    expect(jump.host).toBe("jump.invalid");
    h.controller.submitCredential(jump.requestId, jump.promptId, "jump-secret");
    await tick();
    const target = h.controller.getSnapshot().prompt!;
    expect(target.host).toBe("target.invalid");
    h.controller.submitCredential(jump.requestId, jump.promptId, "stale");
    expect(commandCalls(h, "ssh_connect")).toHaveLength(0);
    h.controller.submitCredential(target.requestId, target.promptId, "target-secret");
    await tick();
    expect(commandCalls(h, "ssh_connect")[0][1]).toMatchObject({
      options: {
        host: "target.invalid",
        jumpKeyPassphrase: "jump-secret",
        keyPassphrase: "target-secret",
        jumpPassword: "",
      },
    });
    h.connection.resolve({ session_id: "session" });
    await pending;
    expect(vi.mocked(connectionHistoryClient.record).mock.calls[0][0]).not.toHaveProperty(
      "password"
    );
  });
  for (const authMethod of ["password", "keyboard_interactive", "auto"] as const) {
    it(`keeps backend authentication prompts for ${authMethod}`, async () => {
      const h = setup();
      const data = input();
      data.ssh.authMethod = authMethod;
      data.ssh.privateKeyPath = "";
      const pending = h.controller.start("ssh", data);
      await tick();
      expect(h.controller.getSnapshot().prompt).toBeNull();
      expect(commandCalls(h, "ssh_connect")).toHaveLength(1);
      h.connection.resolve({ session_id: "session" });
      await pending;
    });
  }
  it("allows auto authentication to proceed after key probing fails", async () => {
    const h = setup();
    const original = h.call.getMockImplementation()!;
    h.call.mockImplementation(async (command, args) => {
      if (command === "ssh_private_key_requires_passphrase") throw new Error("key");
      return original(command, args);
    });
    const data = input();
    data.ssh.authMethod = "auto";
    const pending = h.controller.start("ssh", data);
    await tick();
    expect(commandCalls(h, "ssh_connect")).toHaveLength(1);
    h.connection.resolve({ session_id: "session" });
    await pending;
  });
  it("returns to editing after explicit key probing fails", async () => {
    const h = setup();
    const original = h.call.getMockImplementation()!;
    h.call.mockImplementation(async (command, args) => {
      if (command === "ssh_private_key_requires_passphrase") throw new Error("key");
      return original(command, args);
    });
    await h.controller.start("ssh", input());
    expect(h.controller.getSnapshot().status).toBe("editing");
    expect(commandCalls(h, "ssh_connect")).toHaveLength(0);
  });
  it("cancels while the key probe is unresolved", async () => {
    const h = setup();
    const probe = deferred<boolean>();
    const original = h.call.getMockImplementation()!;
    h.call.mockImplementation(async (command, args) =>
      command === "ssh_private_key_requires_passphrase" ? probe.promise : original(command, args)
    );
    const pending = h.controller.start("ssh", input());
    await tick();
    await h.controller.cancel();
    probe.resolve(true);
    await pending;
    expect(commandCalls(h, "ssh_connect")).toHaveLength(0);
    expect(h.controller.getSnapshot().prompt).toBeNull();
  });
  for (const protocol of ["telnet", "serial"] as const) {
    it(`uses the same owner for ${protocol}, logs before registration, and records only network history`, async () => {
      const h = setup();
      const data = input();
      data.tab = protocol;
      await h.controller.start(protocol, data);
      expect(commandCalls(h, `${protocol}_connect`)).toHaveLength(1);
      expect(h.onConnect).toHaveBeenCalledTimes(1);
      const connectOrder = h.call.mock.calls.findIndex(([name]) => name === `${protocol}_connect`);
      const logOrder = h.call.mock.calls.findIndex(
        ([name]) => name === "logger_start_on_connection"
      );
      expect(logOrder).toBeGreaterThan(connectOrder);
      expect(vi.mocked(connectionHistoryClient.record)).toHaveBeenCalledTimes(
        protocol === "telnet" ? 1 : 0
      );
    });
  }
});
