import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';

import en from './locales/en.json';
import ja from './locales/ja.json';

const resources = {
  en: { translation: en },
  ja: { translation: ja }
};

i18n
  .use(initReactI18next)
  .init({
    resources,
    lng: 'en', // default language
    fallbackLng: 'en',
    interpolation: {
      escapeValue: false
    }
  });

// Load the language from config on startup
invoke<any>("config_load").then((config) => {
  if (config && config.language) {
    i18n.changeLanguage(config.language);
  }
}).catch(console.error);

export default i18n;
