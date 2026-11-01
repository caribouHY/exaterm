import type { AiSecretStatus, AppConfig } from "../../types";
import {
  SECRET_FIELDS,
  areConfigsEqual,
  normalizeExternalControlConfig,
  type SecretKey,
  type SecretEdits,
  type SecretEditMode,
  type SecretProvider,
} from "./settingsModel";

type Result = "idle" | "saving" | "success" | "failed";
interface SecretState {
  key: SecretKey;
  provider: SecretProvider;
  value: string;
  editing: boolean;
  configured: boolean;
  generation: number;
  statusVersion: number;
  result: Result;
}

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

// Rebase only edits made after submission onto the backend's merged result.
function rebase(sent: unknown, current: unknown, saved: unknown, path: string[] = []): unknown {
  if (JSON.stringify(sent) === JSON.stringify(current)) return saved;
  // Match config.rs: shortcut bindings are indivisible and these root fields are backend-owned.
  if (path.length === 1 && ["config_version", "saved_connections"].includes(path[0])) return saved;
  if (path.length === 2 && path[0] === "shortcuts") return current;
  if (
    sent &&
    current &&
    saved &&
    typeof sent === "object" &&
    typeof current === "object" &&
    typeof saved === "object" &&
    !Array.isArray(sent) &&
    !Array.isArray(current)
  ) {
    const before = new Map(Object.entries(sent));
    const result = new Map(Object.entries(saved));
    for (const [key, value] of Object.entries(current))
      result.set(key, rebase(before.get(key), value, result.get(key), [...path, key]));
    return Object.fromEntries(result);
  }
  return current;
}

