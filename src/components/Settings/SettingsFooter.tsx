import { Check, CircleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";

interface SettingsFooterProps {
  hasUnsavedChanges: boolean;
  saved: boolean;
  error: string;
  isSaving: boolean;
  onSave: () => void;
  onRevert: () => void;
}

export function SettingsError({ message }: { message: string }) {
  return (
    <span className="settings-error" role="alert">
      <CircleAlert size={14} aria-hidden="true" />
      <span>{message}</span>
    </span>
  );
}

export function SettingsFooter({
  hasUnsavedChanges,
  saved,
  error,
  isSaving,
  onSave,
  onRevert,
}: SettingsFooterProps) {
  const { t } = useTranslation();

  return (
    <div className="settings-actions">
      <div className="settings-actions__status">
        {hasUnsavedChanges && (
          <span className="settings-unsaved">{t("settings.unsaved_changes")}</span>
        )}
        {saved && (
          <span className="settings-saved">
            <Check size={14} /> {t("settings.saved")}
          </span>
        )}
        {error && <SettingsError message={error} />}
      </div>
      <div className="settings-actions__buttons">
        {hasUnsavedChanges && (
          <button type="button" className="btn btn-ghost" onClick={onRevert} disabled={isSaving}>
            {t("settings.revert")}
          </button>
        )}
        <button
          type="button"
          className="btn btn-primary"
          onClick={onSave}
          disabled={isSaving || !hasUnsavedChanges}
        >
          {isSaving
            ? t("settings.saving")
            : hasUnsavedChanges
              ? t("settings.save_changes")
              : t("settings.save")}
        </button>
      </div>
    </div>
  );
}
