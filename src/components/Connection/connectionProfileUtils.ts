import type {
  AppConfig,
  Encoding,
  SavedConnection,
  SshAuthMethod,
  TerminalMode,
} from "../../types";

export const SSH_ENCODINGS: { label: string; value: Encoding }[] = [
  { label: "UTF-8", value: "utf-8" },
  { label: "Shift-JIS", value: "shift-jis" },
  { label: "EUC-JP", value: "euc-jp" },
];

export const SSH_AUTH_METHODS: { labelKey: string; value: SshAuthMethod }[] = [
  { labelKey: "connection.auth_password", value: "password" },
  { labelKey: "connection.auth_public_key", value: "public_key" },
];

export const SSH_KEY_PATH_PLACEHOLDER = ["C:", "Users", "user", ".ssh", "id_ed25519"].join("\\");

export const normalizeEncoding = (encoding: string | null | undefined): Encoding => {
  return SSH_ENCODINGS.some((entry) => entry.value === encoding) ? (encoding as Encoding) : "utf-8";
};

export const normalizeSshAuthMethod = (authMethod: string | null | undefined): SshAuthMethod => {
  return authMethod === "public_key" ? "public_key" : "password";
};

export const normalizeProfileMemo = (memo: string): string | null => {
  const trimmed = memo.trim();
  return trimmed ? trimmed : null;
};

export const normalizeProfileExternalControlEnabled = (
  enabled: boolean | null | undefined
): boolean => {
  return enabled ?? true;
};

export interface SshProfileDraft {
  profileName: string;
  host: string;
  port: string;
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath: string;
  jumpProfileId: string;
  encoding: Encoding;
  terminalMode: TerminalMode;
  memo: string;
  externalControlEnabled: boolean;
}

export interface TelnetProfileDraft {
  profileName: string;
  host: string;
  port: string;
  encoding: Encoding;
  terminalMode: TerminalMode;
  memo: string;
  externalControlEnabled: boolean;
}

export const createSshProfile = (draft: SshProfileDraft): SavedConnection => ({
  id: draft.profileName.trim(),
  connection_type: "ssh",
  host: draft.host.trim(),
  port: Number.parseInt(draft.port, 10),
  username: draft.username.trim(),
  auth_method: draft.authMethod,
  private_key_path: draft.authMethod === "public_key" ? draft.privateKeyPath.trim() : null,
  jump_profile_id: draft.jumpProfileId || null,
  encoding: draft.encoding,
  terminal_mode: draft.terminalMode,
  memo: normalizeProfileMemo(draft.memo),
  external_control_enabled: draft.externalControlEnabled,
});

export const createTelnetProfile = (draft: TelnetProfileDraft): SavedConnection => ({
  id: draft.profileName.trim(),
  connection_type: "telnet",
  host: draft.host.trim(),
  port: Number.parseInt(draft.port, 10),
  encoding: draft.encoding,
  terminal_mode: draft.terminalMode,
  memo: normalizeProfileMemo(draft.memo),
  external_control_enabled: draft.externalControlEnabled,
});

export const hasDuplicateProfile = (
  connections: SavedConnection[],
  profile: SavedConnection,
  selectedProfileId: string
) => {
  return connections.some(
    (entry) =>
      entry.connection_type === profile.connection_type &&
      entry.id === profile.id &&
      entry.id !== selectedProfileId
  );
};

export const upsertSavedProfile = (
  config: AppConfig,
  profile: SavedConnection,
  selectedProfileId: string
): AppConfig => {
  const existingConnections = config.saved_connections;
  const shouldUpdate = Boolean(selectedProfileId);
  return {
    ...config,
    saved_connections: shouldUpdate
      ? existingConnections.map((entry) =>
          entry.connection_type === profile.connection_type && entry.id === selectedProfileId
            ? profile
            : entry
        )
      : [...existingConnections, profile],
  };
};

export const removeSavedProfile = (
  config: AppConfig,
  connectionType: "ssh" | "telnet",
  profileId: string
): AppConfig => ({
  ...config,
  saved_connections: config.saved_connections.filter(
    (entry) => entry.connection_type !== connectionType || entry.id !== profileId
  ),
});

export const getConnectionErrorMessage = (error: unknown, fallback: string) => {
  return typeof error === "string" ? error : error instanceof Error ? error.message : fallback;
};
