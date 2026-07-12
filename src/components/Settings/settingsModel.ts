import type { AiSecretStatus, AppConfig } from "../../types";

export type SecretKey = keyof AiSecretStatus;
export type SecretProvider = "OpenAi" | "AzureOpenAi" | "Anthropic" | "Gemini" | "OpenRouter";
export type AiProviderId = SecretProvider | "Ollama";
export type SecretEdits = Record<SecretKey, string>;
export type SecretEditMode = Record<SecretKey, boolean>;

export type SecretFieldDefinition = {
  key: SecretKey;
  provider: SecretProvider;
  labelKey: string;
  placeholder: string;
};

export type SettingsCategoryId = "general" | "ai" | "logs" | "external_control";

export const SETTINGS_CATEGORIES: Array<{ id: SettingsCategoryId; labelKey: string }> = [
  { id: "general", labelKey: "settings.category.general" },
  { id: "ai", labelKey: "settings.category.ai" },
  { id: "logs", labelKey: "settings.category.logs" },
  { id: "external_control", labelKey: "settings.category.external_control" },
];

export const SECRET_FIELDS: SecretFieldDefinition[] = [
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

export function createSecretStatus(): AiSecretStatus {
  const status = {} as AiSecretStatus;
  for (const { key } of SECRET_FIELDS) status[key] = false;
  return status;
}

export function createSecretEdits(): SecretEdits {
  const edits = {} as SecretEdits;
  for (const { key } of SECRET_FIELDS) edits[key] = "";
  return edits;
}

export function createSecretEditMode(): SecretEditMode {
  const editMode = {} as SecretEditMode;
  for (const { key } of SECRET_FIELDS) editMode[key] = false;
  return editMode;
}

export function normalizeExternalControlConfig(config: AppConfig): AppConfig {
  const externalControl = config.external_control as
    | Partial<AppConfig["external_control"]>
    | undefined;

  return {
    ...config,
    external_control: {
      enabled: false,
      connect_enabled: false,
      mcp_enabled: false,
      cli_enabled: false,
      ...(externalControl ?? {}),
    },
  };
}

export function areSecretEditsEqual(left: SecretEdits, right: SecretEdits): boolean {
  return SECRET_FIELDS.every(({ key }) => left[key] === right[key]);
}

export function areSecretEditModesEqual(left: SecretEditMode, right: SecretEditMode): boolean {
  return SECRET_FIELDS.every(({ key }) => left[key] === right[key]);
}

export function areConfigsEqual(left: AppConfig | null, right: AppConfig | null): boolean {
  if (!left || !right) return left === right;
  return (
    JSON.stringify(normalizeExternalControlConfig(left)) ===
    JSON.stringify(normalizeExternalControlConfig(right))
  );
}
