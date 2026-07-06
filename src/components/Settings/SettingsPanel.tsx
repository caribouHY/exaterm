import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check } from "lucide-react";
import type { AppConfig, AiSecretStatus } from "../../types";
import { useTranslation } from "react-i18next";
import { resolveAppLanguage } from "../../i18n";
import "./SettingsPanel.css";

interface SettingsPanelProps {
  onSave?: () => void;
}

type SecretKey = "openai" | "azure_openai" | "anthropic" | "gemini" | "openrouter";
type SecretProvider = "OpenAi" | "AzureOpenAi" | "Anthropic" | "Gemini" | "OpenRouter";

type SecretEdits = Record<SecretKey, string>;
type LanguageOption = {
  value: AppConfig["language"];
  label?: string;
  labelKey?: "settings.language_system";
};

const MASKED_VALUE = "••••••••";
const FONT_SIZE_MIN = 8;
const FONT_SIZE_MAX = 32;
const SCROLLBACK_MIN = 100;
const SCROLLBACK_MAX = 100000;
const DEFAULT_EXTERNAL_CONTROL_CONFIG = {
  enabled: false,
  connect_enabled: false,
  mcp_enabled: false,
  cli_enabled: false,
};

const EMPTY_SECRET_STATUS: AiSecretStatus = {
  openai: false,
  azure_openai: false,
  anthropic: false,
  gemini: false,
  openrouter: false,
};

const EMPTY_SECRET_EDITS: SecretEdits = {
  openai: "",
  azure_openai: "",
  anthropic: "",
  gemini: "",
  openrouter: "",
};

const LANGUAGE_OPTIONS: LanguageOption[] = [
  { value: "system", labelKey: "settings.language_system" },
  { value: "en", label: "English" },
  { value: "ja", label: "日本語" },
];

function normalizeExternalControlConfig(config: AppConfig): AppConfig {
  return {
    ...config,
    external_control: {
      ...DEFAULT_EXTERNAL_CONTROL_CONFIG,
      ...(config.external_control ?? {}),
    },
  };
}

function parseBoundedNumber(value: string, currentValue: number, min: number, max: number): number {
  if (!value.trim()) {
    return currentValue;
  }

  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return currentValue;
  }

  return Math.min(Math.max(parsed, min), max);
}

