import type { AiSecretStatus, AppConfig } from "../../types";
import {
  SECRET_FIELDS,
  areConfigsEqual,
  createSecretEdits,
  createSecretEditMode,
  createSecretStatus,
  normalizeExternalControlConfig,
  type SecretKey,
  type SecretEdits,
} from "./settingsModel";

type Result = "idle" | "saving" | "success" | "failed";
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
    const before = sent as Record<string, unknown>;
    const draft = current as Record<string, unknown>;
    const result = { ...(saved as Record<string, unknown>) };
    for (const key of Object.keys(draft))
      result[key] = rebase(before[key], draft[key], result[key], [...path, key]);
    return result;
  }
  return current;
}

export function createSettingsSaveController(invoke: Invoke, notify: () => Promise<void>) {
  let active = true;
  let loadId = 0;
  let statusLoadId = 0;
  let config: AppConfig | null = null;
  let baseline: AppConfig | null = null;
  let values = createSecretEdits();
  let modes = createSecretEditMode();
  let status = createSecretStatus();
  const generations = Object.fromEntries(SECRET_FIELDS.map(({ key }) => [key, 0])) as Record<
    SecretKey,
    number
  >;
  const statusVersions = { ...generations };
  let results = Object.fromEntries(SECRET_FIELDS.map(({ key }) => [key, "idle"])) as Record<
    SecretKey,
    Result
  >;
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
    SECRET_FIELDS.some(({ key }) => Boolean(values[key].trim()) || modes[key]) ||
    notificationPending;
  const snapshot = () => ({
    config: structuredClone(config),
    baseline: structuredClone(baseline),
    secretStatus: { ...status },
    secretEditMode: { ...modes },
    results: { ...results },
    configResult,
    isLoadingConfig: loading,
    loadFailed,
    isSaving: pending > 0,
    saved: saved && !dirty(),
    error,
    hasUnsavedChanges: dirty(),
  });
  const publish = () => {
    if (active) for (const listener of listeners) listener();
  };
  const resetInput = (key: SecretKey) => {
    values[key] = "";
    modes[key] = false;
    generations[key]++;
  };
  const enqueue = (operation: () => Promise<void>) => {
    pending++;
    saved = false;
    error = "";
    publish();
    const task = tail.then(async () => {
      if (active) await operation();
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
      if (active) error = "settings.save_failed";
    }
  };
  return {
    getSnapshot: snapshot,
    // Only the password inputs consume these values; snapshots and diagnostics never do.
    getSecretInputs: (): SecretEdits => ({ ...values }),
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    async load() {
      if (!active || pending) return;
      const id = ++loadId;
      loading = true;
      loadFailed = false;
      publish();
      try {
        const loaded = normalizeExternalControlConfig(await invoke<AppConfig>("config_load"));
        if (!active || id !== loadId) return;
        config = loaded;
        baseline = loaded;
      } catch {
        if (active && id === loadId) loadFailed = true;
      } finally {
        if (active && id === loadId) {
          loading = false;
          publish();
        }
      }
    },
    async loadSecretStatus() {
      if (!active) return;
      const id = ++statusLoadId;
      const versions = { ...statusVersions };
      try {
        const loaded = await invoke<AiSecretStatus>("ai_secret_status");
        if (!active || id !== statusLoadId) return;
        for (const { key } of SECRET_FIELDS)
          if (versions[key] === statusVersions[key]) status[key] = loaded[key];
        publish();
      } catch {
        /* Availability stays unknown until a successful operation or refresh. */
      }
    },
    updateConfig(update: (previous: AppConfig) => AppConfig) {
      if (!active || !config) return;
      config = update(config);
      saved = false;
      publish();
    },
    editSecret(key: SecretKey, value: string) {
      if (!active) return;
      values[key] = value;
      generations[key]++;
      saved = false;
      publish();
    },
    beginSecretEdit(key: SecretKey) {
      if (active) {
        resetInput(key);
        modes[key] = true;
        saved = false;
        publish();
      }
    },
    cancelSecretEdit(key: SecretKey) {
      if (active) {
        resetInput(key);
        publish();
      }
    },
    revert() {
      if (!active || pending) return;
      config = baseline;
      for (const { key } of SECRET_FIELDS) resetInput(key);
      error = "";
      saved = false;
      publish();
    },
    save() {
      if (!active || !config || !baseline || pending || !dirty()) return Promise.resolve();
      const sent = normalizeExternalControlConfig(structuredClone(config));
      const base = structuredClone(baseline);
      let submitted = { ...values };
      const versions = { ...generations };
      return enqueue(async () => {
        if (!areConfigsEqual(sent, base)) {
          configResult = "saving";
          publish();
          try {
            const merged = normalizeExternalControlConfig(
              await invoke<AppConfig>("config_save", { baseConfig: base, editedConfig: sent })
            );
            if (!active) return;
            config = rebase(sent, config, merged) as AppConfig;
            baseline = merged;
            configResult = "success";
          } catch {
            if (active) {
              configResult = "failed";
              error = "settings.save_failed";
            }
          }
          publish();
        }
        for (const { key, provider } of SECRET_FIELDS) {
          if (!active) break;
          const value = submitted[key].trim();
          if (!value) {
            if (versions[key] === generations[key]) resetInput(key);
            continue;
          }
          results[key] = "saving";
          publish();
          try {
            await invoke("ai_secret_set", { provider, value });
            notificationPending = true;
            if (!active) break;
            status[key] = true;
            statusVersions[key]++;
            results[key] = "success";
            if (versions[key] === generations[key]) resetInput(key);
          } catch {
            if (active) {
              results[key] = "failed";
              error = "settings.save_failed";
            }
          }
          submitted[key] = "";
          publish();
        }
        submitted = createSecretEdits();
        await flushNotification();
        if (active) saved = !error;
      }).finally(() => {
        submitted = createSecretEdits();
      });
    },
    clearSecret(key: SecretKey) {
      if (!active) return Promise.resolve();
      const field = SECRET_FIELDS.find((candidate) => candidate.key === key)!;
      const version = generations[key];
      return enqueue(async () => {
        results[key] = "saving";
        publish();
        try {
          await invoke("ai_secret_clear", { provider: field.provider });
          notificationPending = true;
          if (active) {
            status[key] = false;
            statusVersions[key]++;
            results[key] = "success";
            if (version === generations[key]) resetInput(key);
          }
        } catch {
          if (active) {
            results[key] = "failed";
            error = "settings.secret_clear_failed";
          }
        }
        await flushNotification();
      });
    },
    dispose() {
      active = false;
      loadId++;
      values = createSecretEdits();
      listeners.clear();
    },
  };
}
