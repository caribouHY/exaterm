import { ChevronRight } from "lucide-react";
import { useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import {
  ProfileGroupField,
  ProfileMetadataFields,
  ProfileNameField,
  ProfileSelector,
} from "./ConnectionFormFields";
import type {
  NetworkFormActions,
  NetworkFormState,
  NetworkProfileOptions,
} from "./connectionDialogTypes";

interface NetworkConnectionFormProps {
  formState: NetworkFormState;
  formActions: NetworkFormActions;
  profileOptions: NetworkProfileOptions;
  children: ReactNode;
  advancedFields: ReactNode;
  advancedSummary?: string;
}

export function NetworkConnectionForm({
  formState,
  formActions,
  profileOptions,
  children,
  advancedFields,
  advancedSummary,
}: NetworkConnectionFormProps) {
  const { t } = useTranslation();
  const profileNameRef = useRef<HTMLInputElement>(null);
  const profileDetailsRef = useRef<HTMLDetailsElement>(null);
  const saveRequest = useRef(0);
  const [saveError, setSaveError] = useState("");

  const clearSaveFeedback = () => {
    saveRequest.current += 1;
    setSaveError("");
  };

  const saveProfile = () => {
    const request = ++saveRequest.current;
    formActions.onSaveProfile((error) => {
      const details = profileDetailsRef.current;
      if (!details || request !== saveRequest.current) return;
      if (error) details.open = true;
      setSaveError(error);
    });
  };

  return (
    <>
      <ProfileSelector
        selectedProfileId={formState.selectedProfileId}
        selectedHistoryId={formState.selectedHistoryId}
        profiles={profileOptions.profiles}
        historyEntries={profileOptions.historyEntries}
        getDisplayName={profileOptions.getDisplayName}
        getHistoryDisplayName={profileOptions.getHistoryDisplayName}
        onSelectSource={(source) => {
          clearSaveFeedback();
          formActions.onSelectSource(source);
        }}
        onDeleteProfile={formActions.onDeleteProfile}
        onDeleteHistory={formActions.onDeleteHistory}
        onCopyProfile={() => {
          if (!formActions.onCopyProfile()) return;
          clearSaveFeedback();
          if (profileDetailsRef.current) profileDetailsRef.current.open = true;
          profileNameRef.current?.focus();
        }}
      />
      {children}
      <div className="connection-dialog__network-options">
        <details className="connection-dialog__disclosure">
          <summary className="connection-dialog__disclosure-summary">
            <ChevronRight size={14} aria-hidden="true" />
            {t("connection.advanced_settings")}
            {advancedSummary && (
              <span className="connection-dialog__disclosure-value">{advancedSummary}</span>
            )}
          </summary>
          <div className="connection-dialog__disclosure-fields">{advancedFields}</div>
        </details>
        <details className="connection-dialog__disclosure" ref={profileDetailsRef}>
          <summary className="connection-dialog__disclosure-summary">
            <ChevronRight size={14} aria-hidden="true" />
            {t(
              formState.selectedProfileId
                ? "connection.edit_profile_details"
                : "connection.save_profile_details"
            )}
          </summary>
          <div className="connection-dialog__disclosure-fields">
            <ProfileNameField
              value={formState.profileName}
              onChange={formActions.onProfileNameChange}
              inputRef={profileNameRef}
            />
            <ProfileGroupField
              value={formState.group}
              groupNames={profileOptions.groupNames}
              onChange={formActions.onGroupChange}
            />
            <ProfileMetadataFields
              memo={formState.memo}
              externalControlEnabled={formState.externalControlEnabled}
              selectedProfileId={formState.selectedProfileId}
              onMemoChange={formActions.onMemoChange}
              onExternalControlEnabledChange={formActions.onExternalControlEnabledChange}
              onSaveProfile={saveProfile}
            />
            {saveError && (
              <div role="alert" className="connection-dialog__field-error">
                {saveError}
              </div>
            )}
          </div>
        </details>
      </div>
    </>
  );
}
