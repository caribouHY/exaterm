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
  language: string;
  ai: AiConfig;
  mcp: McpConfig;
  terminal: TerminalConfig;
  ssh: SshConfig;
  saved_connections: SavedConnection[];
}

export interface McpConfig {
  enabled: boolean;
  connect_enabled: boolean;
  host: string;
  port: number;
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
  allow_legacy_algorithms: boolean;
}

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
}

export type ConnectionType = "ssh" | "serial" | "telnet";
export type ViewMode = "terminal" | "settings" | "logs";
export type UtilityTabKind = "settings" | "logs";
export type Encoding = "utf-8" | "shift-jis" | "euc-jp";

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
  isAutoLogging?: boolean;
  isManualLogging?: boolean;
  isLoggingPaused?: boolean;
  manualLogFilePath?: string;
}

export interface UtilityTabInfo {
  kind: UtilityTabKind;
  id: UtilityTabKind;
}

export type AppTabInfo = TabInfo | UtilityTabInfo;
