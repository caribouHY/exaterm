import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en.json";
import ja from "./locales/ja.json";
import { resolveAppLanguage } from "./features/language/languageModel";

const resources = {
  en: { translation: en },
  ja: { translation: ja },
};

i18n.use(initReactI18next).init({
  resources,
  lng: resolveAppLanguage("system", globalThis.navigator.language),
  fallbackLng: "en",
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
