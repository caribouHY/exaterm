import { useTranslation } from "react-i18next";
import type { SecretFieldDefinition } from "./settingsModel";

interface SecretFieldProps {
  field: SecretFieldDefinition;
  hasSecret: boolean;
  isEditing: boolean;
  value: string;
  onValueChange: (value: string) => void;
  onBeginEdit: () => void;
  onCancelEdit: () => void;
  onClear: () => void;
}

export function SecretField({
  field,
  hasSecret,
  isEditing,
  value,
  onValueChange,
  onBeginEdit,
  onCancelEdit,
  onClear,
}: SecretFieldProps) {
  const { t } = useTranslation();
  const inputId = `settings-${field.key}-api-key`;

  return (
    <div className="settings-provider-detail__secret">
      <div className="settings-provider-detail__summary">
        <label className="label" htmlFor={inputId}>
          {t(field.labelKey)}
        </label>
        <span
          className={`settings-provider-status ${
            hasSecret
              ? "settings-provider-status--configured"
              : "settings-provider-status--unconfigured"
          }`}
        >
          {t(hasSecret ? "settings.configured" : "settings.not_configured")}
        </span>
      </div>
      <div className="settings-secret-row">
        {isEditing && (
          <input
            className="input"
            id={inputId}
            type="password"
            value={value}
            onChange={(event) => {
              onValueChange(event.target.value);
            }}
            placeholder={field.placeholder}
          />
        )}
        {!isEditing && (
          <button type="button" className="btn btn-ghost btn-sm" onClick={onBeginEdit}>
            {t(hasSecret ? "settings.change" : "settings.configure")}
          </button>
        )}
        {hasSecret && !isEditing && (
          <button type="button" className="btn btn-danger btn-sm" onClick={onClear}>
            {t("settings.clear")}
          </button>
        )}
        {isEditing && (
          <button type="button" className="btn btn-ghost btn-sm" onClick={onCancelEdit}>
            {t("settings.cancel")}
          </button>
        )}
      </div>
    </div>
  );
}
