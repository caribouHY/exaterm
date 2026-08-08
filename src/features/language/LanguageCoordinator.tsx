import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import i18n from "../../i18n";
import { createLanguageSyncController, type LanguageSyncStage } from "./languageSync";

interface LanguageCoordinatorProps {
  children: ReactNode;
}

interface LanguageConfig {
  language?: string;
}

function reportLanguageSyncError(stage: LanguageSyncStage, error: unknown) {
  console.error(`Language synchronization failed during ${stage}:`, error);
}

export function LanguageCoordinator({ children }: LanguageCoordinatorProps) {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    const controller = createLanguageSyncController({
      loadConfiguredLanguage: async () =>
        (await invoke<LanguageConfig | null>("config_load"))?.language,
      getFrontendLanguage: () => i18n.resolvedLanguage ?? i18n.language,
      changeFrontendLanguage: async (language) => {
        await i18n.changeLanguage(language);
      },
      systemLanguage: globalThis.navigator.language,
      reportError: reportLanguageSyncError,
    });

    const start = async () => {
      try {
        const stopListening = await listen("config://updated", () => {
          void controller.requestSync();
        });
        if (!active) {
          stopListening();
          return;
        }
        unlisten = stopListening;
      } catch (error) {
        reportLanguageSyncError("event_listener", error);
      }

      await controller.requestSync();
      if (active) setReady(true);
    };

    void start();
    return () => {
      active = false;
      controller.dispose();
      unlisten?.();
    };
  }, []);

  return ready ? children : null;
}
