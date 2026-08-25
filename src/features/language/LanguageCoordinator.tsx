import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import i18n from "../../i18n";
import type { AppConfig } from "../../types";
import {
  createLanguageSyncController,
  loadAndApplyConfig,
  type LanguageSyncStage,
} from "./languageSync";

interface LanguageCoordinatorProps {
  children: ReactNode;
}

const AppConfigContext = createContext<AppConfig | null | undefined>(undefined);

export function useAppConfig() {
  const config = useContext(AppConfigContext);
  if (config === undefined) {
    throw new Error("useAppConfig must be used within LanguageCoordinator.");
  }
  return config;
}

function reportLanguageSyncError(stage: LanguageSyncStage, error: unknown) {
  console.error(`Language synchronization failed during ${stage}:`, error);
}

export function LanguageCoordinator({ children }: LanguageCoordinatorProps) {
  const [ready, setReady] = useState(false);
  const [config, setConfig] = useState<AppConfig | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    const controller = createLanguageSyncController({
      loadConfiguredLanguage: () =>
        loadAndApplyConfig(
          () => invoke<AppConfig>("config_load"),
          (nextConfig) => {
            if (active) setConfig(nextConfig);
          }
        ),
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

  return ready ? (
    <AppConfigContext.Provider value={config}>{children}</AppConfigContext.Provider>
  ) : null;
}
