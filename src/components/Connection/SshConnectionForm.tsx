import { useRef } from "react";
import { ConnectionSearchSelect, ConnectionSelect } from "./ConnectionSearchSelect";
import { profileSearchSections } from "./connectionSearchModel";
import { parseConnectionSource } from "./connectionHistoryModel";
import { FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  normalizeSshAuthMethod,
  SSH_KEY_PATH_PLACEHOLDER,
  SSH_AUTH_METHODS,
  usesPrivateKeyAuthentication,
} from "./connectionProfileUtils";
import type { SshFormActions, SshFormState, SshProfileOptions } from "./connectionDialogTypes";
import {
  ConnectionFieldError,
  ConnectionFieldLabelHtml,
  EncodingSelect,
  ProfileMetadataFields,
  ProfileNameField,
  ProfileGroupField,
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
  const errors = formState.validationErrors;
  const profileNameRef = useRef<HTMLInputElement>(null);

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
        onCopyProfile={() => {
          if (formActions.onCopyProfile()) profileNameRef.current?.focus();
        }}
      />
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
      <div className="connection-dialog__row">
        <div>
          <ConnectionFieldLabelHtml htmlFor="connection-ssh-host" required>
            {t("connection.host")}
          </ConnectionFieldLabelHtml>
          <input
            id="connection-ssh-host"
            className="input"
            value={formState.host}
            onChange={(event) => {
              formActions.onHostChange(event.target.value);
            }}
            placeholder="192.168.1.1"
            required
            aria-invalid={Boolean(errors.host)}
            aria-describedby={errors.host ? "connection-ssh-host-error" : undefined}
          />
          <ConnectionFieldError id="connection-ssh-host-error" error={errors.host} />
        </div>
        <div style={{ maxWidth: 100 }}>
          <ConnectionFieldLabelHtml htmlFor="connection-ssh-port" required>
            {t("connection.port")}
          </ConnectionFieldLabelHtml>
          <input
            id="connection-ssh-port"
            className="input"
            type="number"
            value={formState.port}
            onChange={(event) => {
              formActions.onPortChange(event.target.value);
            }}
            required
            min={1}
            max={65535}
            step={1}
            aria-invalid={Boolean(errors.port)}
            aria-describedby={errors.port ? "connection-ssh-port-error" : undefined}
          />
          <ConnectionFieldError id="connection-ssh-port-error" error={errors.port} />
        </div>
      </div>
      <div>
        <ConnectionFieldLabelHtml htmlFor="connection-ssh-username" required>
          {t("connection.username")}
        </ConnectionFieldLabelHtml>
        <input
          id="connection-ssh-username"
          className="input"
          value={formState.username}
          onChange={(event) => {
            formActions.onUsernameChange(event.target.value);
          }}
          required
          aria-invalid={Boolean(errors.username)}
          aria-describedby={errors.username ? "connection-ssh-username-error" : undefined}
        />
        <ConnectionFieldError id="connection-ssh-username-error" error={errors.username} />
      </div>
      <ConnectionSelect
        label={t("connection.auth_method")}
        value={formState.authMethod}
        options={SSH_AUTH_METHODS.map((entry) => ({
          value: entry.value,
          label: t(entry.labelKey),
        }))}
        onChange={(value) => formActions.onAuthMethodChange(normalizeSshAuthMethod(value))}
      />
      {usesPrivateKeyAuthentication(formState.authMethod) && (
        <div>
          <ConnectionFieldLabelHtml
            htmlFor="connection-ssh-private-key"
            required={formState.authMethod === "public_key"}
          >
            {t("connection.private_key_path")}
          </ConnectionFieldLabelHtml>
          <div className="connection-dialog__file-row">
            <input
              id="connection-ssh-private-key"
              className="input"
              value={formState.privateKeyPath}
              onChange={(event) => {
                formActions.onPrivateKeyPathChange(event.target.value);
              }}
              placeholder={SSH_KEY_PATH_PLACEHOLDER}
              required={formState.authMethod === "public_key"}
              aria-invalid={Boolean(errors.privateKeyPath)}
              aria-describedby={
                errors.privateKeyPath ? "connection-ssh-private-key-error" : undefined
              }
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
          <ConnectionFieldError
            id="connection-ssh-private-key-error"
            error={errors.privateKeyPath}
          />
          {formState.authMethod === "auto" && (
            <p className="connection-dialog__field-help">{t("connection.auto_private_key_desc")}</p>
          )}
        </div>
      )}
      <ConnectionSearchSelect
        label={t("connection.jump_profile")}
        value={formState.jumpProfileId ? `profile:${formState.jumpProfileId}` : ""}
        sections={[
          {
            key: "direct",
            options: [
              {
                value: "",
                label: t("connection.jump_profile_none"),
                searchText: "",
                alwaysVisible: true,
              },
            ],
          },
          ...profileSearchSections(
            profileOptions.jumpProfiles,
            profileOptions.getDisplayName,
            t("connection.profile_saved"),
            t("connection.profile_ungrouped")
          ),
        ]}
        onChange={(value) => {
          const source = parseConnectionSource(value);
          formActions.onJumpProfileChange(source.kind === "profile" ? source.id : "");
        }}
      />
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
