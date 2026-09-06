import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { AppConfig } from "../../types";
import { finalizeBoundedInteger, parseBoundedInteger } from "./terminalSettingsModel";

interface TerminalSettingsProps {
  config: AppConfig["terminal"];
  onChange: (patch: Partial<AppConfig["terminal"]>) => void;
}

const FONT_SIZE_MIN = 8;
const FONT_SIZE_MAX = 32;
const SCROLLBACK_MIN = 100;
const SCROLLBACK_MAX = 100000;

export function TerminalSettings({ config, onChange }: TerminalSettingsProps) {
  const { t } = useTranslation();
  const [fontSizeInput, setFontSizeInput] = useState(() => String(config.font_size));
  const [scrollbackInput, setScrollbackInput] = useState(() => String(config.scrollback));
  const [editingField, setEditingField] = useState<"font_size" | "scrollback" | null>(null);

  useEffect(() => {
    if (editingField !== "font_size") setFontSizeInput(String(config.font_size));
    if (editingField !== "scrollback") setScrollbackInput(String(config.scrollback));
  }, [config.font_size, config.scrollback, editingField]);

  const handleNumberChange = (
    value: string,
    min: number,
    max: number,
    update: (value: number) => void
  ) => {
    const parsed = parseBoundedInteger(value, min, max);
    if (parsed !== null) update(parsed);
  };

  const finalizeNumberInput = (
    value: string,
    currentValue: number,
    min: number,
    max: number,
    update: (value: number) => void,
    setInput: (value: string) => void
  ) => {
    const finalized = finalizeBoundedInteger(value, currentValue, min, max);
    setInput(String(finalized));
    update(finalized);
  };

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
            value={fontSizeInput}
            onFocus={() => {
              setEditingField("font_size");
            }}
            onChange={(event) => {
              setFontSizeInput(event.target.value);
              handleNumberChange(event.target.value, FONT_SIZE_MIN, FONT_SIZE_MAX, (fontSize) => {
                onChange({ font_size: fontSize });
              });
            }}
            onBlur={() => {
              finalizeNumberInput(
                fontSizeInput,
                config.font_size,
                FONT_SIZE_MIN,
                FONT_SIZE_MAX,
                (fontSize) => {
                  onChange({ font_size: fontSize });
                },
                setFontSizeInput
              );
              setEditingField(null);
            }}
            min={FONT_SIZE_MIN}
            max={FONT_SIZE_MAX}
            step={1}
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
            value={scrollbackInput}
            onFocus={() => {
              setEditingField("scrollback");
            }}
            onChange={(event) => {
              setScrollbackInput(event.target.value);
              handleNumberChange(
                event.target.value,
                SCROLLBACK_MIN,
                SCROLLBACK_MAX,
                (scrollback) => {
                  onChange({ scrollback });
                }
              );
            }}
            onBlur={() => {
              finalizeNumberInput(
                scrollbackInput,
                config.scrollback,
                SCROLLBACK_MIN,
                SCROLLBACK_MAX,
                (scrollback) => {
                  onChange({ scrollback });
                },
                setScrollbackInput
              );
              setEditingField(null);
            }}
            min={SCROLLBACK_MIN}
            max={SCROLLBACK_MAX}
            step={1}
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
