import { FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  normalizeSshAuthMethod,
  SSH_KEY_PATH_PLACEHOLDER,
  SSH_AUTH_METHODS,
} from "./connectionProfileUtils";
import type { SshFormActions, SshFormState, SshProfileOptions } from "./connectionDialogTypes";
import {
  EncodingSelect,
  ProfileMetadataFields,
  ProfileNameField,
  ProfileSelector,
  TerminalModeSelect,
} from "./ConnectionFormFields";

interface SshConnectionFormProps {
  formState: SshFormState;
  formActions: SshFormActions;
  profileOptions: SshProfileOptions;
}

export function SshConnectionForm({
  formState,
  formActions,
  profileOptions,
}: SshConnectionFormProps) {
  const { t } = useTranslation();

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
          />
        </div>
      </div>
      <div>
        <label className="label">{t("connection.username")}</label>
        <input
          className="input"
          value={formState.username}
          onChange={(event) => {
            formActions.onUsernameChange(event.target.value);
          }}
        />
      </div>
      <div>
        <label className="label">{t("connection.auth_method")}</label>
        <select
          className="select"
          style={{ width: "100%" }}
          value={formState.authMethod}
          onChange={(event) => {
            formActions.onAuthMethodChange(normalizeSshAuthMethod(event.target.value));
          }}
        >
          {SSH_AUTH_METHODS.map((entry) => (
            <option key={entry.value} value={entry.value}>
              {t(entry.labelKey)}
            </option>
          ))}
        </select>
      </div>
      {formState.authMethod === "public_key" && (
        <div>
          <label className="label">{t("connection.private_key_path")}</label>
          <div className="connection-dialog__file-row">
            <input
              className="input"
              value={formState.privateKeyPath}
              onChange={(event) => {
                formActions.onPrivateKeyPathChange(event.target.value);
              }}
              placeholder={SSH_KEY_PATH_PLACEHOLDER}
            />
            <button
              className="btn btn-ghost btn-sm"
              type="button"
              onClick={formActions.onSelectPrivateKeyFile}
              title={t("connection.select_file")}
            >
              <FolderOpen size={14} />
              {t("connection.select_file")}
            </button>
          </div>
        </div>
      )}
      <div>
        <label className="label">{t("connection.jump_profile")}</label>
        <select
          className="select"
          style={{ width: "100%" }}
          value={formState.jumpProfileId}
          onChange={(event) => {
            formActions.onJumpProfileChange(event.target.value);
          }}
        >
          <option value="">{t("connection.jump_profile_none")}</option>
          {profileOptions.jumpProfiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profileOptions.getDisplayName(profile)}
            </option>
          ))}
        </select>
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
