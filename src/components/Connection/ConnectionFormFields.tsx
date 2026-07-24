import { useTranslation } from "react-i18next";
import type { Encoding, SavedConnection, TerminalMode } from "../../types";
import { normalizeTerminalMode, TERMINAL_MODE_OPTIONS } from "../../utils/terminalModes";
import { normalizeEncoding, SSH_ENCODINGS } from "./connectionProfileUtils";

interface ProfileSelectorProps {
  selectedProfileId: string;
  profiles: SavedConnection[];
  getDisplayName: (profile: SavedConnection) => string;
  onSelectProfile: (id: string) => void;
  onDeleteProfile: () => void;
}

export function ProfileSelector({
  selectedProfileId,
  profiles,
  getDisplayName,
  onSelectProfile,
  onDeleteProfile,
}: ProfileSelectorProps) {
  const { t } = useTranslation();

  return (
    <div className="connection-dialog__profile">
      <label className="label">{t("connection.profile")}</label>
      <div className="connection-dialog__profile-row">
        <select
          className="select"
          value={selectedProfileId}
          onChange={(event) => {
            onSelectProfile(event.target.value);
          }}
        >
          <option value="">{t("connection.profile_manual")}</option>
          {profiles.map((profile) => (
            <option key={profile.id} value={profile.id}>
              {getDisplayName(profile)}
            </option>
          ))}
        </select>
        {selectedProfileId && (
          <button className="btn btn-danger btn-sm" onClick={onDeleteProfile}>
            {t("connection.profile_delete")}
          </button>
        )}
      </div>
    </div>
  );
}

interface ProfileNameFieldProps {
  value: string;
  onChange: (value: string) => void;
}

export function ProfileNameField({ value, onChange }: ProfileNameFieldProps) {
  const { t } = useTranslation();

  return (
    <div>
      <label className="label">{t("connection.profile_name")}</label>
      <input
        className="input"
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        placeholder={t("connection.profile_name_placeholder")}
      />
    </div>
  );
}

interface EncodingSelectProps {
  value: Encoding;
  onChange: (value: Encoding) => void;
}

export function EncodingSelect({ value, onChange }: EncodingSelectProps) {
  const { t } = useTranslation();

  return (
    <div>
      <label className="label">{t("connection.encoding")}</label>
      <select
        className="select"
        style={{ width: "100%" }}
        value={value}
        onChange={(event) => {
          onChange(normalizeEncoding(event.target.value));
        }}
      >
        {SSH_ENCODINGS.map((entry) => (
          <option key={entry.value} value={entry.value}>
            {entry.label}
          </option>
        ))}
      </select>
    </div>
  );
}

interface TerminalModeSelectProps {
  value: TerminalMode;
  onChange: (value: TerminalMode) => void;
}

export function TerminalModeSelect({ value, onChange }: TerminalModeSelectProps) {
  const { t } = useTranslation();

  return (
    <div>
      <label className="label">{t("connection.terminal_mode")}</label>
      <select
        className="select"
        style={{ width: "100%" }}
        value={value}
        onChange={(event) => {
          onChange(normalizeTerminalMode(event.target.value));
        }}
      >
        {TERMINAL_MODE_OPTIONS.map((entry) => (
          <option key={entry.value} value={entry.value}>
            {t(entry.labelKey)}
          </option>
        ))}
      </select>
    </div>
  );
}

interface ProfileMetadataFieldsProps {
  memo: string;
  externalControlEnabled: boolean;
  selectedProfileId: string;
  onMemoChange: (value: string) => void;
  onExternalControlEnabledChange: (value: boolean) => void;
  onSaveProfile: () => void;
}

export function ProfileMetadataFields({
  memo,
  externalControlEnabled,
  selectedProfileId,
  onMemoChange,
  onExternalControlEnabledChange,
  onSaveProfile,
}: ProfileMetadataFieldsProps) {
  const { t } = useTranslation();

  return (
    <>
      <div>
        <label className="label">{t("connection.profile_memo")}</label>
        <textarea
          className="input connection-dialog__memo"
          value={memo}
          onChange={(event) => {
            onMemoChange(event.target.value);
          }}
          placeholder={t("connection.profile_memo_placeholder")}
        />
        <div className="connection-dialog__field-help">
          {t("connection.profile_memo_mcp_notice")}
        </div>
      </div>
      <label className="connection-dialog__checkbox">
        <input
          type="checkbox"
          checked={externalControlEnabled}
          onChange={(event) => {
            onExternalControlEnabledChange(event.target.checked);
          }}
        />
        <span>{t("connection.profile_mcp_enabled")}</span>
      </label>
      <div className="connection-dialog__field-help">
        {t("connection.profile_mcp_enabled_help")}
      </div>
      <div className="connection-dialog__profile-actions">
        <button className="btn btn-ghost btn-sm" onClick={onSaveProfile}>
          {selectedProfileId ? t("connection.profile_update") : t("connection.profile_save")}
        </button>
        <span>{t("connection.profile_password_notice")}</span>
      </div>
    </>
  );
}
