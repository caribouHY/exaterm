import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { translateBackendCommandError } from "../../features/backend-errors/backendCommandError";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { AiSecretStatus, AppConfig, SshAlgorithmCatalog } from "../../types";
import { AiSettings } from "./AiSettings";
import { ExternalControlSettings } from "./ExternalControlSettings";
import { GeneralSettings } from "./GeneralSettings";
import { LogSettings } from "./LogSettings";
import { SettingsError, SettingsFooter } from "./SettingsFooter";
import { SettingsSidebar } from "./SettingsSidebar";
import { ShortcutsSettings } from "./ShortcutsSettings";
import {
  SECRET_FIELDS,
  areConfigsEqual,
  areSecretEditModesEqual,
  areSecretEditsEqual,
  createSecretEditMode,
  createSecretEdits,
  createSecretStatus,
  getSecretEdit,
  normalizeExternalControlConfig,
  type AiProviderId,
  type SecretEditMode,
  type SecretEdits,
  type SecretKey,
  type SecretProvider,
  type SettingsCategoryId,
} from "./settingsModel";
import "./SettingsPanel.css";

export default function SettingsPanel() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [initialConfigSnapshot, setInitialConfigSnapshot] = useState<AppConfig | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  const [loadFailed, setLoadFailed] = useState(false);
  const [isLoadingConfig, setIsLoadingConfig] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [sshAlgorithmCatalog, setSshAlgorithmCatalog] = useState<SshAlgorithmCatalog | null>(null);
  const [sshAlgorithmCatalogLoadFailed, setSshAlgorithmCatalogLoadFailed] = useState(false);
  const [secretStatus, setSecretStatus] = useState<AiSecretStatus>(createSecretStatus);
  const [secretEdits, setSecretEdits] = useState<SecretEdits>(createSecretEdits);
  const [initialSecretEditsSnapshot, setInitialSecretEditsSnapshot] =
    useState<SecretEdits>(createSecretEdits);
  const [secretEditMode, setSecretEditMode] = useState<SecretEditMode>(createSecretEditMode);
  const [initialSecretEditModeSnapshot, setInitialSecretEditModeSnapshot] =
    useState<SecretEditMode>(createSecretEditMode);
  const [activeCategory, setActiveCategory] = useState<SettingsCategoryId>("general");
  const [expandedOtherProvider, setExpandedOtherProvider] = useState<AiProviderId | null>(null);
  const savedTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const refreshSecretStatus = async () => {
    try {
      setSecretStatus(await invoke<AiSecretStatus>("ai_secret_status"));
    } catch (refreshError) {
      console.error("Failed to load AI secret status:", refreshError);
      setSecretStatus(createSecretStatus());
    }
  };

  const loadSshAlgorithmCatalog = async () => {
    setSshAlgorithmCatalogLoadFailed(false);
    try {
      setSshAlgorithmCatalog(await invoke<SshAlgorithmCatalog>("ssh_algorithm_catalog"));
    } catch (catalogError) {
      console.error("Failed to load SSH algorithm catalog:", catalogError);
      setSshAlgorithmCatalog(null);
      setSshAlgorithmCatalogLoadFailed(true);
    }
  };

  const clearSavedTimer = () => {
    if (!savedTimeoutRef.current) return;
    clearTimeout(savedTimeoutRef.current);
    savedTimeoutRef.current = null;
  };

  const loadConfig = async () => {
    setIsLoadingConfig(true);
    setLoadFailed(false);
    setError("");
    try {
      const normalizedConfig = normalizeExternalControlConfig(
        await invoke<AppConfig>("config_load")
      );
      const emptySecretEdits = createSecretEdits();
      const emptySecretEditMode = createSecretEditMode();
      setConfig(normalizedConfig);
      setInitialConfigSnapshot(normalizedConfig);
      setSecretEdits(emptySecretEdits);
      setInitialSecretEditsSnapshot(emptySecretEdits);
      setSecretEditMode(emptySecretEditMode);
      setInitialSecretEditModeSnapshot(emptySecretEditMode);
      setSaved(false);
    } catch (loadError) {
      console.error("Failed to load settings:", loadError);
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
    void loadSshAlgorithmCatalog();
    return clearSavedTimer;
  }, []);

  const hasUnsavedChanges =
    !areConfigsEqual(config, initialConfigSnapshot) ||
    !areSecretEditsEqual(secretEdits, initialSecretEditsSnapshot) ||
    !areSecretEditModesEqual(secretEditMode, initialSecretEditModeSnapshot);

  const handleSave = async () => {
    if (!config || !initialConfigSnapshot || isSaving || !hasUnsavedChanges) return;
    try {
      setIsSaving(true);
      setError("");
      clearSavedTimer();
      const normalizedConfig = normalizeExternalControlConfig(config);
      setConfig(normalizedConfig);
      const savedConfig = normalizeExternalControlConfig(
        await invoke<AppConfig>("config_save", {
          baseConfig: initialConfigSnapshot,
          editedConfig: normalizedConfig,
        })
      );
      setConfig(savedConfig);
      setInitialConfigSnapshot(savedConfig);

      for (const { key, provider } of SECRET_FIELDS) {
        const value = getSecretEdit(secretEdits, key).trim();
        if (value) await invoke("ai_secret_set", { provider, value });
      }

      const emptySecretEdits = createSecretEdits();
      const emptySecretEditMode = createSecretEditMode();
      setSecretEdits(emptySecretEdits);
      setInitialSecretEditsSnapshot(emptySecretEdits);
      setSecretEditMode(emptySecretEditMode);
      setInitialSecretEditModeSnapshot(emptySecretEditMode);
      await refreshSecretStatus();

      setSaved(true);
      savedTimeoutRef.current = setTimeout(() => {
        setSaved(false);
        savedTimeoutRef.current = null;
      }, 2000);
    } catch (saveError) {
      console.error(saveError);
      setError(translateBackendCommandError(saveError, t, t("settings.save_failed")));
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

  const updateLanguage = (language: AppConfig["language"]) => {
    setConfig((previous) => (previous ? { ...previous, language } : previous));
  };

  const updateAiConfig = (patch: Partial<AppConfig["ai"]>) => {
    setConfig((previous) =>
      previous ? { ...previous, ai: { ...previous.ai, ...patch } } : previous
    );
  };

  const updateUpdateConfig = (patch: Partial<AppConfig["updates"]>) => {
    setConfig((previous) =>
      previous ? { ...previous, updates: { ...previous.updates, ...patch } } : previous
    );
  };

  const updateExternalControlConfig = (patch: Partial<AppConfig["external_control"]>) => {
    setConfig((previous) =>
      previous
        ? { ...previous, external_control: { ...previous.external_control, ...patch } }
        : previous
    );
  };

  const updateConnectionHistoryConfig = (patch: Partial<AppConfig["connection_history"]>) => {
    setConfig((previous) =>
      previous
        ? { ...previous, connection_history: { ...previous.connection_history, ...patch } }
        : previous
    );
  };

  const updateTerminalConfig = (patch: Partial<AppConfig["terminal"]>) => {
    setConfig((previous) =>
      previous ? { ...previous, terminal: { ...previous.terminal, ...patch } } : previous
    );
  };

  const updateSshConfig = (patch: Partial<AppConfig["ssh"]>) => {
    setConfig((previous) =>
      previous ? { ...previous, ssh: { ...previous.ssh, ...patch } } : previous
    );
  };

  const updateShortcutConfig = (shortcuts: AppConfig["shortcuts"]) => {
    setConfig((previous) => (previous ? { ...previous, shortcuts } : previous));
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
      setSecretEdits((previous) => ({ ...previous, [key]: "" }));
      setSecretEditMode((previous) => ({ ...previous, [key]: false }));
      setSecretStatus((previous) => ({ ...previous, [key]: false }));
    } catch {
      console.error("Failed to clear AI secret.");
      setError(t("settings.secret_clear_failed"));
    }
  };

  if (!config) {
    return (
      <div className="settings-panel">
        {isLoadingConfig && <p>{t("settings.loading")}</p>}
        {loadFailed && (
          <>
            <SettingsError message={t("settings.load_failed")} />
            <button className="btn btn-primary" onClick={() => void loadConfig()}>
              {t("settings.reload")}
            </button>
          </>
        )}
      </div>
    );
  }

  const renderActiveCategory = () => {
    switch (activeCategory) {
      case "general":
        return (
          <GeneralSettings
            language={config.language}
            updateConfig={config.updates}
            connectionHistoryConfig={config.connection_history}
            terminalConfig={config.terminal}
            sshConfig={config.ssh}
            sshAlgorithmCatalog={sshAlgorithmCatalog}
            sshAlgorithmCatalogLoadFailed={sshAlgorithmCatalogLoadFailed}
            onLanguageChange={updateLanguage}
            onUpdateChange={updateUpdateConfig}
            onConnectionHistoryChange={updateConnectionHistoryConfig}
            onTerminalChange={updateTerminalConfig}
            onSshChange={updateSshConfig}
            onReloadSshAlgorithmCatalog={() => void loadSshAlgorithmCatalog()}
          />
        );
      case "ai":
        return (
          <AiSettings
            config={config.ai}
            secretStatus={secretStatus}
            secretEdits={secretEdits}
            secretEditMode={secretEditMode}
            expandedOtherProvider={expandedOtherProvider}
            onConfigChange={updateAiConfig}
            onExpandedOtherProviderChange={(provider) => {
              setExpandedOtherProvider(provider);
            }}
            onSecretValueChange={(key, value) => {
              setSecretEdits((previous) => ({ ...previous, [key]: value }));
            }}
            onBeginSecretEdit={(key) => {
              setSecretEditMode((previous) => ({ ...previous, [key]: true }));
              setSecretEdits((previous) => ({ ...previous, [key]: "" }));
            }}
            onCancelSecretEdit={(key) => {
              setSecretEditMode((previous) => ({ ...previous, [key]: false }));
              setSecretEdits((previous) => ({ ...previous, [key]: "" }));
            }}
            onClearSecret={(provider, key) => void clearSecret(provider, key)}
          />
        );
      case "shortcuts":
        return <ShortcutsSettings config={config.shortcuts} onChange={updateShortcutConfig} />;
      case "logs":
        return <LogSettings config={config.terminal} onChange={updateTerminalConfig} />;
      case "external_control":
        return (
          <ExternalControlSettings
            config={config.external_control}
            onChange={updateExternalControlConfig}
          />
        );
    }
  };

  return (
    <div className="settings-panel">
      <h2>{t("settings.title")}</h2>
      <div className="settings-layout">
        <SettingsSidebar activeCategory={activeCategory} onCategoryChange={setActiveCategory} />
        <div className="settings-content">{renderActiveCategory()}</div>
      </div>
      <SettingsFooter
        hasUnsavedChanges={hasUnsavedChanges}
        saved={saved}
        error={error}
        isSaving={isSaving}
        onSave={() => void handleSave()}
        onRevert={handleRevert}
      />
    </div>
  );
}
