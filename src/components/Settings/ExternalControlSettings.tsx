import { useTranslation } from "react-i18next";
import type { AppConfig } from "../../types";
import { SettingsToggle } from "./SettingsToggle";

interface ExternalControlSettingsProps {
  config: AppConfig["external_control"];
  onChange: (patch: Partial<AppConfig["external_control"]>) => void;
}

export function ExternalControlSettings({ config, onChange }: ExternalControlSettingsProps) {
  const { t } = useTranslation();
  const childrenDisabled = !config.enabled;

  return (
    <div className="settings-section">
      <div className="settings-section__title">{t("settings.mcp_settings")}</div>
      <SettingsToggle
        id="settings-external-control-enabled"
        label={t("settings.mcp_enabled")}
        description={t("settings.mcp_enabled_desc")}
        checked={Boolean(config.enabled)}
        onChange={(enabled) => {
          onChange({ enabled });
        }}
      />
      {childrenDisabled && (
        <p className="settings-help">{t("settings.mcp_children_disabled_desc")}</p>
      )}
      <SettingsToggle
        id="settings-external-control-cli-enabled"
        label={t("settings.mcp_cli_enabled")}
        description={t("settings.mcp_cli_enabled_desc")}
        checked={Boolean(config.cli_enabled)}
        onChange={(cli_enabled) => {
          onChange({ cli_enabled });
        }}
        disabled={childrenDisabled}
      />
      <SettingsToggle
        id="settings-external-control-connect-enabled"
        label={t("settings.mcp_connect_enabled")}
        description={t("settings.mcp_connect_enabled_desc")}
        checked={Boolean(config.connect_enabled)}
        onChange={(connect_enabled) => {
          onChange({ connect_enabled });
        }}
        disabled={childrenDisabled}
      />
      <div className="settings-section__title" style={{ marginTop: 20 }}>
        {t("settings.mcp_adapter_title")}
      </div>
      <SettingsToggle
        id="settings-external-control-stdio-enabled"
        label={t("settings.mcp_stdio_enabled")}
        description={t("settings.mcp_stdio_enabled_desc")}
        checked={Boolean(config.mcp_enabled)}
        onChange={(mcp_enabled) => {
          onChange({ mcp_enabled });
        }}
        disabled={childrenDisabled}
      />
      <p className="settings-help">{t("settings.mcp_restart_notice")}</p>
      {t("settings.mcp_loopback_warning") && (
        <p className="settings-help">{t("settings.mcp_loopback_warning")}</p>
      )}
    </div>
  );
}
