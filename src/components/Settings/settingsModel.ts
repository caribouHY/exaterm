import type { AiSecretStatus, AppConfig } from "../../types";
import { normalizeShortcutConfig } from "../../features/shortcuts/shortcutModel";

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

export type SettingsCategoryId = "general" | "shortcuts" | "ai" | "logs" | "external_control";

export const SETTINGS_CATEGORIES: Array<{ id: SettingsCategoryId; labelKey: string }> = [
  { id: "general", labelKey: "settings.category.general" },
  { id: "shortcuts", labelKey: "settings.category.shortcuts" },
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
  return {
    openai: false,
    azure_openai: false,
    anthropic: false,
    gemini: false,
    openrouter: false,
  };
}

export function createSecretEdits(): SecretEdits {
  return {
    openai: "",
    azure_openai: "",
    anthropic: "",
    gemini: "",
    openrouter: "",
  };
}

export function createSecretEditMode(): SecretEditMode {
  return {
    openai: false,
    azure_openai: false,
    anthropic: false,
    gemini: false,
    openrouter: false,
  };
}

export function getSecretEdit(edits: SecretEdits, key: SecretKey): string {
  switch (key) {
    case "openai":
      return edits.openai;
    case "azure_openai":
      return edits.azure_openai;
    case "anthropic":
      return edits.anthropic;
    case "gemini":
      return edits.gemini;
    case "openrouter":
      return edits.openrouter;
  }
}

export function normalizeExternalControlConfig(config: AppConfig): AppConfig {
  const externalControl = config.external_control as
    | Partial<AppConfig["external_control"]>
    | undefined;

  return {
    ...config,
    shortcuts: normalizeShortcutConfig(config.shortcuts),
    external_control: {
      enabled: false,
      connect_enabled: false,
      direct_connect_enabled: false,
      mcp_enabled: false,
      cli_enabled: false,
      ...(externalControl ?? {}),
    },
  };
}

export function isDirectConnectControlDisabled(
  externalControlEnabled: boolean,
  connectEnabled: boolean
): boolean {
  return !externalControlEnabled || !connectEnabled;
}

export function areSecretEditsEqual(left: SecretEdits, right: SecretEdits): boolean {
  return (
    left.openai === right.openai &&
    left.azure_openai === right.azure_openai &&
    left.anthropic === right.anthropic &&
    left.gemini === right.gemini &&
    left.openrouter === right.openrouter
  );
}

export function areSecretEditModesEqual(left: SecretEditMode, right: SecretEditMode): boolean {
  return (
    left.openai === right.openai &&
    left.azure_openai === right.azure_openai &&
    left.anthropic === right.anthropic &&
    left.gemini === right.gemini &&
    left.openrouter === right.openrouter
  );
}

export function areConfigsEqual(left: AppConfig | null, right: AppConfig | null): boolean {
  if (!left || !right) return left === right;
  return (
    JSON.stringify(normalizeExternalControlConfig(left)) ===
    JSON.stringify(normalizeExternalControlConfig(right))
  );
}
