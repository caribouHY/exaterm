import { useTranslation } from "react-i18next";
import type {
  TelnetFormActions,
  TelnetFormState,
  TelnetProfileOptions,
} from "./connectionDialogTypes";
import {
  EncodingSelect,
  ProfileMetadataFields,
  ProfileNameField,
  ProfileSelector,
  TerminalModeSelect,
} from "./ConnectionFormFields";

interface TelnetConnectionFormProps {
  formState: TelnetFormState;
  formActions: TelnetFormActions;
  profileOptions: TelnetProfileOptions;
}

export function TelnetConnectionForm({
  formState,
  formActions,
  profileOptions,
}: TelnetConnectionFormProps) {
  const { t } = useTranslation();

  return (
    <>
      <ProfileSelector
        selectedProfileId={formState.selectedProfileId}
        profiles={profileOptions.profiles}
        getDisplayName={profileOptions.getDisplayName}
        onSelectProfile={formActions.onSelectProfile}
        onDeleteProfile={formActions.onDeleteProfile}
      />
      <ProfileNameField value={formState.profileName} onChange={formActions.onProfileNameChange} />
      <div className="connection-dialog__row">
        <div>
          <label className="label">{t("connection.host")}</label>
          <input
            className="input"
            value={formState.host}
            onChange={(event) => {
              formActions.onHostChange(event.target.value);
            }}
            placeholder="192.168.1.1"
          />
        </div>
        <div style={{ maxWidth: 100 }}>
          <label className="label">{t("connection.port")}</label>
          <input
            className="input"
            type="number"
            value={formState.port}
            onChange={(event) => {
              formActions.onPortChange(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                formActions.onPortEnter();
              }
            }}
          />
        </div>
      </div>
      <EncodingSelect value={formState.encoding} onChange={formActions.onEncodingChange} />
      <TerminalModeSelect
        value={formState.terminalMode}
        onChange={formActions.onTerminalModeChange}
      />
      <ProfileMetadataFields
        memo={formState.memo}
        externalControlEnabled={formState.externalControlEnabled}
        selectedProfileId={formState.selectedProfileId}
        onMemoChange={formActions.onMemoChange}
        onExternalControlEnabledChange={formActions.onExternalControlEnabledChange}
        onSaveProfile={formActions.onSaveProfile}
      />
    </>
  );
}
