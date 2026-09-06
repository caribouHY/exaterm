import type { Encoding, SavedConnection, SshAuthMethod, TerminalMode } from "../../types";
import {
  translateBackendCommandError,
  type BackendErrorTranslator,
} from "../../features/backend-errors/backendCommandError";

export const SSH_ENCODINGS: { label: string; value: Encoding }[] = [
  { label: "UTF-8", value: "utf-8" },
  { label: "Shift-JIS", value: "shift-jis" },
  { label: "EUC-JP", value: "euc-jp" },
];

export const SSH_AUTH_METHODS: { labelKey: string; value: SshAuthMethod }[] = [
  { labelKey: "connection.auth_auto", value: "auto" },
  { labelKey: "connection.auth_password", value: "password" },
  { labelKey: "connection.auth_keyboard_interactive", value: "keyboard_interactive" },
  { labelKey: "connection.auth_public_key", value: "public_key" },
];

export const SSH_KEY_PATH_PLACEHOLDER = ["C:", "Users", "user", ".ssh", "id_ed25519"].join("\\");

export const normalizeEncoding = (encoding: string | null | undefined): Encoding => {
  return SSH_ENCODINGS.some((entry) => entry.value === encoding) ? (encoding as Encoding) : "utf-8";
};

export const normalizeSshAuthMethod = (authMethod: string | null | undefined): SshAuthMethod => {
  if (authMethod == null || authMethod.trim() === "") return "auto";
  return SSH_AUTH_METHODS.some((entry) => entry.value === authMethod)
    ? (authMethod as SshAuthMethod)
    : "password";
};

export const usesPrivateKeyAuthentication = (authMethod: SshAuthMethod): boolean =>
  authMethod === "auto" || authMethod === "public_key";

export const resolveSshAuthentication = (
  authMethod: SshAuthMethod,
  connectionPrivateKeyPath: string,
  defaultPrivateKeyPath: string
): { authMethod: SshAuthMethod; privateKeyPath: string } => {
  const connectionPath = connectionPrivateKeyPath.trim();
  if (authMethod === "public_key") return { authMethod, privateKeyPath: connectionPath };
  if (authMethod !== "auto") return { authMethod, privateKeyPath: "" };
  return {
    authMethod,
    privateKeyPath: connectionPath || defaultPrivateKeyPath.trim(),
  };
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
  private_key_path: usesPrivateKeyAuthentication(draft.authMethod)
    ? draft.privateKeyPath.trim()
    : null,
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

export const getConnectionErrorMessage = (
  error: unknown,
  t: BackendErrorTranslator,
  fallback: string
) => translateBackendCommandError(error, t, fallback);
