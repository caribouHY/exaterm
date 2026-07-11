import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Check } from "lucide-react";
import type { AppConfig, AiSecretStatus, LogFormat } from "../../types";
import { useTranslation } from "react-i18next";
import { resolveAppLanguage } from "../../i18n";
import "./SettingsPanel.css";

interface SettingsPanelProps {
  onSave?: () => void;
}

type SecretKey = "openai" | "azure_openai" | "anthropic" | "gemini" | "openrouter";
type SecretProvider = "OpenAi" | "AzureOpenAi" | "Anthropic" | "Gemini" | "OpenRouter";
type AiProviderId = SecretProvider | "Ollama";

type SecretEdits = Record<SecretKey, string>;
type SecretEditMode = Record<SecretKey, boolean>;
type SecretField = {
  key: SecretKey;
  provider: SecretProvider;
  labelKey: string;
  placeholder: string;
};
type AiProviderOption = {
  id: AiProviderId;
  label: string;
};
type LanguageOption = {
  value: AppConfig["language"];
  label?: string;
  labelKey?: "settings.language_system";
};
type SettingsCategoryId = "general" | "ai" | "logs" | "external_control";
type SettingsCategory = {
  id: SettingsCategoryId;
  labelKey: string;
};

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

const SECRET_FIELDS: SecretField[] = [
  { key: "openai", provider: "OpenAi", labelKey: "settings.openai_key", placeholder: "sk-..." },
  {
    key: "azure_openai",
    provider: "AzureOpenAi",
    labelKey: "settings.azure_openai_key",
    placeholder: "...",
  },
  {
    key: "anthropic",
    provider: "Anthropic",
    labelKey: "settings.anthropic_key",
    placeholder: "sk-ant-...",
  },
  { key: "gemini", provider: "Gemini", labelKey: "settings.gemini_key", placeholder: "AIza..." },
  {
    key: "openrouter",
    provider: "OpenRouter",
    labelKey: "settings.openrouter_key",
    placeholder: "sk-or-...",
  },
];

const AI_PROVIDER_OPTIONS: AiProviderOption[] = [
  { id: "OpenAi", label: "OpenAI" },
  { id: "AzureOpenAi", label: "Azure OpenAI" },
  { id: "Anthropic", label: "Anthropic" },
  { id: "Gemini", label: "Google Gemini" },
  { id: "OpenRouter", label: "OpenRouter" },
  { id: "Ollama", label: "Ollama" },
];

const createSecretStatus = (): AiSecretStatus => {
  const status = {} as AiSecretStatus;
  for (const { key } of SECRET_FIELDS) {
    status[key] = false;
  }
  return status;
};

const createSecretEdits = (): SecretEdits => {
  const edits = {} as SecretEdits;
  for (const { key } of SECRET_FIELDS) {
    edits[key] = "";
  }
  return edits;
};

const createSecretEditMode = (): SecretEditMode => {
  const editMode = {} as SecretEditMode;
  for (const { key } of SECRET_FIELDS) {
    editMode[key] = false;
  }
  return editMode;
};

const EMPTY_SECRET_STATUS = createSecretStatus();
const EMPTY_SECRET_EDITS = createSecretEdits();
const EMPTY_SECRET_EDIT_MODE = createSecretEditMode();

