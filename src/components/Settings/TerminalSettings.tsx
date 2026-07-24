import { useTranslation } from "react-i18next";
import type { AppConfig } from "../../types";

interface TerminalSettingsProps {
  config: AppConfig["terminal"];
  onChange: (patch: Partial<AppConfig["terminal"]>) => void;
}

const FONT_SIZE_MIN = 8;
const FONT_SIZE_MAX = 32;
const SCROLLBACK_MIN = 100;
const SCROLLBACK_MAX = 100000;

function parseBoundedNumber(value: string, currentValue: number, min: number, max: number): number {
  if (!value.trim()) return currentValue;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return currentValue;
  return Math.min(Math.max(parsed, min), max);
}

export function TerminalSettings({ config, onChange }: TerminalSettingsProps) {
  const { t } = useTranslation();

  return (
    <>
      <div className="settings-section__title">{t("settings.terminal_settings")}</div>
      <div className="settings-row">
        <div>
          <label className="label" htmlFor="settings-font-size">
            {t("settings.font_size")}
          </label>
          <input
            className="input"
            id="settings-font-size"
            type="number"
            value={config.font_size}
            onChange={(event) => {
              onChange({
                font_size: parseBoundedNumber(
                  event.target.value,
                  config.font_size,
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
          <label className="label" htmlFor="settings-scrollback">
            {t("settings.scrollback")}
          </label>
          <input
            className="input"
            id="settings-scrollback"
            type="number"
            value={config.scrollback}
            onChange={(event) => {
              onChange({
                scrollback: parseBoundedNumber(
                  event.target.value,
                  config.scrollback,
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
        <label className="label" htmlFor="settings-font-family">
          {t("settings.font_family")}
        </label>
        <input
          className="input"
          id="settings-font-family"
          value={config.font_family}
          onChange={(event) => {
            onChange({ font_family: event.target.value });
          }}
        />
      </div>
    </>
  );
}
