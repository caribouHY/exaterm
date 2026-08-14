import { useTranslation } from "react-i18next";
import type { AppConfig } from "../../types";
import type { SshAlgorithmCatalog } from "../../types";
import { ConnectionHistorySettings } from "./ConnectionHistorySettings";
import { SshSettings } from "./SshSettings";
import { TerminalSettings } from "./TerminalSettings";
import { SettingsToggle } from "./SettingsToggle";

interface GeneralSettingsProps {
  language: AppConfig["language"];
  updateConfig: AppConfig["updates"];
  connectionHistoryConfig: AppConfig["connection_history"];
  terminalConfig: AppConfig["terminal"];
  sshConfig: AppConfig["ssh"];
  sshAlgorithmCatalog: SshAlgorithmCatalog | null;
  sshAlgorithmCatalogLoadFailed: boolean;
  onLanguageChange: (language: AppConfig["language"]) => void;
  onUpdateChange: (patch: Partial<AppConfig["updates"]>) => void;
  onConnectionHistoryChange: (patch: Partial<AppConfig["connection_history"]>) => void;
  onTerminalChange: (patch: Partial<AppConfig["terminal"]>) => void;
  onSshChange: (patch: Partial<AppConfig["ssh"]>) => void;
  onReloadSshAlgorithmCatalog: () => void;
}

const LANGUAGE_OPTIONS: Array<{
  value: AppConfig["language"];
  label?: string;
  labelKey?: "settings.language_system";
}> = [
  { value: "system", labelKey: "settings.language_system" },
  { value: "en", label: "English" },
  { value: "ja", label: "日本語" },
];

export function GeneralSettings({
  language,
  updateConfig,
  connectionHistoryConfig,
  terminalConfig,
  sshConfig,
  sshAlgorithmCatalog,
  sshAlgorithmCatalogLoadFailed,
  onLanguageChange,
  onUpdateChange,
  onConnectionHistoryChange,
  onTerminalChange,
  onSshChange,
  onReloadSshAlgorithmCatalog,
}: GeneralSettingsProps) {
  const { t } = useTranslation();

  return (
    <div className="settings-section">
      <div className="settings-section__title">{t("settings.language")}</div>
      <div className="settings-row">
        <div>
          <label className="label" htmlFor="settings-language">
            {t("settings.language")}
          </label>
          <select
            id="settings-language"
            className="select"
            value={language}
            onChange={(event) => {
              onLanguageChange(event.target.value);
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

      <div className="settings-section__title">{t("settings.updates")}</div>
      <SettingsToggle
        id="settings-check-updates-on-startup"
        label={t("settings.check_updates_on_startup")}
        description={t("settings.check_updates_on_startup_desc")}
        checked={updateConfig.check_on_startup}
        onChange={(checkOnStartup) => {
          onUpdateChange({ check_on_startup: checkOnStartup });
        }}
      />

      <ConnectionHistorySettings
        config={connectionHistoryConfig}
        onChange={onConnectionHistoryChange}
      />

      <TerminalSettings config={terminalConfig} onChange={onTerminalChange} />

      <SshSettings
        config={sshConfig}
        catalog={sshAlgorithmCatalog}
        catalogLoadFailed={sshAlgorithmCatalogLoadFailed}
        onChange={onSshChange}
        onReloadCatalog={onReloadSshAlgorithmCatalog}
      />
    </div>
  );
}
