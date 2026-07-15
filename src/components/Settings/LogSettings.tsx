import { useTranslation } from "react-i18next";
import type { AppConfig, LogFormat } from "../../types";
import { SettingsToggle } from "./SettingsToggle";

interface LogSettingsProps {
  config: AppConfig["terminal"];
  onChange: (patch: Partial<AppConfig["terminal"]>) => void;
}

const LOG_FORMAT_OPTIONS: LogFormat[] = ["display", "strip_controls"];

function isLogFormat(value: string): value is LogFormat {
  return LOG_FORMAT_OPTIONS.includes(value as LogFormat);
}

export function LogSettings({ config, onChange }: LogSettingsProps) {
  const { t } = useTranslation();

  return (
    <div className="settings-section">
      <div className="settings-section__title">{t("settings.log_settings")}</div>
      <SettingsToggle
        id="settings-auto-session-log"
        label={t("settings.auto_session_log")}
        description={t("settings.auto_session_log_desc")}
        checked={config.auto_session_log}
        onChange={(auto_session_log) => {
          onChange({ auto_session_log });
        }}
      />
      <div style={{ marginBottom: 14 }}>
        <label className="label" htmlFor="settings-log-format">
          {t("settings.log_format")}
        </label>
        <select
          id="settings-log-format"
          className="select"
          value={config.log_format}
          onChange={(event) => {
            if (isLogFormat(event.target.value)) onChange({ log_format: event.target.value });
          }}
        >
          <option value="display">{t("settings.log_format_display")}</option>
          <option value="strip_controls">{t("settings.log_format_strip_controls")}</option>
        </select>
      </div>
      <SettingsToggle
        id="settings-include-log-header"
        label={t("settings.include_log_header")}
        description={t("settings.include_log_header_desc")}
        checked={config.include_log_header}
        onChange={(include_log_header) => {
          onChange({ include_log_header });
        }}
      />
    </div>
  );
}
