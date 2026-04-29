/* ============================================
   ExaTerm — Types
   ============================================ */

export interface SshConnectParams {
  host: string;
  port: number;
  username: string;
  password: string;
  cols: number;
  rows: number;
}

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
}

export interface LogSession {
  session_id: string;
  connection_type: string;
  target: string;
  started_at: string;
  file_path: string;
}

export interface AppConfig {
  config_version: number;
  language: string;
  ai: AiConfig;
  terminal: TerminalConfig;
  ssh: SshConfig;
  saved_connections: SavedConnection[];
}

export interface AiConfig {
  azure_openai_enabled: boolean;
  azure_openai_endpoint: string;
  azure_openai_deployment: string;
  ollama_enabled: boolean;
  ollama_base_url: string;
  default_provider: string;
  default_model: string;
}

export interface TerminalConfig {
  font_size: number;
  font_family: string;
  cursor_style: string;
  scrollback: number;
  auto_session_log: boolean;
}

export interface SshConfig {
  allow_legacy_algorithms: boolean;
}

export interface SavedConnection {
  id: string;
  name: string;
  connection_type: string;
  host?: string | null;
  port?: number | null;
  username?: string | null;
  serial_port?: string | null;
  baud_rate?: number | null;
}

export type ConnectionType = "ssh" | "serial" | "telnet";
export type ViewMode = "terminal" | "settings" | "logs";
export type Encoding = "utf-8" | "shift-jis" | "euc-jp";

export interface TabInfo {
  id: string;
  title: string;
  connectionType: ConnectionType;
  sessionId?: string;
  isConnected: boolean;
  encoding: Encoding;
}
