import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

import en from "./locales/en.json";
import ja from "./locales/ja.json";

const resources = {
  en: { translation: en },
  ja: { translation: ja },
};

export type AppLanguage = "system" | "en" | "ja";

function resolveSystemLanguage(systemLanguage: string | undefined): "en" | "ja" {
  return systemLanguage?.toLowerCase().startsWith("ja") ? "ja" : "en";
}

export function resolveAppLanguage(language: string | undefined): "en" | "ja" {
  if (language === "ja" || language === "en") {
    return language;
  }

  return resolveSystemLanguage(globalThis.navigator.language);
}

i18n.use(initReactI18next).init({
  resources,
  lng: resolveAppLanguage("system"),
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
});

function syncBackendLanguage(language: "en" | "ja") {
  void invoke("backend_language_set", { language }).catch(console.error);
}

// Load the language from config on startup
invoke<{ language?: string } | null>("config_load")
  .then((config) => {
    const language = resolveAppLanguage(config?.language);
    syncBackendLanguage(language);
    if (language !== i18n.language) {
      void i18n.changeLanguage(language);
    }
  })
  .catch((error) => {
    console.error(error);
    syncBackendLanguage(resolveAppLanguage("system"));
  });

export default i18n;
