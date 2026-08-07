/* ============================================
   ExaTerm — Types
   ============================================ */

export interface SshConnectParams {
  host: string;
  port: number;
  username: string;
  password: string;
  auth_method?: SshAuthMethod | null;
  private_key_path?: string | null;
  key_passphrase?: string | null;
  jump_profile_id?: string | null;
  jump_password?: string | null;
  jump_key_passphrase?: string | null;
  request_id?: string | null;
  cols: number;
  rows: number;
}

export type SshAuthMethod = "password" | "public_key";

export type HostKeyCheckStatus = "trusted" | "unknown" | "mismatch";

export interface HostKeyCheckResult {
  status: HostKeyCheckStatus;
  host: string;
  port: number;
  algorithm: string;
  fingerprint: string;
  known_fingerprint?: string;
}

export interface SerialConfig {
  baud_rate: number;
  data_bits: number;
  parity: string;
  stop_bits: number;
  flow_control: string;
}

export interface PortInfo {
  name: string;
  port_type: string;
}

export interface AiModelInfo {
  provider: string;
  model_id: string;
  display_name: string;
}

export interface ChatMessage {
  role: string;
  content: string;
  provider?: string;
  model_id?: string;
  is_error?: boolean;
}

export interface AiSecretStatus {
  openai: boolean;
  azure_openai: boolean;
  anthropic: boolean;
  gemini: boolean;
  openrouter: boolean;
}

export interface LogSession {
  session_id: string;
  connection_type: string;
  target: string;
  started_at: string;
  file_path: string;
  log_mode: "auto" | "manual";
}

export interface LogBulkDeleteResult {
  removed_history_count: number;
  removed_auto_file_count: number;
  skipped_active_count: number;
  skipped_manual_file_count: number;
  skipped_missing_file_count: number;
  skipped_unsafe_path_count: number;
}

export type ManualLogWriteMode = "overwrite" | "append";

export interface AppConfig {
  config_version: number;
  language: "system" | "en" | "ja" | (string & {});
  updates: UpdateConfig;
  connection_history: ConnectionHistoryConfig;
  ai: AiConfig;
  external_control: ExternalControlConfig;
  shortcuts: ShortcutConfig;
  terminal: TerminalConfig;
  ssh: SshConfig;
  saved_connections: SavedConnection[];
}

export interface UpdateConfig {
  check_on_startup: boolean;
}

export interface ConnectionHistoryConfig {
  enabled: boolean;
}

export interface ShortcutBinding {
  key: string;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
}

export interface ShortcutConfig {
  new_connection: ShortcutBinding | null;
  new_window: ShortcutBinding | null;
  open_settings: ShortcutBinding | null;
  exit: ShortcutBinding | null;
  terminal_select_all: ShortcutBinding | null;
  terminal_copy: ShortcutBinding | null;
  terminal_paste: ShortcutBinding | null;
  terminal_log_start_overwrite: ShortcutBinding | null;
  terminal_log_start_append: ShortcutBinding | null;
  terminal_log_stop: ShortcutBinding | null;
  terminal_log_pause: ShortcutBinding | null;
  terminal_log_resume: ShortcutBinding | null;
}

export interface ExternalControlConfig {
  enabled: boolean;
  connect_enabled: boolean;
  mcp_enabled: boolean;
  cli_enabled: boolean;
}

export interface AiConfig {
  azure_openai_enabled: boolean;
  azure_openai_endpoint: string;
  azure_openai_deployment: string;
  ollama_enabled: boolean;
  ollama_base_url: string;
  default_provider: string;
  default_model: string;
  debug_log_enabled: boolean;
}

export interface TerminalConfig {
  font_size: number;
  font_family: string;
  cursor_style: string;
  scrollback: number;
  auto_session_log: boolean;
  log_format: LogFormat;
  include_log_header: boolean;
}

export type LogFormat = "display" | "strip_controls";

export interface SshConfig {
  algorithm_mode: SshAlgorithmMode;
  algorithms: SshAlgorithmSelection;
}

export type SshAlgorithmMode = "default" | "custom";
export type SshAlgorithmGroup = "kex" | "host_key" | "cipher" | "mac" | "compression";

export interface SshAlgorithmSelection {
  kex: string[];
  host_key: string[];
  cipher: string[];
  mac: string[];
  compression: string[];
}

export interface SshAlgorithmCatalogItem {
  name: string;
  recommended: boolean;
  compatibility: boolean;
}

export type SshAlgorithmCatalog = Record<SshAlgorithmGroup, SshAlgorithmCatalogItem[]>;

