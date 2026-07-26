import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ShortcutConfig } from "../../types";
import {
  DEFAULT_SHORTCUT_CONFIG,
  SHORTCUT_ACTIONS,
  captureShortcut,
  createDefaultShortcutConfig,
  findShortcutConflict,
  formatShortcut,
  shortcutBindingsEqual,
  type ShortcutAction,
} from "../../features/shortcuts/shortcutModel";

interface ShortcutsSettingsProps {
  config: ShortcutConfig;
  onChange: (config: ShortcutConfig) => void;
}

export function ShortcutsSettings({ config, onChange }: ShortcutsSettingsProps) {
  const { t } = useTranslation();
  const [recordingAction, setRecordingAction] = useState<ShortcutAction | null>(null);
  const [errors, setErrors] = useState<Partial<Record<ShortcutAction, string>>>({});

  const clearError = (action: ShortcutAction) => {
    setErrors((previous) => ({ ...previous, [action]: undefined }));
  };

  const updateShortcut = (action: ShortcutAction, binding: ShortcutConfig[ShortcutAction]) => {
    onChange({ ...config, [action]: binding });
    clearError(action);
    setRecordingAction(null);
  };

  return (
    <div className="settings-section settings-shortcuts">
      <div className="settings-shortcuts__header">
        <div>
          <div className="settings-section__title">{t("settings.shortcuts.title")}</div>
          <p className="settings-help">{t("settings.shortcuts.help")}</p>
        </div>
        <button
          type="button"
          className="btn btn-ghost btn-sm"
          onClick={() => {
            onChange(createDefaultShortcutConfig());
            setErrors({});
            setRecordingAction(null);
          }}
        >
          {t("settings.shortcuts.reset_all")}
        </button>
      </div>

      <div className="settings-shortcuts__list">
        {SHORTCUT_ACTIONS.map(({ id, labelKey }) => {
          const binding = config[id];
          const isRecording = recordingAction === id;
          return (
            <div className="settings-shortcuts__item" key={id}>
              <div className="settings-shortcuts__description">
                <span>{t(labelKey)}</span>
                {errors[id] && (
                  <small className="settings-shortcuts__error" role="alert">
                    {errors[id]}
                  </small>
                )}
              </div>
              <button
                type="button"
                className={`settings-shortcuts__recorder ${
                  isRecording ? "settings-shortcuts__recorder--active" : ""
                }`}
                data-shortcut-recorder="true"
                aria-pressed={isRecording}
                aria-label={t("settings.shortcuts.record_aria", { action: t(labelKey) })}
                onClick={() => {
                  clearError(id);
                  setRecordingAction(id);
                }}
                onBlur={() => {
                  setRecordingAction((current) => (current === id ? null : current));
                }}
                onKeyDown={(event) => {
                  if (!isRecording) return;
                  event.preventDefault();
                  event.stopPropagation();
                  if (["Control", "Alt", "Shift"].includes(event.key)) return;

                  const result = captureShortcut(event.nativeEvent);
                  if (result.kind === "cancel") {
                    clearError(id);
                    setRecordingAction(null);
                    return;
                  }
                  if (result.kind === "clear") {
                    updateShortcut(id, null);
                    return;
                  }
                  if (result.kind === "invalid") {
                    setErrors((previous) => ({
                      ...previous,
                      [id]: t("settings.shortcuts.invalid"),
                    }));
                    return;
                  }

                  const conflict = findShortcutConflict(config, id, result.binding);
                  if (conflict) {
                    const conflictDefinition = SHORTCUT_ACTIONS.find(
                      ({ id: action }) => action === conflict
                    );
                    setErrors((previous) => ({
                      ...previous,
                      [id]: t("settings.shortcuts.conflict", {
                        action: conflictDefinition ? t(conflictDefinition.labelKey) : conflict,
                      }),
                    }));
                    return;
                  }
                  updateShortcut(id, result.binding);
                }}
              >
                {isRecording
                  ? t("settings.shortcuts.press_keys")
                  : formatShortcut(binding) || t("settings.shortcuts.unassigned")}
              </button>
              <div className="settings-shortcuts__actions">
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  disabled={!binding}
                  onClick={() => {
                    updateShortcut(id, null);
                  }}
                >
                  {t("settings.shortcuts.clear")}
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  disabled={shortcutBindingsEqual(binding, DEFAULT_SHORTCUT_CONFIG[id])}
                  onClick={() => {
                    const defaultBinding = DEFAULT_SHORTCUT_CONFIG[id];
                    updateShortcut(id, defaultBinding ? { ...defaultBinding } : null);
                  }}
                >
                  {t("settings.shortcuts.reset")}
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