export function createSettingsSaveController(invoke: Invoke, notify: () => Promise<void>) {
  const lifetime = new AbortController();
  const isActive = () => !lifetime.signal.aborted;
  let loadId = 0;
  let statusLoadId = 0;
  let config: AppConfig | null = null;
  let baseline: AppConfig | null = null;
  const secrets = new Map<SecretKey, SecretState>(
    SECRET_FIELDS.map(({ key, provider }) => [
      key,
      {
        key,
        provider,
        value: "",
        editing: false,
        configured: false,
        generation: 0,
        statusVersion: 0,
        result: "idle",
      },
    ])
  );
  const getSecret = (key: SecretKey) => {
    const secret = secrets.get(key);
    if (!secret) throw new Error("Unknown secret provider.");
    return secret;
  };
  let configResult: Result = "idle";
  let loading = true;
  let loadFailed = false;
  let pending = 0;
  let saved = false;
  let error: "" | "settings.save_failed" | "settings.secret_clear_failed" = "";
  let notificationPending = false;
  let tail = Promise.resolve();
  const listeners = new Set<() => void>();
  const dirty = () =>
    !areConfigsEqual(config, baseline) ||
    Array.from(secrets.values()).some((secret) => Boolean(secret.value.trim()) || secret.editing) ||
    notificationPending;
  const snapshot = () => ({
    config: structuredClone(config),
    baseline: structuredClone(baseline),
    secretStatus: Object.fromEntries(
      Array.from(secrets.values(), (secret) => [secret.key, secret.configured])
    ) as Record<SecretKey, boolean>,
    secretEditMode: Object.fromEntries(
      Array.from(secrets.values(), (secret) => [secret.key, secret.editing])
    ) as SecretEditMode,
    results: Object.fromEntries(
      Array.from(secrets.values(), (secret) => [secret.key, secret.result])
    ) as Record<SecretKey, Result>,
    configResult,
    isLoadingConfig: loading,
    loadFailed,
    isSaving: pending > 0,
    saved: saved && !dirty(),
    error,
    hasUnsavedChanges: dirty(),
  });
  const publish = () => {
    if (isActive()) for (const listener of listeners) listener();
  };
  const resetInput = (key: SecretKey) => {
    const secret = getSecret(key);
    secret.value = "";
    secret.editing = false;
    secret.generation++;
  };
  const enqueue = (operation: () => Promise<void>) => {
    pending++;
    saved = false;
    error = "";
    publish();
    const task = tail.then(async () => {
      if (isActive()) await operation();
    });
    tail = task.catch(() => {});
    return task.finally(() => {
      pending--;
      publish();
    });
  };
  const flushNotification = async () => {
    if (!notificationPending) return;
    try {
      await notify();
      notificationPending = false;
    } catch {
      if (isActive()) error = "settings.save_failed";
    }
  };
  return {
    getSnapshot: snapshot,
    // Only the password inputs consume these values; snapshots and diagnostics never do.
    getSecretInputs: (): SecretEdits =>
      Object.fromEntries(
        Array.from(secrets.values(), (secret) => [secret.key, secret.value])
      ) as SecretEdits,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    async load() {
      if (!isActive() || pending) return;
      const id = ++loadId;
      loading = true;
      loadFailed = false;
      publish();
      try {
        const loaded = normalizeExternalControlConfig(await invoke<AppConfig>("config_load"));
        if (!isActive() || id !== loadId) return;
        config = loaded;
        baseline = loaded;
      } catch {
        if (isActive() && id === loadId) loadFailed = true;
      } finally {
        if (isActive() && id === loadId) {
          loading = false;
          publish();
        }
      }
    },
    async loadSecretStatus() {
      if (!isActive()) return;
      const id = ++statusLoadId;
      const versions = new Map(
        Array.from(secrets.values(), (secret) => [secret.key, secret.statusVersion])
      );
      try {
        const loaded = await invoke<AiSecretStatus>("ai_secret_status");
        if (!isActive() || id !== statusLoadId) return;
        const statuses = new Map(Object.entries(loaded));
        for (const secret of secrets.values())
          if (versions.get(secret.key) === secret.statusVersion)
            secret.configured = statuses.get(secret.key) === true;
        publish();
      } catch {
        /* Availability stays unknown until a successful operation or refresh. */
      }
    },
    updateConfig(update: (previous: AppConfig) => AppConfig) {
      if (!isActive() || !config) return;
      config = update(config);
      saved = false;
      publish();
    },
    editSecret(key: SecretKey, value: string) {
      if (!isActive()) return;
      const secret = getSecret(key);
      secret.value = value;
      secret.generation++;
      saved = false;
      publish();
    },
    beginSecretEdit(key: SecretKey) {
      if (isActive()) {
        resetInput(key);
        getSecret(key).editing = true;
        saved = false;
        publish();
      }
    },
    cancelSecretEdit(key: SecretKey) {
      if (isActive()) {
        resetInput(key);
        publish();
      }
    },
    revert() {
      if (!isActive() || pending) return;
      config = baseline;
      for (const { key } of SECRET_FIELDS) resetInput(key);
      error = "";
      saved = false;
      publish();
    },
    save() {
      if (!isActive() || !config || !baseline || pending || !dirty()) return Promise.resolve();
      const sent = normalizeExternalControlConfig(structuredClone(config));
      const base = structuredClone(baseline);
      const submitted = new Map(
        Array.from(secrets.values(), (secret) => [
          secret.key,
          {
            value: secret.value,
            generation: secret.generation,
          },
        ])
      );
      return enqueue(async () => {
        if (!areConfigsEqual(sent, base)) {
          configResult = "saving";
          publish();
          try {
            const merged = normalizeExternalControlConfig(
              await invoke<AppConfig>("config_save", { baseConfig: base, editedConfig: sent })
            );
            if (!isActive()) return;
            config = rebase(sent, config, merged) as AppConfig;
            baseline = merged;
            configResult = "success";
          } catch {
            if (isActive()) {
              configResult = "failed";
              error = "settings.save_failed";
            }
          }
          publish();
        }
        for (const [key, input] of submitted) {
          const secret = getSecret(key);
          if (!isActive()) break;
          const value = input.value.trim();
          if (!value) {
            if (input.generation === secret.generation) resetInput(key);
            continue;
          }
          secret.result = "saving";
          publish();
          try {
            await invoke("ai_secret_set", { provider: secret.provider, value });
            notificationPending = true;
            if (!isActive()) break;
            secret.configured = true;
            secret.statusVersion++;
            secret.result = "success";
            if (input.generation === secret.generation) resetInput(key);
          } catch {
            if (isActive()) {
              secret.result = "failed";
              error = "settings.save_failed";
            }
          }
          input.value = "";
          publish();
        }
        submitted.clear();
        await flushNotification();
        if (isActive()) saved = !error;
      }).finally(() => {
        submitted.clear();
      });
    },
    clearSecret(key: SecretKey) {
      if (!isActive()) return Promise.resolve();
      const secret = getSecret(key);
      const version = secret.generation;
      return enqueue(async () => {
        secret.result = "saving";
        publish();
        try {
          await invoke("ai_secret_clear", { provider: secret.provider });
          notificationPending = true;
          if (isActive()) {
            secret.configured = false;
            secret.statusVersion++;
            secret.result = "success";
            if (version === secret.generation) resetInput(key);
          }
        } catch {
          if (isActive()) {
            secret.result = "failed";
            error = "settings.secret_clear_failed";
          }
        }
        await flushNotification();
      });
    },
    dispose() {
      lifetime.abort();
      loadId++;
      for (const secret of secrets.values()) secret.value = "";
      listeners.clear();
    },
  };
}
