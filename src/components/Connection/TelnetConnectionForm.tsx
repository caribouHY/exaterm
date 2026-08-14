import { useTranslation } from "react-i18next";
import type {
  TelnetFormActions,
  TelnetFormState,
  TelnetProfileOptions,
} from "./connectionDialogTypes";
import {
  ConnectionFieldError,
  ConnectionFieldLabel,
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
  const errors = formState.validationErrors;

  return (
    <>
      <ProfileSelector
        selectedProfileId={formState.selectedProfileId}
        selectedHistoryId={formState.selectedHistoryId}
        profiles={profileOptions.profiles}
        historyEntries={profileOptions.historyEntries}
        getDisplayName={profileOptions.getDisplayName}
        getHistoryDisplayName={profileOptions.getHistoryDisplayName}
        onSelectSource={formActions.onSelectSource}
        onDeleteProfile={formActions.onDeleteProfile}
        onDeleteHistory={formActions.onDeleteHistory}
      />
      <ProfileNameField value={formState.profileName} onChange={formActions.onProfileNameChange} />
      <div className="connection-dialog__row">
        <div>
          <ConnectionFieldLabel htmlFor="connection-telnet-host" required>
            {t("connection.host")}
          </ConnectionFieldLabel>
          <input
            id="connection-telnet-host"
            className="input"
            value={formState.host}
            onChange={(event) => {
              formActions.onHostChange(event.target.value);
            }}
            placeholder="192.168.1.1"
            required
            aria-invalid={Boolean(errors.host)}
            aria-describedby={errors.host ? "connection-telnet-host-error" : undefined}
          />
          <ConnectionFieldError id="connection-telnet-host-error" error={errors.host} />
        </div>
        <div style={{ maxWidth: 100 }}>
          <ConnectionFieldLabel htmlFor="connection-telnet-port" required>
            {t("connection.port")}
          </ConnectionFieldLabel>
          <input
            id="connection-telnet-port"
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
            required
            min={1}
            max={65535}
            step={1}
            aria-invalid={Boolean(errors.port)}
            aria-describedby={errors.port ? "connection-telnet-port-error" : undefined}
          />
          <ConnectionFieldError id="connection-telnet-port-error" error={errors.port} />
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
