import { useTranslation } from "react-i18next";
import type { AppConfig } from "../../types";
import { SettingsToggle } from "./SettingsToggle";
import { TerminalSettings } from "./TerminalSettings";

interface GeneralSettingsProps {
  language: AppConfig["language"];
  terminalConfig: AppConfig["terminal"];
  sshConfig: AppConfig["ssh"];
  onLanguageChange: (language: AppConfig["language"]) => void;
  onTerminalChange: (patch: Partial<AppConfig["terminal"]>) => void;
  onSshChange: (patch: Partial<AppConfig["ssh"]>) => void;
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
  terminalConfig,
  sshConfig,
  onLanguageChange,
  onTerminalChange,
  onSshChange,
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

      <TerminalSettings config={terminalConfig} onChange={onTerminalChange} />

      <div className="settings-section__title">{t("settings.ssh_settings")}</div>
      <SettingsToggle
        id="settings-allow-legacy-ssh-algorithms"
        label={t("settings.allow_legacy_ssh_algorithms")}
        description={t("settings.allow_legacy_ssh_algorithms_desc")}
        checked={Boolean(sshConfig.allow_legacy_algorithms)}
        onChange={(allow_legacy_algorithms) => {
          onSshChange({ allow_legacy_algorithms });
        }}
      />
    </div>
  );
}
