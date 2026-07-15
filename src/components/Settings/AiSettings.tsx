import { useTranslation } from "react-i18next";
import type { AiSecretStatus, AppConfig } from "../../types";
import { SecretField } from "./SecretField";
import { SettingsToggle } from "./SettingsToggle";
import {
  SECRET_FIELDS,
  type AiProviderId,
  type SecretEditMode,
  type SecretEdits,
  type SecretKey,
  type SecretProvider,
} from "./settingsModel";

interface AiSettingsProps {
  config: AppConfig["ai"];
  secretStatus: AiSecretStatus;
  secretEdits: SecretEdits;
  secretEditMode: SecretEditMode;
  expandedOtherProvider: AiProviderId | null;
  onConfigChange: (patch: Partial<AppConfig["ai"]>) => void;
  onExpandedOtherProviderChange: (provider: AiProviderId | null) => void;
  onSecretValueChange: (key: SecretKey, value: string) => void;
  onBeginSecretEdit: (key: SecretKey) => void;
  onCancelSecretEdit: (key: SecretKey) => void;
  onClearSecret: (provider: SecretProvider, key: SecretKey) => void;
}

const AI_PROVIDER_OPTIONS: Array<{ id: AiProviderId; label: string }> = [
  { id: "OpenAi", label: "OpenAI" },
  { id: "AzureOpenAi", label: "Azure OpenAI" },
  { id: "Anthropic", label: "Anthropic" },
  { id: "Gemini", label: "Google Gemini" },
  { id: "OpenRouter", label: "OpenRouter" },
  { id: "Ollama", label: "Ollama" },
];

export function AiSettings({
  config,
  secretStatus,
  secretEdits,
  secretEditMode,
  expandedOtherProvider,
  onConfigChange,
  onExpandedOtherProviderChange,
  onSecretValueChange,
  onBeginSecretEdit,
  onCancelSecretEdit,
  onClearSecret,
}: AiSettingsProps) {
  const { t } = useTranslation();

  const getProviderSecretField = (provider: AiProviderId) =>
    SECRET_FIELDS.find((field) => field.provider === provider);

  const getProviderStatus = (provider: AiProviderId): boolean => {
    if (provider === "Ollama") return Boolean(config.ollama_enabled);
    const secretField = getProviderSecretField(provider);
    return Boolean(secretField && secretStatus[secretField.key]);
  };

  const renderProviderDetails = (provider: AiProviderId) => {
    const secretField = getProviderSecretField(provider);

    return (
      <div className="settings-provider-detail">
        {secretField && (
          <SecretField
            field={secretField}
            hasSecret={secretStatus[secretField.key]}
            isEditing={secretEditMode[secretField.key]}
            value={secretEdits[secretField.key]}
            onValueChange={(value) => {
              onSecretValueChange(secretField.key, value);
            }}
            onBeginEdit={() => {
              onBeginSecretEdit(secretField.key);
            }}
            onCancelEdit={() => {
              onCancelSecretEdit(secretField.key);
            }}
            onClear={() => {
              onClearSecret(secretField.provider, secretField.key);
            }}
          />
        )}
        {provider === "AzureOpenAi" && (
          <div className="settings-provider-detail__options">
            <SettingsToggle
              id="settings-azure-openai-enabled"
              label={t("settings.azure_openai_enabled")}
              description={t("settings.azure_openai_enabled_desc")}
              checked={Boolean(config.azure_openai_enabled)}
              onChange={(azure_openai_enabled) => {
                onConfigChange({ azure_openai_enabled });
              }}
            />
            <div className="settings-provider-detail__field">
              <label className="label" htmlFor="settings-azure-openai-endpoint">
                {t("settings.azure_openai_endpoint")}
              </label>
              <input
                className="input"
                id="settings-azure-openai-endpoint"
                type="text"
                value={config.azure_openai_endpoint}
                onChange={(event) => {
                  onConfigChange({ azure_openai_endpoint: event.target.value });
                }}
                placeholder="https://your-resource.openai.azure.com/openai/v1/chat/completions"
                disabled={!config.azure_openai_enabled}
              />
            </div>
            <div className="settings-provider-detail__field">
              <label className="label" htmlFor="settings-azure-openai-deployment">
                {t("settings.azure_openai_deployment")}
              </label>
              <input
                className="input"
                id="settings-azure-openai-deployment"
                type="text"
                value={config.azure_openai_deployment}
                onChange={(event) => {
                  onConfigChange({ azure_openai_deployment: event.target.value });
                }}
                placeholder="my-gpt4o-deployment"
                disabled={!config.azure_openai_enabled}
              />
            </div>
          </div>
        )}
        {provider === "Ollama" && (
          <div className="settings-provider-detail__options">
            <SettingsToggle
              id="settings-ollama-enabled"
              label={t("settings.ollama_enabled")}
              description={t("settings.ollama_enabled_desc")}
              checked={Boolean(config.ollama_enabled)}
              onChange={(ollama_enabled) => {
                onConfigChange({ ollama_enabled });
              }}
            />
            <div className="settings-provider-detail__field">
              <label className="label" htmlFor="settings-ollama-url">
                {t("settings.ollama_url")}
              </label>
              <input
                className="input"
                id="settings-ollama-url"
                type="text"
                value={config.ollama_base_url}
                onChange={(event) => {
                  onConfigChange({ ollama_base_url: event.target.value });
                }}
                placeholder="http://localhost:11434"
                disabled={!config.ollama_enabled}
              />
            </div>
          </div>
        )}
      </div>
    );
  };

  const defaultProvider =
    AI_PROVIDER_OPTIONS.find((provider) => provider.id === config.default_provider) ??
    AI_PROVIDER_OPTIONS[0];
  const otherProviders = AI_PROVIDER_OPTIONS.filter(
    (provider) => provider.id !== defaultProvider.id
  );

  const getStatusKey = (provider: AiProviderId) => {
    const configured = getProviderStatus(provider);
    if (provider === "Ollama") return configured ? "settings.enabled" : "settings.disabled";
    return configured ? "settings.configured" : "settings.not_configured";
  };

  return (
    <div className="settings-section">
      <div className="settings-section__title">{t("settings.ai_provider")}</div>
      <div className="settings-provider-card">
        <div className="settings-provider-card__header">
          <div className="settings-provider-card__selector">
            <label className="label" htmlFor="settings-default-provider">
              {t("settings.default_provider")}
            </label>
            <select
              id="settings-default-provider"
              className="select"
              value={config.default_provider}
              onChange={(event) => {
                onConfigChange({ default_provider: event.target.value });
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
            {t(getStatusKey(defaultProvider.id))}
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
                  {t(getStatusKey(provider.id))}
                </span>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  aria-expanded={isExpanded}
                  onClick={() => {
                    onExpandedOtherProviderChange(isExpanded ? null : provider.id);
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
}
