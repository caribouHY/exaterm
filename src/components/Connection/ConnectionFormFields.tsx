import { useTranslation } from "react-i18next";
import { useId, type ReactNode, type Ref } from "react";
import type { ConnectionHistoryEntry, Encoding, SavedConnection, TerminalMode } from "../../types";
import { getTerminalModeOptions, normalizeTerminalMode } from "../../utils/terminalModes";
import { normalizeEncoding, SSH_ENCODINGS } from "./connectionProfileUtils";
import { encodeConnectionSource } from "./connectionHistoryModel";
import { ConnectionSearchSelect, ConnectionSelect } from "./ConnectionSearchSelect";
import { historySearchSection, profileSearchSections } from "./connectionSearchModel";
import type { ConnectionValidationError } from "./connectionFormValidation";

interface ConnectionFieldLabelHtmlProps {
  children: ReactNode;
  htmlFor: string;
  required?: boolean;
}

export function ConnectionFieldLabelHtml({
  children,
  htmlFor,
  required = false,
}: ConnectionFieldLabelHtmlProps) {
  return (
    <label className="label" htmlFor={htmlFor}>
      {children}
      {required && (
        <span className="connection-dialog__required-mark" aria-hidden="true">
          *
        </span>
      )}
    </label>
  );
}

interface ConnectionFieldErrorProps {
  error?: ConnectionValidationError;
  id: string;
}

export function ConnectionFieldError({ error, id }: ConnectionFieldErrorProps) {
  const { t } = useTranslation();
  if (!error) return null;

  return (
    <p className="connection-dialog__field-error" id={id}>
      {t(`connection.validation_${error}`)}
    </p>
  );
}

interface ProfileSelectorProps {
  selectedProfileId: string;
  selectedHistoryId: string;
  profiles: SavedConnection[];
  historyEntries: ConnectionHistoryEntry[];
  getDisplayName: (profile: SavedConnection) => string;
  getHistoryDisplayName: (entry: ConnectionHistoryEntry) => string;
  onSelectSource: (value: string) => void;
  onDeleteProfile: () => void;
  onDeleteHistory: () => void;
  onCopyProfile: () => void;
}

export function ProfileSelector({
  selectedProfileId,
  selectedHistoryId,
  profiles,
  historyEntries,
  getDisplayName,
  getHistoryDisplayName,
  onSelectSource,
  onDeleteProfile,
  onDeleteHistory,
  onCopyProfile,
}: ProfileSelectorProps) {
  const { t } = useTranslation();

  return (
    <div className="connection-dialog__profile">
      <div className="connection-dialog__profile-row">
        <ConnectionSearchSelect
          label={t("connection.source")}
          value={encodeConnectionSource(selectedProfileId, selectedHistoryId)}
          sections={[
            {
              key: "manual",
              options: [
                {
                  value: "",
                  label: t("connection.profile_manual"),
                  searchText: "",
                  alwaysVisible: true,
                },
              ],
            },
            historySearchSection(
              historyEntries,
              getHistoryDisplayName,
              t("connection.history_recent")
            ),
            ...profileSearchSections(
              profiles,
              getDisplayName,
              t("connection.profile_saved"),
              t("connection.profile_ungrouped")
            ),
          ]}
          onChange={onSelectSource}
        />
        {selectedHistoryId ? (
          <button className="btn btn-danger btn-sm" onClick={onDeleteHistory}>
            {t("connection.history_delete")}
          </button>
        ) : selectedProfileId ? (
          <div className="connection-dialog__profile-buttons">
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={onCopyProfile}
              title={t("connection.profile_copy_hint")}
              disabled={!profiles.some((profile) => profile.id === selectedProfileId)}
            >
              {t("connection.profile_copy")}
            </button>
            <button type="button" className="btn btn-danger btn-sm" onClick={onDeleteProfile}>
              {t("connection.profile_delete")}
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}

interface ProfileNameFieldProps {
  value: string;
  onChange: (value: string) => void;
  inputRef?: Ref<HTMLInputElement>;
}

export { ProfileGroupField } from "./ProfileGroupField";

export function ProfileNameField({ value, onChange, inputRef }: ProfileNameFieldProps) {
  const { t } = useTranslation();
  const id = useId();

  return (
    <div>
      <label className="label" htmlFor={id}>
        {t("connection.profile_name")}
      </label>
      <input
        id={id}
        ref={inputRef}
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
    <ConnectionSelect
      label={t("connection.encoding")}
      value={value}
      options={SSH_ENCODINGS}
      onChange={(nextValue) => {
        onChange(normalizeEncoding(nextValue));
      }}
    />
  );
}

interface TerminalModeSelectProps {
  value: TerminalMode;
  onChange: (value: TerminalMode) => void;
}

export function TerminalModeSelect({ value, onChange }: TerminalModeSelectProps) {
  const { t } = useTranslation();
  const terminalModeOptions = getTerminalModeOptions(t);

  return (
    <ConnectionSelect
      label={t("connection.terminal_mode")}
      value={value}
      options={terminalModeOptions}
      onChange={(nextValue) => {
        onChange(normalizeTerminalMode(nextValue));
      }}
    />
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
