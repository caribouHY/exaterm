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
import type { SshConnectionProgressEvent } from "./connectionProtocolEvents";
import type { SerialPortDetectionStatus } from "./serialPortDetection";
import type { ConnectionLogState } from "../../features/terminal-logging/connectionLogModel";
import type {
  SerialConnectionValidationErrors,
  SshConnectionValidationErrors,
  TelnetConnectionValidationErrors,
} from "./connectionFormValidation";

export interface ConnectionDialogInitialValues {
  connectionInfo: WorkspaceConnectionInfo;
  profileId?: string;
  encoding: Encoding;
  terminalMode: TerminalMode;
}

export interface ConnectionDialogProps {
  shortcutsEnabled?: boolean;
  initialValues?: ConnectionDialogInitialValues | null;
  startupRequest?: import("../../types").StartupCliRequest | null;
  onStartupRequestHandled?: () => void;
  onClose: () => void;
  onConnect: (
    type: ConnectionType,
    sessionId: string,
    title: string,
    logState: ConnectionLogState,
    encoding?: Encoding,
    terminalMode?: TerminalMode,
    connectionInfo?: WorkspaceConnectionInfo
  ) => void | Promise<void>;
}

export interface SshCredentialPrompt {
  profileName?: string;
  phase: "jump" | "target";
  host: string;
  port: number;
  targetPort?: number;
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath: string;
  requestId: string;
  promptId: string;
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

export interface NetworkFormState {
  selectedProfileId: string;
  selectedHistoryId: string;
  profileName: string;
  group: string;
  host: string;
  port: string;
  encoding: Encoding;
  terminalMode: TerminalMode;
  memo: string;
  externalControlEnabled: boolean;
}

export interface SshFormState extends NetworkFormState {
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath: string;
  jumpProfileId: string;
  validationErrors: SshConnectionValidationErrors;
}

export interface NetworkFormActions {
  onCopyProfile: () => boolean;
  onSelectSource: (value: string) => void;
  onDeleteProfile: () => void;
  onDeleteHistory: () => void;
  onProfileNameChange: (value: string) => void;
  onGroupChange: (value: string) => void;
  onHostChange: (value: string) => void;
  onPortChange: (value: string) => void;
  onEncodingChange: (value: string) => void;
  onTerminalModeChange: (value: string) => void;
  onMemoChange: (value: string) => void;
  onExternalControlEnabledChange: (value: boolean) => void;
  onSaveProfile: (reportError: (error: string) => void) => void;
}

export interface SshFormActions extends NetworkFormActions {
  onUsernameChange: (value: string) => void;
  onAuthMethodChange: (value: string) => void;
  onPrivateKeyPathChange: (value: string) => void;
  onSelectPrivateKeyFile: () => void;
  onJumpProfileChange: (value: string) => void;
}

export interface NetworkProfileOptions {
  profiles: SavedConnection[];
  groupNames: string[];
  historyEntries: ConnectionHistoryEntry[];
  getDisplayName: (profile: SavedConnection) => string;
  getHistoryDisplayName: (entry: ConnectionHistoryEntry) => string;
}

export interface SshProfileOptions extends NetworkProfileOptions {
  jumpProfiles: SavedConnection[];
}

export interface TelnetFormState extends NetworkFormState {
  validationErrors: TelnetConnectionValidationErrors;
}

export interface TelnetFormActions extends NetworkFormActions {
  onPortEnter: () => void;
}

export interface SerialFormState {
  ports: PortInfo[];
  portDetectionStatus: SerialPortDetectionStatus;
  selectedPort: string;
  baudRate: string;
  dataBits: string;
  parity: string;
  stopBits: string;
  terminalMode: TerminalMode;
  validationErrors: SerialConnectionValidationErrors;
}

export interface SerialFormActions {
  onRefreshPorts: () => void;
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
