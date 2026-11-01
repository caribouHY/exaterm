import { describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../../types";
import { normalizeShortcutConfig } from "../../features/shortcuts/shortcutModel";
import { createSettingsSaveController } from "./settingsSaveController";
import { normalizeExternalControlConfig } from "./settingsModel";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
const tick = async () => {
  for (let i = 0; i < 12; i++) await Promise.resolve();
};
function fixture() {
  return normalizeExternalControlConfig({
    language: "en",
    config_version: 1,
    terminal: {
      font_size: 14,
      font_family: "initial",
      scrollback: 1000,
      cursor_style: "block",
      auto_session_log: false,
      log_format: "display",
      include_log_header: false,
    },
    ai: {
      default_provider: "OpenAi",
      default_model: "",
      azure_openai_enabled: false,
      azure_openai_endpoint: "",
      azure_openai_deployment: "",
      ollama_enabled: false,
      ollama_base_url: "http://localhost:11434",
      debug_log_enabled: false,
    },
    updates: { check_on_startup: true },
    connection_history: { enabled: true },
    ssh: {
      algorithm_mode: "default",
      default_private_key_path: "",
      algorithms: { kex: [], host_key: [], cipher: [], mac: [], compression: [] },
    },
    shortcuts: normalizeShortcutConfig(undefined),
    external_control: {
      enabled: false,
      connect_enabled: false,
      direct_connect_enabled: false,
      mcp_enabled: false,
      cli_enabled: false,
    },
    saved_connections: [],
  });
}
async function setup() {
  const configSave = deferred<AppConfig>();
  const keySave = deferred<undefined>();
  const clear = deferred<undefined>();
  const notify = vi.fn(async () => {});
  const invoke = vi.fn(
    async (command: string, _args?: Record<string, unknown>): Promise<unknown> => {
      if (command === "config_load") return fixture();
      if (command === "config_save") return configSave.promise;
      if (command === "ai_secret_set") return keySave.promise;
      if (command === "ai_secret_clear") return clear.promise;
      throw new Error("unexpected command");
    }
  );
  const controller = createSettingsSaveController(
    invoke as <T>(command: string, args?: Record<string, unknown>) => Promise<T>,
    notify
  );
  await controller.load();
  return { controller, invoke, configSave, keySave, clear, notify };
}
const font =
  (size: number) =>
  (config: AppConfig): AppConfig => ({
    ...config,
    terminal: { ...config.terminal, font_size: size },
  });

describe("settings saving through the production controller", () => {
  it("keeps shortcut bindings atomic like the backend instead of inventing a combined binding", async () => {
    const { controller: c, configSave } = await setup();
    c.updateConfig(font(16));
    const save = c.save();
    const binding = { key: "m", ctrl: true, alt: false, shift: false };
    c.updateConfig((cfg) => ({ ...cfg, shortcuts: { ...cfg.shortcuts, new_connection: binding } }));
    const merged = font(16)(fixture());
    merged.shortcuts.new_connection = { key: "n", ctrl: false, alt: true, shift: false };
    configSave.resolve(merged);
    await save;
    expect(c.getSnapshot().config?.shortcuts.new_connection).toEqual(binding);
    expect(c.getSnapshot().baseline?.shortcuts.new_connection).toEqual(
      merged.shortcuts.new_connection
    );
  });

  it("retains new key input during config save and config edits during key save", async () => {
    const { controller: c, configSave, keySave, invoke } = await setup();
    c.updateConfig(font(16));
    c.beginSecretEdit("openai");
    c.editSecret("openai", "fake-before");
    const save = c.save();
    await tick();
    c.editSecret("openai", "fake-after");
    configSave.resolve(font(16)(fixture()));
    await tick();
    c.updateConfig(font(20));
    keySave.resolve(undefined);
    await save;
    expect(c.getSnapshot()).toMatchObject({
      config: { terminal: { font_size: 20 } },
      baseline: { terminal: { font_size: 16 } },
      hasUnsavedChanges: true,
    });
    expect(c.getSecretInputs().openai).toBe("fake-after");
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ai_secret_set")[0][1]).toEqual({
      provider: "OpenAi",
      value: "fake-before",
    });
    c.revert();
    expect(c.getSnapshot().config?.terminal.font_size).toBe(16);
    expect(c.getSnapshot().hasUnsavedChanges).toBe(false);
  });

  it("accepts loaded credential status despite input edits, but rejects status older than a mutation", async () => {
    const { controller: c, invoke, keySave } = await setup();
    const status = deferred<unknown>();
    invoke.mockReturnValueOnce(status.promise);
    const loading = c.loadSecretStatus();
    c.beginSecretEdit("openai");
    c.editSecret("openai", "fake-key");
    c.cancelSecretEdit("openai");
    status.resolve({ openai: true });
    await loading;
    expect(c.getSnapshot().secretStatus.openai).toBe(true);
    const stale = deferred<unknown>();
    invoke.mockReturnValueOnce(stale.promise);
    const refresh = c.loadSecretStatus();
    c.editSecret("gemini", "fake-key");
    const save = c.save();
    keySave.resolve(undefined);
    await save;
    stale.resolve({ openai: true, gemini: false });
    await refresh;
    expect(c.getSnapshot().secretStatus.gemini).toBe(true);
  });

  it("supports delete retry and prevents save from overtaking a pending delete", async () => {
    const { controller: c, invoke, clear } = await setup();
    c.beginSecretEdit("openai");
    c.editSecret("openai", "fake-key");
    const deletion = c.clearSecret("openai");
    await c.save();
    clear.reject(new Error("fake-key echoed by store"));
    await deletion;
    expect(c.getSnapshot()).toMatchObject({
      error: "settings.secret_clear_failed",
      hasUnsavedChanges: true,
    });
    expect(c.getSecretInputs().openai).toBe("fake-key");
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ai_secret_set")).toHaveLength(0);
    invoke.mockResolvedValue(undefined);
    await c.clearSecret("openai");
    expect(c.getSnapshot()).toMatchObject({
      error: "",
      secretStatus: { openai: false },
      hasUnsavedChanges: false,
    });
  });

  it("retains same-field and sibling edits while accepting backend changes from another window", async () => {
    const { controller: c, configSave, invoke } = await setup();
    c.updateConfig(font(16));
    const save = c.save();
    c.updateConfig(font(18));
    c.updateConfig((cfg) => ({ ...cfg, terminal: { ...cfg.terminal, font_family: "new" } }));
    const merged = fixture();
    merged.terminal.font_size = 16;
    merged.language = "ja";
    merged.saved_connections = [{ id: "other-window" }] as AppConfig["saved_connections"];
    configSave.resolve(merged);
    await save;
    expect(c.getSnapshot().config).toMatchObject({
      language: "ja",
      terminal: { font_size: 18, font_family: "new" },
      saved_connections: merged.saved_connections,
    });
    expect(c.getSnapshot().baseline).toEqual(merged);
    expect(c.getSnapshot()).toMatchObject({ hasUnsavedChanges: true, saved: false });
    await c.save();
    const args = invoke.mock.calls.filter(([command]) => command === "config_save")[1][1];
    expect(args).toMatchObject({
      baseConfig: merged,
      editedConfig: { terminal: { font_size: 18 } },
    });
  });

  it("keeps edits made during key save and submits only the captured key", async () => {
    const { controller: c, keySave, invoke, notify } = await setup();
    c.beginSecretEdit("openai");
    c.editSecret("openai", "fake-first");
    const save = c.save();
    await tick();
    c.editSecret("openai", "fake-second");
    keySave.resolve(undefined);
    await save;
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ai_secret_set")).toEqual([
      ["ai_secret_set", { provider: "OpenAi", value: "fake-first" }],
    ]);
    expect(c.getSecretInputs().openai).toBe("fake-second");
    expect(c.getSnapshot()).toMatchObject({
      hasUnsavedChanges: true,
      secretStatus: { openai: true },
    });
    expect(notify).toHaveBeenCalledOnce();
    expect(JSON.stringify(c.getSnapshot())).not.toContain("fake-");
    await c.save();
    expect(c.getSnapshot().hasUnsavedChanges).toBe(false);
  });

  it("commits successful resources and retries only failed keys without exposing errors", async () => {
    const { controller: c, configSave, keySave, invoke } = await setup();
    c.updateConfig(font(16));
    c.beginSecretEdit("openai");
    c.editSecret("openai", "fake-good");
    c.beginSecretEdit("gemini");
    c.editSecret("gemini", "fake-bad");
    invoke.mockImplementation(async (command, args) => {
      if (command === "config_save") return configSave.promise;
      if (command === "ai_secret_set" && args?.provider === "Gemini")
        throw new Error("fake-bad secret echoed");
      return keySave.promise;
    });
    const save = c.save();
    configSave.resolve(font(16)(fixture()));
    keySave.resolve(undefined);
    await save;
    expect(c.getSnapshot()).toMatchObject({
      configResult: "success",
      results: { openai: "success", gemini: "failed" },
      hasUnsavedChanges: true,
      error: "settings.save_failed",
    });
    expect(c.getSecretInputs()).toMatchObject({ openai: "", gemini: "fake-bad" });
    expect(JSON.stringify(c.getSnapshot())).not.toContain("fake-");
    invoke.mockResolvedValue(undefined);
    await c.save();
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "config_save")).toHaveLength(1);
    expect(
      invoke.mock.calls.filter(
        ([cmd, args]) => cmd === "ai_secret_set" && args?.provider === "OpenAi"
      )
    ).toHaveLength(1);
    expect(c.getSnapshot()).toMatchObject({ hasUnsavedChanges: false, error: "", saved: true });
  });

  it("allows keys to succeed when config fails and retries with the unchanged config baseline", async () => {
    const { controller: c, configSave, keySave, invoke } = await setup();
    c.updateConfig(font(16));
    c.editSecret("openai", "fake-key");
    const save = c.save();
    configSave.reject(new Error("failure"));
    keySave.resolve(undefined);
    await save;
    expect(c.getSnapshot()).toMatchObject({
      configResult: "failed",
      baseline: { terminal: { font_size: 14 } },
      results: { openai: "success" },
    });
    invoke.mockResolvedValue(font(16)(fixture()));
    await c.save();
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ai_secret_set")).toHaveLength(1);
    expect(c.getSnapshot().hasUnsavedChanges).toBe(false);
  });

  it("serializes save/delete, rejects duplicate saves and preserves edits made after delete request", async () => {
    const { controller: c, keySave, clear, invoke } = await setup();
    c.beginSecretEdit("openai");
    c.editSecret("openai", "fake-first");
    const save = c.save();
    const duplicate = c.save();
    const deletion = c.clearSecret("openai");
    await tick();
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ai_secret_clear")).toHaveLength(0);
    c.editSecret("openai", "fake-new");
    keySave.resolve(undefined);
    await save;
    await duplicate;
    await tick();
    expect(c.getSnapshot().isSaving).toBe(true);
    clear.resolve(undefined);
    await deletion;
    expect(c.getSecretInputs().openai).toBe("fake-new");
    expect(c.getSnapshot()).toMatchObject({
      secretStatus: { openai: false },
      hasUnsavedChanges: true,
      isSaving: false,
    });
  });

  it("does not repeat credential mutations when only notification delivery needs a retry", async () => {
    const { controller: c, keySave, notify, invoke } = await setup();
    notify.mockRejectedValueOnce(new Error("notification failed"));
    c.editSecret("openai", "fake-key");
    const save = c.save();
    keySave.resolve(undefined);
    await save;
    expect(c.getSnapshot().hasUnsavedChanges).toBe(true);
    await c.save();
    expect(notify).toHaveBeenCalledTimes(2);
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ai_secret_set")).toHaveLength(1);
    expect(c.getSnapshot().hasUnsavedChanges).toBe(false);
  });

  it("ignores obsolete loads and load completion after disposal", async () => {
    const { controller: c, invoke } = await setup();
    const old = deferred<AppConfig>();
    const recent = deferred<AppConfig>();
    invoke.mockReturnValueOnce(old.promise).mockReturnValueOnce(recent.promise);
    const first = c.load();
    const second = c.load();
    recent.resolve(font(20)(fixture()));
    await second;
    old.resolve(fixture());
    await first;
    expect(c.getSnapshot().config?.terminal.font_size).toBe(20);
    const late = deferred<AppConfig>();
    invoke.mockReturnValueOnce(late.promise);
    const load = c.load();
    c.dispose();
    const before = c.getSnapshot();
    late.resolve(fixture());
    await load;
    expect(c.getSnapshot()).toEqual(before);
  });

  it("drops queued mutations after unmount and sends availability notification for an already committed key", async () => {
    const { controller: c, keySave, invoke, notify } = await setup();
    c.editSecret("openai", "fake-key");
    const save = c.save();
    await tick();
    const deletion = c.clearSecret("openai");
    const listener = vi.fn();
    c.subscribe(listener);
    c.dispose();
    keySave.resolve(undefined);
    await save;
    await deletion;
    expect(listener).not.toHaveBeenCalled();
    expect(c.getSecretInputs().openai).toBe("");
    expect(invoke.mock.calls.filter(([cmd]) => cmd === "ai_secret_clear")).toHaveLength(0);
    expect(notify).toHaveBeenCalledOnce();
  });
});
