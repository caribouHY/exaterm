import { resolveAppLanguage, type EffectiveLanguage } from "./languageModel";

export type LanguageSyncStage = "event_listener" | "config_load" | "frontend" | "backend";

interface LanguageSyncDependencies {
  loadConfiguredLanguage: () => Promise<string | undefined>;
  getFrontendLanguage: () => string | undefined;
  changeFrontendLanguage: (language: EffectiveLanguage) => Promise<void>;
  setBackendLanguage: (language: EffectiveLanguage) => Promise<void>;
  systemLanguage: string | undefined;
  reportError: (stage: LanguageSyncStage, error: unknown) => void;
}

export interface LanguageSyncController {
  requestSync: () => Promise<void>;
  dispose: () => void;
}

export function createLanguageSyncController({
  loadConfiguredLanguage,
  getFrontendLanguage,
  changeFrontendLanguage,
  setBackendLanguage,
  systemLanguage,
  reportError,
}: LanguageSyncDependencies): LanguageSyncController {
  let disposed = false;
  let hasAppliedLanguage = false;
  let pending = false;
  let running: Promise<void> | null = null;

  const syncOnce = async () => {
    let configuredLanguage: string | undefined;
    try {
      configuredLanguage = await loadConfiguredLanguage();
    } catch (error) {
      reportError("config_load", error);
      if (hasAppliedLanguage) return;
      configuredLanguage = "system";
    }

    if (disposed) return;
    const effectiveLanguage = resolveAppLanguage(configuredLanguage, systemLanguage);

    if (getFrontendLanguage() !== effectiveLanguage) {
      try {
        await changeFrontendLanguage(effectiveLanguage);
      } catch (error) {
        reportError("frontend", error);
      }
    }

    if (disposed) return;
    try {
      await setBackendLanguage(effectiveLanguage);
    } catch (error) {
      reportError("backend", error);
    }
    hasAppliedLanguage = true;
  };

  const requestSync = () => {
    if (disposed) return Promise.resolve();
    pending = true;
    if (running) return running;

    running = (async () => {
      while (pending && !disposed) {
        pending = false;
        await syncOnce();
      }
    })().finally(() => {
      running = null;
    });
    return running;
  };

  return {
    requestSync,
    dispose() {
      disposed = true;
      pending = false;
    },
  };
}