export type TerminalMode = "general" | "cisco_ios";

export interface SavedConnection {
  id: string;
  connection_type: "ssh" | "telnet" | (string & {});
  host?: string | null;
  port?: number | null;
  username?: string | null;
  encoding?: Encoding | null;
  terminal_mode?: TerminalMode | null;
  auth_method?: SshAuthMethod | null;
  private_key_path?: string | null;
  jump_profile_id?: string | null;
  memo?: string | null;
  external_control_enabled?: boolean | null;
}

export type ConnectionType = "ssh" | "serial" | "telnet";
export type ViewMode = "terminal" | "settings" | "logs";
export type UtilityTabKind = "settings" | "logs";
export type Encoding = "utf-8" | "shift-jis" | "euc-jp";

export type WorkspaceConnectionInfo =
  | {
      kind: "ssh";
      host: string;
      port: number;
      username: string;
      auth_method: SshAuthMethod;
      private_key_path?: string | null;
      jump_profile_id?: string | null;
    }
  | {
      kind: "telnet";
      host: string;
      port: number;
    };

export interface ConnectionHistoryEntry {
  id: string;
  connection_info: WorkspaceConnectionInfo;
  encoding: Encoding;
  terminal_mode: TerminalMode;
  last_connected_at: string;
}

export interface ConnectionHistoryRecordInput {
  connection_info: WorkspaceConnectionInfo;
  encoding: Encoding;
  terminal_mode: TerminalMode;
}

export type StartupSshTargetKind = "direct" | "profile";

export interface StartupSshRequest {
  kind: "ssh";
  target_kind: StartupSshTargetKind;
  host?: string | null;
  username?: string | null;
  profile_name?: string | null;
  port?: number | null;
}

export interface StartupTelnetRequest {
  kind: "telnet";
  target: string;
  port?: number | null;
}

export type StartupCliRequest = StartupSshRequest | StartupTelnetRequest;

export interface TabInfo {
  kind: "terminal";
  id: string;
  title: string;
  connectionType: ConnectionType;
  sessionId?: string;
  isConnected: boolean;
  encoding: Encoding;
  terminalMode: TerminalMode;
  connectionInfo?: WorkspaceConnectionInfo;
  isAutoLogging?: boolean;
  isManualLogging?: boolean;
  isLoggingPaused?: boolean;
  manualLogFilePath?: string;
}

export interface WorkspaceTabInfo {
  tab_id: string;
  session_id: string;
  connection_type: ConnectionType;
  title: string;
  owner_window_id: string;
  encoding: Encoding;
  terminal_mode: TerminalMode;
  connection_info?: WorkspaceConnectionInfo | null;
  is_connected: boolean;
  is_auto_logging: boolean;
  is_manual_logging: boolean;
  is_logging_paused: boolean;
  manual_log_file_path?: string | null;
}

export interface WindowWorkspaceSnapshot {
  window_id: string;
  label: string;
  tab_order: string[];
  active_tab_id?: string | null;
}

export interface WorkspaceSnapshot {
  revision: number;
  window_id: string;
  window: WindowWorkspaceSnapshot;
  tabs: WorkspaceTabInfo[];
  tab_update?: WorkspaceTabUpdate | null;
}

export type WorkspaceTabUpdate =
  | { kind: "connected"; tab_id: string }
  | { kind: "moved"; tab_id: string; target_index: number };

export interface WorkspaceWindowCreateResult {
  window_id: string;
}

export interface WorkspacePointerPosition {
  x: number;
  y: number;
}

export interface ForeignTabPlacement {
  tabId: string;
  previousTabId: string | null;
  nextTabId: string | null;
  visibleSlotIndex: number;
}

export interface WorkspaceDragPreview {
  active: boolean;
  tab_id?: string | null;
  source_window_id?: string | null;
  pointer_screen_position?: WorkspacePointerPosition | null;
  target_window_id?: string | null;
  target_index?: number | null;
}

export interface WorkspaceDragDropResult {
  action: "move" | "detach" | string;
  tab_id: string;
  source_window_id: string;
  target_window_id?: string | null;
  created_window_id?: string | null;
  snapshots: WorkspaceSnapshot[];
}

export interface WorkspaceWindowCloseResult {
  window_id: string;
  rehome_window_id?: string | null;
  remaining_window_count: number;
  snapshots: WorkspaceSnapshot[];
}

export interface UtilityTabInfo {
  kind: UtilityTabKind;
  id: UtilityTabKind;
}

export type AppTabInfo = TabInfo | UtilityTabInfo;