export default function SettingsPanel({ onSave }: SettingsPanelProps) {
  const { t, i18n } = useTranslation();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  const [loadFailed, setLoadFailed] = useState(false);
  const [isLoadingConfig, setIsLoadingConfig] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [secretStatus, setSecretStatus] = useState<AiSecretStatus>(EMPTY_SECRET_STATUS);
  const [secretEdits, setSecretEdits] = useState<SecretEdits>(EMPTY_SECRET_EDITS);
  const savedTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [secretEditMode, setSecretEditMode] = useState({
    openai: false,
    azure_openai: false,
    anthropic: false,
    gemini: false,
    openrouter: false,
  });

  const refreshSecretStatus = async () => {
    try {
      const status = await invoke<AiSecretStatus>("ai_secret_status");
      setSecretStatus(status);
    } catch (e) {
      console.error("Failed to load AI secret status:", e);
      setSecretStatus(EMPTY_SECRET_STATUS);
    }
  };

  const clearSavedTimer = () => {
    if (savedTimeoutRef.current) {
      clearTimeout(savedTimeoutRef.current);
      savedTimeoutRef.current = null;
    }
  };

  const loadConfig = async () => {
    setIsLoadingConfig(true);
    setLoadFailed(false);
    setError("");
    try {
      const cfg = await invoke<AppConfig>("config_load");
      setConfig(normalizeExternalControlConfig(cfg));
    } catch (e) {
      console.error("Failed to load settings:", e);
      setConfig(null);
      setLoadFailed(true);
    } finally {
      setIsLoadingConfig(false);
    }
  };

  useEffect(() => {
    void loadConfig();
    void refreshSecretStatus();
    return () => {
      clearSavedTimer();
    };
  }, []);

  const handleSave = async () => {
    if (!config || isSaving) return;
    try {
      setIsSaving(true);
      setError("");
      clearSavedTimer();
      const normalizedConfig = normalizeExternalControlConfig(config);
      setConfig(normalizedConfig);
      await invoke("config_save", { config: normalizedConfig });

      if (secretEdits.openai.trim()) {
        await invoke("ai_secret_set", { provider: "OpenAi", value: secretEdits.openai.trim() });
      }
      if (secretEdits.azure_openai.trim()) {
        await invoke("ai_secret_set", {
          provider: "AzureOpenAi",
          value: secretEdits.azure_openai.trim(),
        });
      }
      if (secretEdits.anthropic.trim()) {
        await invoke("ai_secret_set", {
          provider: "Anthropic",
          value: secretEdits.anthropic.trim(),
        });
      }
      if (secretEdits.gemini.trim()) {
        await invoke("ai_secret_set", { provider: "Gemini", value: secretEdits.gemini.trim() });
      }
      if (secretEdits.openrouter.trim()) {
        await invoke("ai_secret_set", {
          provider: "OpenRouter",
          value: secretEdits.openrouter.trim(),
        });
      }

      setSecretEdits(EMPTY_SECRET_EDITS);
      setSecretEditMode({
        openai: false,
        azure_openai: false,
        anthropic: false,
        gemini: false,
        openrouter: false,
      });
      await refreshSecretStatus();

      const resolvedLanguage = resolveAppLanguage(normalizedConfig.language);
      await invoke("backend_language_set", { language: resolvedLanguage });
      if (resolvedLanguage !== i18n.language) {
        void i18n.changeLanguage(resolvedLanguage);
      }
      setSaved(true);
      if (onSave) onSave();
      savedTimeoutRef.current = setTimeout(() => {
        setSaved(false);
        savedTimeoutRef.current = null;
      }, 2000);
    } catch (e) {
      console.error(e);
      setError(typeof e === "string" ? e : "Failed to save settings.");
    } finally {
      setIsSaving(false);
    }
  };

  if (!config)
    return (
      <div className="settings-panel">
        {isLoadingConfig && <p>{t("settings.loading")}</p>}
        {loadFailed && (
          <>
            <p className="settings-error">{t("settings.load_failed")}</p>
            <button className="btn btn-primary" onClick={loadConfig}>
              {t("settings.reload")}
            </button>
          </>
        )}
      </div>
    );

  const update = (path: string, value: unknown) => {
    const newConfig = JSON.parse(JSON.stringify(config));
    const keys = path.split(".");
    let obj = newConfig;
    for (let i = 0; i < keys.length - 1; i++) obj = obj[keys[i]];
    obj[keys[keys.length - 1]] = value;
    setConfig(newConfig);
  };

  const clearSecret = async (provider: SecretProvider, key: SecretKey) => {
    try {
      await invoke("ai_secret_clear", { provider });
      setSecretEdits((prev) => ({ ...prev, [key]: "" }));
      setSecretEditMode((prev) => ({ ...prev, [key]: false }));
      setSecretStatus((prev) => ({ ...prev, [key]: false }));
    } catch (e) {
      console.error(e);
    }
  };

  const beginEditSecret = (key: SecretKey) => {
    setSecretEditMode((prev) => ({ ...prev, [key]: true }));
    setSecretEdits((prev) => ({ ...prev, [key]: "" }));
  };

  const cancelEditSecret = (key: SecretKey) => {
    setSecretEditMode((prev) => ({ ...prev, [key]: false }));
    setSecretEdits((prev) => ({ ...prev, [key]: "" }));
  };

  const renderSecretField = (
    key: SecretKey,
    provider: SecretProvider,
    label: string,
    placeholder: string
  ) => {
    const hasSecret = secretStatus[key];
    const isEditing = secretEditMode[key];
    const canType = !hasSecret || isEditing;
    const value = canType ? secretEdits[key] : MASKED_VALUE;

    return (
      <div style={{ marginBottom: 14 }}>
        <label className="label">{label}</label>
        <div className="settings-secret-row">
          <input
            className="input"
            type="password"
            value={value}
            readOnly={!canType}
            onChange={(e) => {
              setSecretEdits((prev) => ({ ...prev, [key]: e.target.value }));
            }}
            placeholder={hasSecret ? "" : placeholder}
          />
          {hasSecret && !isEditing && (
            <>
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                onClick={() => {
                  beginEditSecret(key);
                }}
              >
                {t("settings.change")}
              </button>
              <button
                type="button"
                className="btn btn-danger btn-sm"
                onClick={() => clearSecret(provider, key)}
              >
                {t("settings.clear")}
              </button>
            </>
          )}
          {isEditing && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => {
                cancelEditSecret(key);
              }}
            >
              {t("settings.cancel")}
            </button>
          )}
        </div>
      </div>
    );
  };

  return (
    <div className="settings-panel">
      <h2>{t("settings.title")}</h2>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.language")}</div>
        <div className="settings-row">
          <div>
            <select
              className="select"
              value={config.language}
              onChange={(e) => {
                update("language", e.target.value);
              }}
            >
              {LANGUAGE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.labelKey ? t(option.labelKey) : option.label}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.ai_provider")}</div>
        <div className="settings-row">
          <div>
            <label className="label">{t("settings.default_provider")}</label>
            <select
              className="select"
              value={config.ai.default_provider}
              onChange={(e) => {
                update("ai.default_provider", e.target.value);
              }}
            >
              <option value="OpenAi">OpenAI</option>
              <option value="AzureOpenAi">Azure OpenAI</option>
              <option value="Anthropic">Anthropic</option>
              <option value="Gemini">Google Gemini</option>
              <option value="OpenRouter">OpenRouter</option>
              <option value="Ollama">Ollama</option>
            </select>
          </div>
        </div>

        {renderSecretField("openai", "OpenAi", t("settings.openai_key"), "sk-...")}
        {renderSecretField("azure_openai", "AzureOpenAi", t("settings.azure_openai_key"), "...")}
        {renderSecretField("anthropic", "Anthropic", t("settings.anthropic_key"), "sk-ant-...")}
        {renderSecretField("gemini", "Gemini", t("settings.gemini_key"), "AIza...")}
        {renderSecretField("openrouter", "OpenRouter", t("settings.openrouter_key"), "sk-or-...")}

        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.azure_openai_enabled")}</span>
            <small>{t("settings.azure_openai_enabled_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={Boolean(config.ai.azure_openai_enabled)}
              onChange={(e) => update("ai.azure_openai_enabled", e.target.checked)}
            />
            <span className="toggle-track" />
          </label>
        </div>

        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.azure_openai_endpoint")}</label>
          <input
            className="input"
            type="text"
            value={config.ai.azure_openai_endpoint}
            onChange={(e) => update("ai.azure_openai_endpoint", e.target.value)}
            placeholder="https://your-resource.openai.azure.com/openai/v1/chat/completions"
          />
        </div>

        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.azure_openai_deployment")}</label>
          <input
            className="input"
            type="text"
            value={config.ai.azure_openai_deployment}
            onChange={(e) => update("ai.azure_openai_deployment", e.target.value)}
            placeholder="my-gpt4o-deployment"
          />
        </div>

        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.ollama_enabled")}</span>
            <small>{t("settings.ollama_enabled_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={Boolean(config.ai.ollama_enabled)}
              onChange={(e) => update("ai.ollama_enabled", e.target.checked)}
            />
            <span className="toggle-track" />
          </label>
        </div>

        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.ollama_url")}</label>
          <input
            className="input"
            type="text"
            value={config.ai.ollama_base_url}
            onChange={(e) => update("ai.ollama_base_url", e.target.value)}
            placeholder="http://localhost:11434"
          />
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.mcp_settings")}</div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.mcp_enabled")}</span>
            <small>{t("settings.mcp_enabled_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={Boolean(config.external_control.enabled)}
              onChange={(e) => {
                update("external_control.enabled", e.target.checked);
              }}
            />
            <span className="toggle-track" />
          </label>
        </div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.mcp_cli_enabled")}</span>
            <small>{t("settings.mcp_cli_enabled_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={Boolean(config.external_control.cli_enabled)}
              onChange={(e) => {
                update("external_control.cli_enabled", e.target.checked);
              }}
            />
            <span className="toggle-track" />
          </label>
        </div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.mcp_connect_enabled")}</span>
            <small>{t("settings.mcp_connect_enabled_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={Boolean(config.external_control.connect_enabled)}
              onChange={(e) => {
                update("external_control.connect_enabled", e.target.checked);
              }}
            />
            <span className="toggle-track" />
          </label>
        </div>
        <div className="settings-section__title" style={{ marginTop: 20 }}>
          {t("settings.mcp_adapter_title")}
        </div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.mcp_stdio_enabled")}</span>
            <small>{t("settings.mcp_stdio_enabled_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={Boolean(config.external_control.mcp_enabled)}
              onChange={(e) => {
                update("external_control.mcp_enabled", e.target.checked);
              }}
            />
            <span className="toggle-track" />
          </label>
        </div>
        <p className="settings-help">{t("settings.mcp_restart_notice")}</p>
        {t("settings.mcp_loopback_warning") && (
          <p className="settings-help">{t("settings.mcp_loopback_warning")}</p>
        )}
      </div>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.ssh_settings")}</div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.allow_legacy_ssh_algorithms")}</span>
            <small>{t("settings.allow_legacy_ssh_algorithms_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={Boolean(config.ssh.allow_legacy_algorithms)}
              onChange={(e) => {
                update("ssh.allow_legacy_algorithms", e.target.checked);
              }}
            />
            <span className="toggle-track" />
          </label>
        </div>
      </div>

      <div className="settings-section">
        <div className="settings-section__title">{t("settings.terminal_settings")}</div>
        <div className="settings-row">
          <div>
            <label className="label">{t("settings.font_size")}</label>
            <input
              className="input"
              type="number"
              value={config.terminal.font_size}
              onChange={(e) => {
                update(
                  "terminal.font_size",
                  parseBoundedNumber(
                    e.target.value,
                    config.terminal.font_size,
                    FONT_SIZE_MIN,
                    FONT_SIZE_MAX
                  )
                );
              }}
              min={FONT_SIZE_MIN}
              max={FONT_SIZE_MAX}
            />
          </div>
          <div>
            <label className="label">{t("settings.scrollback")}</label>
            <input
              className="input"
              type="number"
              value={config.terminal.scrollback}
              onChange={(e) => {
                update(
                  "terminal.scrollback",
                  parseBoundedNumber(
                    e.target.value,
                    config.terminal.scrollback,
                    SCROLLBACK_MIN,
                    SCROLLBACK_MAX
                  )
                );
              }}
              min={SCROLLBACK_MIN}
              max={SCROLLBACK_MAX}
            />
          </div>
        </div>
        <div>
          <label className="label">{t("settings.font_family")}</label>
          <input
            className="input"
            value={config.terminal.font_family}
            onChange={(e) => {
              update("terminal.font_family", e.target.value);
            }}
          />
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
              onChange={(e) => {
                update("terminal.auto_session_log", e.target.checked);
              }}
            />
            <span className="toggle-track" />
          </label>
        </div>
        <div style={{ marginBottom: 14 }}>
          <label className="label">{t("settings.log_format")}</label>
          <select
            className="select"
            value={config.terminal.log_format || "display"}
            onChange={(e) => {
              update("terminal.log_format", e.target.value);
            }}
          >
            <option value="display">{t("settings.log_format_display")}</option>
            <option value="strip_controls">{t("settings.log_format_strip_controls")}</option>
          </select>
        </div>
        <div className="settings-toggle-row">
          <div className="settings-toggle-label">
            <span>{t("settings.include_log_header")}</span>
            <small>{t("settings.include_log_header_desc")}</small>
          </div>
          <label className="toggle">
            <input
              type="checkbox"
              checked={config.terminal.include_log_header ?? false}
              onChange={(e) => {
                update("terminal.include_log_header", e.target.checked);
              }}
            />
            <span className="toggle-track" />
          </label>
        </div>
      </div>

      <div className="settings-actions">
        <button className="btn btn-primary" onClick={handleSave} disabled={isSaving}>
          {isSaving ? t("settings.saving") : t("settings.save")}
        </button>
        {saved && (
          <span className="settings-saved">
            <Check size={14} /> {t("settings.saved")}
          </span>
        )}
        {error && <span className="settings-error">{error}</span>}
      </div>
    </div>
  );
}
