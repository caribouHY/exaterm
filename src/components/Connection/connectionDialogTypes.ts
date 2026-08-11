import type {
  AppConfig,
  ConnectionHistoryEntry,
  ConnectionType,
  Encoding,
  PortInfo,
  SavedConnection,
  SshAuthMethod,
  TerminalMode,
  WorkspaceConnectionInfo,
} from "../../types";
import type { SshConnectionProgressEvent } from "./sshConnectionAttemptModel";

export interface ConnectionDialogInitialValues {
  connectionInfo: WorkspaceConnectionInfo;
  encoding: Encoding;
  terminalMode: TerminalMode;
}

export interface ConnectionDialogProps {
  initialValues?: ConnectionDialogInitialValues | null;
  startupRequest?: import("../../types").StartupCliRequest | null;
  onStartupRequestHandled?: () => void;
  onClose: () => void;
  onConnect: (
    type: ConnectionType,
    sessionId: string,
    title: string,
    isAutoLogging: boolean,
    encoding?: Encoding,
    terminalMode?: TerminalMode,
    connectionInfo?: WorkspaceConnectionInfo
  ) => void | Promise<void>;
}

export interface SshCredentialPrompt {
  phase: "jump" | "target";
  host: string;
  port: number;
  targetPort?: number;
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath: string;
  value: string;
  error: string;
}

export interface SshDiagnosticEvent {
  level: "info" | "error";
  message: string;
}

export interface SshConnectionProgressUpdate {
  requestId: string;
  progress: SshConnectionProgressEvent;
}

export type SshDiagnosticEntry = SshDiagnosticEvent & {
  id: number;
  time: string;
};

export interface ProfileSelectionState {
  ssh: string;
  telnet: string;
}

export interface SshFormState {
  selectedProfileId: string;
  selectedHistoryId: string;
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

export interface SshFormActions {
  onSelectSource: (value: string) => void;
  onDeleteProfile: () => void;
  onDeleteHistory: () => void;
  onProfileNameChange: (value: string) => void;
  onHostChange: (value: string) => void;
  onPortChange: (value: string) => void;
  onUsernameChange: (value: string) => void;
  onAuthMethodChange: (value: string) => void;
  onPrivateKeyPathChange: (value: string) => void;
  onSelectPrivateKeyFile: () => void;
  onJumpProfileChange: (value: string) => void;
  onEncodingChange: (value: string) => void;
  onTerminalModeChange: (value: string) => void;
  onMemoChange: (value: string) => void;
  onExternalControlEnabledChange: (value: boolean) => void;
  onSaveProfile: () => void;
}

export interface SshProfileOptions {
  profiles: SavedConnection[];
  historyEntries: ConnectionHistoryEntry[];
  jumpProfiles: SavedConnection[];
  getDisplayName: (profile: SavedConnection) => string;
  getHistoryDisplayName: (entry: ConnectionHistoryEntry) => string;
}

export interface TelnetFormState {
  selectedProfileId: string;
  selectedHistoryId: string;
  profileName: string;
  host: string;
  port: string;
  encoding: Encoding;
  terminalMode: TerminalMode;
  memo: string;
  externalControlEnabled: boolean;
}

export interface TelnetFormActions {
  onSelectSource: (value: string) => void;
  onDeleteProfile: () => void;
  onDeleteHistory: () => void;
  onProfileNameChange: (value: string) => void;
  onHostChange: (value: string) => void;
  onPortChange: (value: string) => void;
  onPortEnter: () => void;
  onEncodingChange: (value: string) => void;
  onTerminalModeChange: (value: string) => void;
  onMemoChange: (value: string) => void;
  onExternalControlEnabledChange: (value: boolean) => void;
  onSaveProfile: () => void;
}

export interface TelnetProfileOptions {
  profiles: SavedConnection[];
  historyEntries: ConnectionHistoryEntry[];
  getDisplayName: (profile: SavedConnection) => string;
  getHistoryDisplayName: (entry: ConnectionHistoryEntry) => string;
}

export interface SerialFormState {
  ports: PortInfo[];
  selectedPort: string;
  baudRate: string;
  dataBits: string;
  parity: string;
  stopBits: string;
  terminalMode: TerminalMode;
}

export interface SerialFormActions {
  onSelectedPortChange: (value: string) => void;
  onBaudRateChange: (value: string) => void;
  onDataBitsChange: (value: string) => void;
  onParityChange: (value: string) => void;
  onStopBitsChange: (value: string) => void;
  onTerminalModeChange: (value: string) => void;
}

export interface LoadedConfigState {
  config: AppConfig | null;
  setConfig: (config: AppConfig) => void;
  loadConfig: () => Promise<AppConfig>;
}
