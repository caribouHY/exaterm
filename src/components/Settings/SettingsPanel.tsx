import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { createSettingsSaveController } from "./settingsSaveController";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import type { AppConfig, SshAlgorithmCatalog } from "../../types";
import { AiSettings } from "./AiSettings";
import { ExternalControlSettings } from "./ExternalControlSettings";
import { GeneralSettings } from "./GeneralSettings";
import { LogSettings } from "./LogSettings";
import { SettingsError, SettingsFooter } from "./SettingsFooter";
import { SettingsSidebar } from "./SettingsSidebar";
import { ShortcutsSettings } from "./ShortcutsSettings";
import {
  type AiProviderId,
  type SecretKey,
  type SecretProvider,
  type SettingsCategoryId,
} from "./settingsModel";
import "./SettingsPanel.css";

export default function SettingsPanel() {
  const { t } = useTranslation();
  const [controller, setController] = useState(() =>
    createSettingsSaveController(invoke, () => emit("config://updated"))
  );
  const [, refresh] = useState(0);
  const {
    config,
    saved,
    error: errorKey,
    loadFailed,
    isLoadingConfig,
    isSaving,
    secretStatus,
    secretEditMode,
    hasUnsavedChanges,
  } = controller.getSnapshot();
  const secretEdits = controller.getSecretInputs();
  const error = errorKey ? t(errorKey) : "";
  const setConfig = (update: (previous: AppConfig) => AppConfig) => controller.updateConfig(update);
  const [sshAlgorithmCatalog, setSshAlgorithmCatalog] = useState<SshAlgorithmCatalog | null>(null);
  const [sshAlgorithmCatalogLoadFailed, setSshAlgorithmCatalogLoadFailed] = useState(false);
  const [activeCategory, setActiveCategory] = useState<SettingsCategoryId>("general");
  const [expandedOtherProvider, setExpandedOtherProvider] = useState<AiProviderId | null>(null);
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

  useEffect(() => {
    // A fresh owner also supports React StrictMode's setup/cleanup replay.
    const owner = createSettingsSaveController(invoke, () => emit("config://updated"));
    setController(owner);
    const unsubscribe = owner.subscribe(() => refresh((value) => value + 1));
    void owner.load();
    void owner.loadSecretStatus();
    void loadSshAlgorithmCatalog();
    return () => {
      unsubscribe();
      owner.dispose();
    };
  }, []);

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

  const clearSecret = async (_provider: SecretProvider, key: SecretKey) => {
    const confirmed = await confirm(t("settings.secret_clear_confirm_message"), {
      title: t("settings.secret_clear_confirm_title"),
      kind: "warning",
      okLabel: t("settings.clear"),
      cancelLabel: t("settings.cancel"),
    });
    if (!confirmed) return;

    await controller.clearSecret(key);
  };

  if (!config) {
    return (
      <div className="settings-panel">
        {isLoadingConfig && <p>{t("settings.loading")}</p>}
        {loadFailed && (
          <>
            <SettingsError message={t("settings.load_failed")} />
            <button className="btn btn-primary" onClick={() => void controller.load()}>
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
              controller.editSecret(key, value);
            }}
            onBeginSecretEdit={(key) => {
              controller.beginSecretEdit(key);
            }}
            onCancelSecretEdit={(key) => {
              controller.cancelSecretEdit(key);
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
        onSave={() => void controller.save()}
        onRevert={() => controller.revert()}
      />
    </div>
  );
}