const LANGUAGE_OPTIONS: LanguageOption[] = [
  { value: "system", labelKey: "settings.language_system" },
  { value: "en", label: "English" },
  { value: "ja", label: "日本語" },
];
const LOG_FORMAT_OPTIONS: LogFormat[] = ["display", "strip_controls"];
const SETTINGS_CATEGORIES: SettingsCategory[] = [
  { id: "general", labelKey: "settings.category.general" },
  { id: "ai", labelKey: "settings.category.ai" },
  { id: "logs", labelKey: "settings.category.logs" },
  { id: "external_control", labelKey: "settings.category.external_control" },
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

function isLogFormat(value: string): value is LogFormat {
  return LOG_FORMAT_OPTIONS.includes(value as LogFormat);
}

function areSecretEditsEqual(left: SecretEdits, right: SecretEdits): boolean {
  return SECRET_FIELDS.every(({ key }) => left[key] === right[key]);
}

function areSecretEditModesEqual(left: SecretEditMode, right: SecretEditMode): boolean {
  return SECRET_FIELDS.every(({ key }) => left[key] === right[key]);
}

function areConfigsEqual(left: AppConfig | null, right: AppConfig | null): boolean {
  if (!left || !right) return left === right;
  return JSON.stringify(normalizeExternalControlConfig(left)) === JSON.stringify(right);
}

export default function SettingsPanel({ onSave }: SettingsPanelProps) {
  const { t, i18n } = useTranslation();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [initialConfigSnapshot, setInitialConfigSnapshot] = useState<AppConfig | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  const [loadFailed, setLoadFailed] = useState(false);
  const [isLoadingConfig, setIsLoadingConfig] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [secretStatus, setSecretStatus] = useState<AiSecretStatus>(EMPTY_SECRET_STATUS);
  const [secretEdits, setSecretEdits] = useState<SecretEdits>(EMPTY_SECRET_EDITS);
  const [initialSecretEditsSnapshot, setInitialSecretEditsSnapshot] =
    useState<SecretEdits>(EMPTY_SECRET_EDITS);
  const savedTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [secretEditMode, setSecretEditMode] = useState<SecretEditMode>(EMPTY_SECRET_EDIT_MODE);
  const [initialSecretEditModeSnapshot, setInitialSecretEditModeSnapshot] =
    useState<SecretEditMode>(EMPTY_SECRET_EDIT_MODE);
  const [activeCategory, setActiveCategory] = useState<SettingsCategoryId>("general");
  const [expandedOtherProvider, setExpandedOtherProvider] = useState<AiProviderId | null>(null);

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
      const normalizedConfig = normalizeExternalControlConfig(cfg);
      const emptySecretEdits = createSecretEdits();
      const emptySecretEditMode = createSecretEditMode();
      setConfig(normalizedConfig);
      setInitialConfigSnapshot(normalizedConfig);
      setSecretEdits(emptySecretEdits);
      setInitialSecretEditsSnapshot(emptySecretEdits);
      setSecretEditMode(emptySecretEditMode);
      setInitialSecretEditModeSnapshot(emptySecretEditMode);
      setSaved(false);
    } catch (e) {
      console.error("Failed to load settings:", e);
      setConfig(null);
      setInitialConfigSnapshot(null);
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

  const hasUnsavedChanges =
    !areConfigsEqual(config, initialConfigSnapshot) ||
    !areSecretEditsEqual(secretEdits, initialSecretEditsSnapshot) ||
    !areSecretEditModesEqual(secretEditMode, initialSecretEditModeSnapshot);

  const handleSave = async () => {
    if (!config || isSaving || !hasUnsavedChanges) return;
    try {
      setIsSaving(true);
      setError("");
      clearSavedTimer();
      const normalizedConfig = normalizeExternalControlConfig(config);
      setConfig(normalizedConfig);
      await invoke("config_save", { config: normalizedConfig });

      for (const { key, provider } of SECRET_FIELDS) {
        const value = secretEdits[key].trim();
        if (value) {
          await invoke("ai_secret_set", { provider, value });
        }
      }

      const emptySecretEdits = createSecretEdits();
      const emptySecretEditMode = createSecretEditMode();
      setSecretEdits(emptySecretEdits);
      setInitialSecretEditsSnapshot(emptySecretEdits);
      setSecretEditMode(emptySecretEditMode);
      setInitialSecretEditModeSnapshot(emptySecretEditMode);
      setInitialConfigSnapshot(normalizedConfig);
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

  const handleRevert = () => {
    if (!initialConfigSnapshot || isSaving) return;
    clearSavedTimer();
    setConfig(initialConfigSnapshot);
    setSecretEdits(initialSecretEditsSnapshot);
    setSecretEditMode(initialSecretEditModeSnapshot);
    setSaved(false);
    setError("");
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

  const updateLanguage = (language: AppConfig["language"]) => {
    setConfig((prev) => (prev ? { ...prev, language } : prev));
  };

  const updateAiConfig = (patch: Partial<AppConfig["ai"]>) => {
    setConfig((prev) => (prev ? { ...prev, ai: { ...prev.ai, ...patch } } : prev));
  };

  const updateExternalControlConfig = (patch: Partial<AppConfig["external_control"]>) => {
    setConfig((prev) =>
      prev ? { ...prev, external_control: { ...prev.external_control, ...patch } } : prev
    );
  };

  const updateTerminalConfig = (patch: Partial<AppConfig["terminal"]>) => {
    setConfig((prev) => (prev ? { ...prev, terminal: { ...prev.terminal, ...patch } } : prev));
  };

  const updateSshConfig = (patch: Partial<AppConfig["ssh"]>) => {
    setConfig((prev) => (prev ? { ...prev, ssh: { ...prev.ssh, ...patch } } : prev));
  };

  const clearSecret = async (provider: SecretProvider, key: SecretKey) => {
    const confirmed = await confirm(t("settings.secret_clear_confirm_message"), {
      title: t("settings.secret_clear_confirm_title"),
      kind: "warning",
      okLabel: t("settings.clear"),
      cancelLabel: t("settings.cancel"),
    });

    if (!confirmed) return;

    try {
      setError("");
      await invoke("ai_secret_clear", { provider });
      setSecretEdits((prev) => ({ ...prev, [key]: "" }));
      setSecretEditMode((prev) => ({ ...prev, [key]: false }));
      setSecretStatus((prev) => ({ ...prev, [key]: false }));
    } catch {
      console.error("Failed to clear AI secret.");
      setError(t("settings.secret_clear_failed"));
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

  const getProviderSecretField = (provider: AiProviderId): SecretField | undefined =>
    SECRET_FIELDS.find((field) => field.provider === provider);

  const getProviderStatus = (provider: AiProviderId): boolean => {
    if (provider === "Ollama") return Boolean(config.ai.ollama_enabled);
    const secretField = getProviderSecretField(provider);
    return Boolean(secretField && secretStatus[secretField.key]);
  };

  const renderSecretConfiguration = ({ key, provider, labelKey, placeholder }: SecretField) => {
    const hasSecret = secretStatus[key];
    const isEditing = secretEditMode[key];

    return (
      <div className="settings-provider-detail__secret">
        <div className="settings-provider-detail__summary">
          <span className="label">{t(labelKey)}</span>
          <span
            className={`settings-provider-status ${
              hasSecret
                ? "settings-provider-status--configured"
                : "settings-provider-status--unconfigured"
            }`}
          >
            {t(hasSecret ? "settings.configured" : "settings.not_configured")}
          </span>
        </div>
        <div className="settings-secret-row">
          {isEditing && (
            <input
              className="input"
              type="password"
              value={secretEdits[key]}
              onChange={(e) => {
                setSecretEdits((prev) => ({ ...prev, [key]: e.target.value }));
              }}
              placeholder={placeholder}
              aria-label={t(labelKey)}
            />
          )}
          {!isEditing && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => beginEditSecret(key)}
            >
              {t(hasSecret ? "settings.change" : "settings.configure")}
            </button>
          )}
          {hasSecret && !isEditing && (
            <button
              type="button"
              className="btn btn-danger btn-sm"
              onClick={() => clearSecret(provider, key)}
            >
              {t("settings.clear")}
            </button>
          )}
          {isEditing && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => cancelEditSecret(key)}
            >
              {t("settings.cancel")}
            </button>
          )}
        </div>
      </div>
    );
  };

  const renderAzureOpenAiConfiguration = () => (
    <div className="settings-provider-detail__options">
      <div className="settings-toggle-row">
        <div className="settings-toggle-label">
          <span>{t("settings.azure_openai_enabled")}</span>
          <small>{t("settings.azure_openai_enabled_desc")}</small>
        </div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={Boolean(config.ai.azure_openai_enabled)}
            onChange={(e) => updateAiConfig({ azure_openai_enabled: e.target.checked })}
          />
          <span className="toggle-track" />
        </label>
      </div>

      <div className="settings-provider-detail__field">
        <label className="label">{t("settings.azure_openai_endpoint")}</label>
        <input
          className="input"
          type="text"
          value={config.ai.azure_openai_endpoint}
          onChange={(e) => updateAiConfig({ azure_openai_endpoint: e.target.value })}
          placeholder="https://your-resource.openai.azure.com/openai/v1/chat/completions"
          disabled={!config.ai.azure_openai_enabled}
        />
      </div>

      <div className="settings-provider-detail__field">
        <label className="label">{t("settings.azure_openai_deployment")}</label>
        <input
          className="input"
          type="text"
          value={config.ai.azure_openai_deployment}
          onChange={(e) => updateAiConfig({ azure_openai_deployment: e.target.value })}
          placeholder="my-gpt4o-deployment"
          disabled={!config.ai.azure_openai_enabled}
        />
      </div>
    </div>
  );

  const renderOllamaConfiguration = () => (
    <div className="settings-provider-detail__options">
      <div className="settings-toggle-row">
        <div className="settings-toggle-label">
          <span>{t("settings.ollama_enabled")}</span>
          <small>{t("settings.ollama_enabled_desc")}</small>
        </div>
        <label className="toggle">
          <input
            type="checkbox"
            checked={Boolean(config.ai.ollama_enabled)}
            onChange={(e) => updateAiConfig({ ollama_enabled: e.target.checked })}
          />
          <span className="toggle-track" />
        </label>
      </div>

      <div className="settings-provider-detail__field">
        <label className="label">{t("settings.ollama_url")}</label>
        <input
          className="input"
          type="text"
          value={config.ai.ollama_base_url}
          onChange={(e) => updateAiConfig({ ollama_base_url: e.target.value })}
          placeholder="http://localhost:11434"
          disabled={!config.ai.ollama_enabled}
        />
      </div>
    </div>
  );

  const renderProviderDetails = (provider: AiProviderId) => {
    const secretField = getProviderSecretField(provider);

    return (
      <div className="settings-provider-detail">
        {secretField && renderSecretConfiguration(secretField)}
        {provider === "AzureOpenAi" && renderAzureOpenAiConfiguration()}
        {provider === "Ollama" && renderOllamaConfiguration()}
      </div>
    );
  };

  const renderActiveCategory = () => {
    switch (activeCategory) {
      case "general":
        return (
          <div className="settings-section">
            <div className="settings-section__title">{t("settings.language")}</div>
            <div className="settings-row">
              <div>
                <select
                  className="select"
                  value={config.language}
                  onChange={(e) => {
                    updateLanguage(e.target.value);
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

            <div className="settings-section__title">{t("settings.terminal_settings")}</div>
            <div className="settings-row">
              <div>
                <label className="label">{t("settings.font_size")}</label>
                <input
                  className="input"
                  type="number"
                  value={config.terminal.font_size}
                  onChange={(e) => {
                    updateTerminalConfig({
                      font_size: parseBoundedNumber(
                        e.target.value,
                        config.terminal.font_size,
                        FONT_SIZE_MIN,
                        FONT_SIZE_MAX
                      ),
                    });
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
                    updateTerminalConfig({
                      scrollback: parseBoundedNumber(
                        e.target.value,
                        config.terminal.scrollback,
                        SCROLLBACK_MIN,
                        SCROLLBACK_MAX
                      ),
                    });
                  }}
                  min={SCROLLBACK_MIN}
                  max={SCROLLBACK_MAX}
                />
              </div>
            </div>
            <div style={{ marginBottom: 14 }}>
              <label className="label">{t("settings.font_family")}</label>
              <input
                className="input"
                value={config.terminal.font_family}
                onChange={(e) => {
                  updateTerminalConfig({ font_family: e.target.value });
                }}
              />
            </div>

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
                    updateSshConfig({ allow_legacy_algorithms: e.target.checked });
                  }}
                />
                <span className="toggle-track" />
              </label>
            </div>
          </div>
        );

      case "ai":
        const defaultProvider =
          AI_PROVIDER_OPTIONS.find((provider) => provider.id === config.ai.default_provider) ??
          AI_PROVIDER_OPTIONS[0];
        const otherProviders = AI_PROVIDER_OPTIONS.filter(
          (provider) => provider.id !== defaultProvider.id
        );

        return (
          <div className="settings-section">
            <div className="settings-section__title">{t("settings.ai_provider")}</div>
            <div className="settings-provider-card">
              <div className="settings-provider-card__header">
                <div className="settings-provider-card__selector">
                  <label className="label">{t("settings.default_provider")}</label>
                  <select
                    className="select"
                    value={config.ai.default_provider}
                    onChange={(e) => {
                      updateAiConfig({ default_provider: e.target.value });
                    }}
                  >
                    {AI_PROVIDER_OPTIONS.map((provider) => (
                      <option key={provider.id} value={provider.id}>
                        {provider.label}
                      </option>
                    ))}
                  </select>
                </div>
                <span
                  className={`settings-provider-status ${
                    getProviderStatus(defaultProvider.id)
                      ? "settings-provider-status--configured"
                      : "settings-provider-status--unconfigured"
                  }`}
                >
                  {t(
                    getProviderStatus(defaultProvider.id)
                      ? defaultProvider.id === "Ollama"
                        ? "settings.enabled"
                        : "settings.configured"
                      : defaultProvider.id === "Ollama"
                        ? "settings.disabled"
                        : "settings.not_configured"
                  )}
                </span>
              </div>
              <div className="settings-provider-card__body">
                <div className="settings-provider-card__title">{defaultProvider.label}</div>
                {renderProviderDetails(defaultProvider.id)}
              </div>
            </div>

            <div className="settings-provider-list">
              <div className="settings-provider-list__header">
                <span>{t("settings.other_ai_providers")}</span>
                <small>{t("settings.ai_provider_status_note")}</small>
              </div>
              {otherProviders.map((provider) => {
                const isExpanded = expandedOtherProvider === provider.id;
                const isConfigured = getProviderStatus(provider.id);
                const statusKey = isConfigured
                  ? provider.id === "Ollama"
                    ? "settings.enabled"
                    : "settings.configured"
                  : provider.id === "Ollama"
                    ? "settings.disabled"
                    : "settings.not_configured";

                return (
                  <div key={provider.id} className="settings-provider-list__item">
                    <div className="settings-provider-list__row">
                      <span className="settings-provider-list__name">{provider.label}</span>
                      <span
                        className={`settings-provider-status ${
                          isConfigured
                            ? "settings-provider-status--configured"
                            : "settings-provider-status--unconfigured"
                        }`}
                      >
                        {t(statusKey)}
                      </span>
                      <button
                        type="button"
                        className="btn btn-ghost btn-sm"
                        aria-expanded={isExpanded}
                        onClick={() => {
                          setExpandedOtherProvider((current) =>
                            current === provider.id ? null : provider.id
                          );
                        }}
                      >
                        {t(isConfigured ? "settings.change" : "settings.configure")}
                      </button>
                    </div>
                    {isExpanded && renderProviderDetails(provider.id)}
                  </div>
                );
              })}
            </div>
          </div>
        );

      case "external_control":
        return (
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
                    updateExternalControlConfig({ enabled: e.target.checked });
                  }}
                />
                <span className="toggle-track" />
              </label>
            </div>
            {!config.external_control.enabled && (
              <p className="settings-help">{t("settings.mcp_children_disabled_desc")}</p>
            )}
            <div className="settings-toggle-row">
              <div className="settings-toggle-label">
                <span>{t("settings.mcp_cli_enabled")}</span>
                <small>{t("settings.mcp_cli_enabled_desc")}</small>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={Boolean(config.external_control.cli_enabled)}
                  disabled={!config.external_control.enabled}
                  onChange={(e) => {
                    updateExternalControlConfig({ cli_enabled: e.target.checked });
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
                  disabled={!config.external_control.enabled}
                  onChange={(e) => {
                    updateExternalControlConfig({ connect_enabled: e.target.checked });
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
                  disabled={!config.external_control.enabled}
                  onChange={(e) => {
                    updateExternalControlConfig({ mcp_enabled: e.target.checked });
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
        );

      case "logs":
        return (
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
                    updateTerminalConfig({ auto_session_log: e.target.checked });
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
                  if (isLogFormat(e.target.value)) {
                    updateTerminalConfig({ log_format: e.target.value });
                  }
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
                    updateTerminalConfig({ include_log_header: e.target.checked });
                  }}
                />
                <span className="toggle-track" />
              </label>
            </div>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className="settings-panel">
      <h2>{t("settings.title")}</h2>

      <div className="settings-category-select">
        <label className="label" htmlFor="settings-category-select">
          {t("settings.category_select_label")}
        </label>
        <select
          id="settings-category-select"
          className="select"
          value={activeCategory}
          onChange={(e) => {
            setActiveCategory(e.target.value as SettingsCategoryId);
          }}
        >
          {SETTINGS_CATEGORIES.map((category) => (
            <option key={category.id} value={category.id}>
              {t(category.labelKey)}
            </option>
          ))}
        </select>
      </div>

      <div className="settings-layout">
        <nav className="settings-category-nav" aria-label={t("settings.category_select_label")}>
          {SETTINGS_CATEGORIES.map((category) => (
            <button
              key={category.id}
              type="button"
              className={`settings-category-button ${
                activeCategory === category.id ? "settings-category-button--active" : ""
              }`}
              aria-current={activeCategory === category.id ? "page" : undefined}
              onClick={() => {
                setActiveCategory(category.id);
              }}
            >
              {t(category.labelKey)}
            </button>
          ))}
        </nav>

        <div className="settings-content">{renderActiveCategory()}</div>
      </div>

      <div className="settings-actions">
        <div className="settings-actions__status">
          {hasUnsavedChanges && (
            <span className="settings-unsaved">{t("settings.unsaved_changes")}</span>
          )}
          {saved && (
            <span className="settings-saved">
              <Check size={14} /> {t("settings.saved")}
            </span>
          )}
          {error && <span className="settings-error">{error}</span>}
        </div>
        <div className="settings-actions__buttons">
          {hasUnsavedChanges && (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={handleRevert}
              disabled={isSaving}
            >
              {t("settings.revert")}
            </button>
          )}
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleSave}
            disabled={isSaving || !hasUnsavedChanges}
          >
            {isSaving
              ? t("settings.saving")
              : hasUnsavedChanges
                ? t("settings.save_changes")
                : t("settings.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
