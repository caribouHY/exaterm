import { FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { normalizeTerminalMode, TERMINAL_MODE_OPTIONS } from "../../utils/terminalModes";
import {
  normalizeEncoding,
  normalizeSshAuthMethod,
  PRIVATE_KEY_PLACEHOLDER,
  SSH_AUTH_METHODS,
  SSH_ENCODINGS,
} from "./connectionProfileUtils";
import type { SshFormActions, SshFormState, SshProfileOptions } from "./connectionDialogTypes";

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
      <div className="connection-dialog__profile">
        <label className="label">{t("connection.profile")}</label>
        <div className="connection-dialog__profile-row">
          <select
            className="select"
            value={formState.selectedProfileId}
            onChange={(e) => formActions.onSelectProfile(e.target.value)}
          >
            <option value="">{t("connection.profile_manual")}</option>
            {profileOptions.profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profileOptions.getDisplayName(profile)}
              </option>
            ))}
          </select>
          {formState.selectedProfileId && (
            <button className="btn btn-danger btn-sm" onClick={formActions.onDeleteProfile}>
              {t("connection.profile_delete")}
            </button>
          )}
        </div>
      </div>
      <div>
        <label className="label">{t("connection.profile_name")}</label>
        <input
          className="input"
          value={formState.profileName}
          onChange={(e) => formActions.onProfileNameChange(e.target.value)}
          placeholder={t("connection.profile_name_placeholder")}
        />
      </div>
      <div className="connection-dialog__row">
        <div>
          <label className="label">{t("connection.host")}</label>
          <input
            className="input"
            value={formState.host}
            onChange={(e) => formActions.onHostChange(e.target.value)}
            placeholder="192.168.1.1"
          />
        </div>
        <div style={{ maxWidth: 100 }}>
          <label className="label">{t("connection.port")}</label>
          <input
            className="input"
            type="number"
            value={formState.port}
            onChange={(e) => formActions.onPortChange(e.target.value)}
          />
        </div>
      </div>
      <div>
        <label className="label">{t("connection.username")}</label>
        <input
          className="input"
          value={formState.username}
          onChange={(e) => formActions.onUsernameChange(e.target.value)}
        />
      </div>
      <div>
        <label className="label">{t("connection.auth_method")}</label>
        <select
          className="select"
          style={{ width: "100%" }}
          value={formState.authMethod}
          onChange={(e) => formActions.onAuthMethodChange(normalizeSshAuthMethod(e.target.value))}
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
              onChange={(e) => formActions.onPrivateKeyPathChange(e.target.value)}
              placeholder={PRIVATE_KEY_PLACEHOLDER}
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
          onChange={(e) => formActions.onJumpProfileChange(e.target.value)}
        >
          <option value="">{t("connection.jump_profile_none")}</option>
          {profileOptions.jumpProfiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {profileOptions.getDisplayName(profile)}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className="label">{t("connection.encoding")}</label>
        <select
          className="select"
          style={{ width: "100%" }}
          value={formState.encoding}
          onChange={(e) => formActions.onEncodingChange(normalizeEncoding(e.target.value))}
        >
          {SSH_ENCODINGS.map((entry) => (
            <option key={entry.value} value={entry.value}>
              {entry.label}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className="label">{t("connection.terminal_mode")}</label>
        <select
          className="select"
          style={{ width: "100%" }}
          value={formState.terminalMode}
          onChange={(e) => formActions.onTerminalModeChange(normalizeTerminalMode(e.target.value))}
        >
          {TERMINAL_MODE_OPTIONS.map((entry) => (
            <option key={entry.value} value={entry.value}>
              {t(entry.labelKey)}
            </option>
          ))}
        </select>
      </div>
      <div>
        <label className="label">{t("connection.profile_memo")}</label>
        <textarea
          className="input connection-dialog__memo"
          value={formState.memo}
          onChange={(e) => formActions.onMemoChange(e.target.value)}
          placeholder={t("connection.profile_memo_placeholder")}
        />
        <div className="connection-dialog__field-help">
          {t("connection.profile_memo_mcp_notice")}
        </div>
      </div>
      <label className="connection-dialog__checkbox">
        <input
          type="checkbox"
          checked={formState.externalControlEnabled}
          onChange={(e) => formActions.onExternalControlEnabledChange(e.target.checked)}
        />
        <span>{t("connection.profile_mcp_enabled")}</span>
      </label>
      <div className="connection-dialog__field-help">
        {t("connection.profile_mcp_enabled_help")}
      </div>
      <div className="connection-dialog__profile-actions">
        <button className="btn btn-ghost btn-sm" onClick={formActions.onSaveProfile}>
          {formState.selectedProfileId
            ? t("connection.profile_update")
            : t("connection.profile_save")}
        </button>
        <span>{t("connection.profile_password_notice")}</span>
      </div>
    </>
  );
}
