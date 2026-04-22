import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check } from "lucide-react";
import type { AppConfig } from "../../types";
import { useTranslation } from "react-i18next";
import "./SettingsPanel.css";

interface SettingsPanelProps {
  onSave?: () => void;
}

export default function SettingsPanel({ onSave }: SettingsPanelProps) {
  const { t, i18n } = useTranslation();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<AppConfig>("config_load").then(setConfig).catch(() => {});
  }, []);

  const handleSave = async () => {
    if (!config) return;
    try {
      await invoke("config_save", { config });
      if (config.language !== i18n.language) {
        i18n.changeLanguage(config.language);
      }
      setSaved(true);
      if (onSave) onSave();
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      console.error(e);
    }
  };

  if (!config) return <div className="settings-panel"><p>{t("settings.loading")}</p></div>;

  const update = (path: string, value: any) => {
    const newConfig = JSON.parse(JSON.stringify(config));
    const keys = path.split(".");
    let obj = newConfig;
    for (let i = 0; i < keys.length - 1; i++) obj = obj[keys[i]];
    obj[keys[keys.length - 1]] = value;
    setConfig(newConfig);
  };

  return (
    <div className="settings-panel">
      <h2>{t("settings.title")}</h2>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.language")}</div>
        <div className="settings-row">
          <div>
            <select className="select" value={config.language} onChange={(e) => update("language", e.target.value)}>
              <option value="en">English</option>
              <option value="ja">日本語</option>
            </select>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.ai_provider")}</div>
        <div className="settings-row">
          <div>
            <label className="label">{t("settings.default_provider")}</label>
            <select className="select" value={config.ai.default_provider} onChange={(e) => update("ai.default_provider", e.target.value)}>
              <option value="OpenAi">OpenAI</option>
              <option value="Anthropic">Anthropic</option>
              <option value="Gemini">Google Gemini</option>
              <option value="Ollama">Ollama</option>
            </select>
          </div>
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.openai_key")}</label>
          <input className="input" type="password" value={config.ai.openai_api_key} onChange={(e) => update("ai.openai_api_key", e.target.value)} placeholder="sk-..." />
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.anthropic_key")}</label>
          <input className="input" type="password" value={config.ai.anthropic_api_key} onChange={(e) => update("ai.anthropic_api_key", e.target.value)} placeholder="sk-ant-..." />
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.gemini_key")}</label>
          <input className="input" type="password" value={config.ai.gemini_api_key} onChange={(e) => update("ai.gemini_api_key", e.target.value)} placeholder="AIza..." />
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.ollama_url")}</label>
          <input className="input" type="text" value={config.ai.ollama_base_url || "http://localhost:11434"} onChange={(e) => update("ai.ollama_base_url", e.target.value)} placeholder="http://localhost:11434" />
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.terminal_settings")}</div>
        <div className="settings-row">
          <div>
            <label className="label">{t("settings.font_size")}</label>
            <input className="input" type="number" value={config.terminal.font_size} onChange={(e) => update("terminal.font_size", parseInt(e.target.value))} min={8} max={32} />
          </div>
          <div>
            <label className="label">{t("settings.scrollback")}</label>
            <input className="input" type="number" value={config.terminal.scrollback} onChange={(e) => update("terminal.scrollback", parseInt(e.target.value))} />
          </div>
        </div>
        <div>
          <label className="label">{t("settings.font_family")}</label>
          <input className="input" value={config.terminal.font_family} onChange={(e) => update("terminal.font_family", e.target.value)} />
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.log_settings")}</div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.auto_session_log")}</span>
            <small>{t("settings.auto_session_log_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={config.terminal.auto_session_log}
              onChange={(e) => update("terminal.auto_session_log", e.target.checked)}
            />
            <span className="toggle-track" />
          </label>
        </div>
      </div>

      <div className="settings-actions">
        <button className="btn btn-primary" onClick={handleSave}>{t("settings.save")}</button>
        {saved && <span className="settings-saved"><Check size={14} /> {t("settings.saved")}</span>}
      </div>
    </div>
  );
}
