import { resolveAppLanguage, type EffectiveLanguage } from "./languageModel";

export type LanguageSyncStage = "event_listener" | "config_load" | "frontend";

interface LanguageSyncDependencies {
  loadConfiguredLanguage: () => Promise<string | undefined>;
  getFrontendLanguage: () => string | undefined;
  changeFrontendLanguage: (language: EffectiveLanguage) => Promise<void>;
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
  systemLanguage,
  reportError,
}: LanguageSyncDependencies): LanguageSyncController {
  const abortController = new AbortController();
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

    if (abortController.signal.aborted) return;
    const effectiveLanguage = resolveAppLanguage(configuredLanguage, systemLanguage);

    if (getFrontendLanguage() !== effectiveLanguage) {
      try {
        await changeFrontendLanguage(effectiveLanguage);
      } catch (error) {
        reportError("frontend", error);
      }
    }

    hasAppliedLanguage = true;
  };

  const requestSync = () => {
    if (abortController.signal.aborted) return Promise.resolve();
    pending = true;
    if (running) return running;

    running = (async () => {
      while (pending) {
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
      abortController.abort();
      pending = false;
    },
  };
}
