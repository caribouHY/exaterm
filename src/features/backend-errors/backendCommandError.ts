export type BackendErrorParam = string | number | boolean;
export type BackendErrorTranslator = (
  key: string,
  params?: Record<string, BackendErrorParam>
) => string;

export interface BackendCommandError {
  code: string;
  message: string;
  params?: Record<string, BackendErrorParam>;
}

const errorKeys: Record<string, string> = {
  "terminal.session_not_found": "backend_errors.terminal_session_not_found",
  "terminal.cursor_out_of_range": "backend_errors.terminal_cursor_out_of_range",
  "workspace.window_not_found": "backend_errors.workspace_window_not_found",
  "workspace.source_window_not_found": "backend_errors.workspace_source_window_not_found",
  "workspace.destination_window_not_found": "backend_errors.workspace_destination_window_not_found",
  "workspace.tab_not_found": "backend_errors.workspace_tab_not_found",
  "workspace.source_tab_not_found": "backend_errors.workspace_source_tab_not_found",
  "workspace.destination_tab_not_found": "backend_errors.workspace_destination_tab_not_found",
  "workspace.tab_not_in_window": "backend_errors.workspace_tab_not_in_window",
  "workspace.tab_owner_mismatch": "backend_errors.workspace_tab_owner_mismatch",
  "workspace.no_active_drag": "backend_errors.workspace_no_active_drag",
  "workspace.destination_snapshot_not_found":
    "backend_errors.workspace_destination_snapshot_not_found",
  "workspace.window_create_failed": "backend_errors.workspace_window_create_failed",
  "logger.history_read_failed": "backend_errors.logger_history_read_failed",
  "logger.history_parse_failed": "backend_errors.logger_history_parse_failed",
  "logger.directory_create_failed": "backend_errors.logger_directory_create_failed",
  "logger.file_create_failed": "backend_errors.logger_file_create_failed",
  "logger.file_write_failed": "backend_errors.logger_file_write_failed",
  "logger.file_delete_failed": "backend_errors.logger_file_delete_failed",
  "serial.list_ports_failed": "backend_errors.serial_list_ports_failed",
  "serial.open_failed": "backend_errors.serial_open_failed",
  "serial.clone_failed": "backend_errors.serial_clone_failed",
  "terminal.send_failed": "backend_errors.terminal_send_failed",
  "telnet.connect_failed": "backend_errors.telnet_connect_failed",
  "telnet.resize_failed": "backend_errors.telnet_resize_failed",
  "ssh.host_key_mismatch": "backend_errors.ssh_host_key_mismatch",
  "ssh.connect_cancelled": "backend_errors.ssh_connect_cancelled",
  "ssh.connect_request_id_required": "backend_errors.ssh_connect_request_id_required",
  "ssh.invalid_auth_method": "backend_errors.ssh_invalid_auth_method",
  "ssh.auth_prompt_cancelled": "backend_errors.ssh_auth_prompt_cancelled",
  "ssh.auth_prompt_timed_out": "backend_errors.ssh_auth_prompt_timed_out",
  "ssh.auth_prompt_response_mismatch": "backend_errors.ssh_auth_prompt_response_mismatch",
  "ssh.host_key_prompt_cancelled": "backend_errors.ssh_host_key_prompt_cancelled",
  "ssh.host_key_prompt_timed_out": "backend_errors.ssh_host_key_prompt_timed_out",
  "ssh.host_key_prompt_request_not_found": "backend_errors.ssh_host_key_prompt_request_not_found",
  "ssh.host_key_prompt_already_finished": "backend_errors.ssh_host_key_prompt_already_finished",
  "ssh.host_key_retrieval_failed": "backend_errors.ssh_host_key_retrieval_failed",
  "ssh.jump_profile_self_reference": "backend_errors.ssh_jump_profile_self_reference",
  "ssh.jump_profile_not_found": "backend_errors.ssh_jump_profile_not_found",
  "ssh.jump_profile_wrong_type": "backend_errors.ssh_jump_profile_wrong_type",
  "ssh.jump_profile_nested": "backend_errors.ssh_jump_profile_nested",
  "ssh.jump_profile_host_missing": "backend_errors.ssh_jump_profile_host_missing",
  "ssh.jump_profile_username_missing": "backend_errors.ssh_jump_profile_username_missing",
  "ssh.jump_profile_key_missing": "backend_errors.ssh_jump_profile_key_missing",
  "ssh.private_key_required": "backend_errors.ssh_private_key_required",
  "ssh.pty_request_failed": "backend_errors.ssh_pty_request_failed",
  "ssh.shell_request_failed": "backend_errors.ssh_shell_request_failed",
  "ssh.channel_open_failed": "backend_errors.ssh_channel_open_failed",
  "ssh.jump_channel_open_failed": "backend_errors.ssh_jump_channel_open_failed",
  "ssh.connection_failed": "backend_errors.ssh_connection_failed",
  "ssh.public_key_auth_failed": "backend_errors.ssh_public_key_auth_failed",
  "ssh.authentication_failed": "backend_errors.ssh_authentication_failed",
  "ssh.private_key_open_failed": "backend_errors.ssh_private_key_open_failed",
  "ssh.private_key_load_failed": "backend_errors.ssh_private_key_load_failed",
  "config.load_failed": "backend_errors.config_load_failed",
  "connection.unknown_type": "backend_errors.connection_unknown_type",
  "ai.secret_empty": "backend_errors.ai_secret_empty",
  "ai.unsupported_secret_provider": "backend_errors.ai_unsupported_secret_provider",
  "ai.model_not_selected": "backend_errors.ai_model_not_selected",
  "ai.azure_endpoint_missing": "backend_errors.ai_azure_endpoint_missing",
  "ai.azure_endpoint_invalid": "backend_errors.ai_azure_endpoint_invalid",
  "ai.secret_missing": "backend_errors.ai_secret_missing",
  "ai.ollama_unavailable": "backend_errors.ai_ollama_unavailable",
  "ai.provider_unavailable": "backend_errors.ai_provider_unavailable",
  "ai.authentication_failed": "backend_errors.ai_authentication_failed",
  "ai.model_not_found": "backend_errors.ai_model_not_found",
  "ai.quota_exceeded": "backend_errors.ai_quota_exceeded",
  "ai.server_error": "backend_errors.ai_server_error",
  "ai.http_error": "backend_errors.ai_http_error",
};

function isBackendCommandError(value: unknown): value is BackendCommandError {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<BackendCommandError>;
  return typeof candidate.code === "string" && typeof candidate.message === "string";
}

export function parseBackendCommandError(error: unknown): BackendCommandError | null {
  if (isBackendCommandError(error)) return error;
  if (typeof error !== "string") return null;
  try {
    const parsed: unknown = JSON.parse(error);
    return isBackendCommandError(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function translateBackendCommandError(
  error: unknown,
  t: BackendErrorTranslator,
  fallback: string
): string {
  const commandError = parseBackendCommandError(error);
  if (!commandError) return typeof error === "string" ? error : fallback;
  const key = errorKeys[commandError.code];
  return key ? t(key, commandError.params ?? {}) : commandError.message || fallback;
}

export function backendCommandErrorMessage(error: unknown, fallback: string): string {
  const commandError = parseBackendCommandError(error);
  if (commandError) return commandError.message || fallback;
  return typeof error === "string" ? error : error instanceof Error ? error.message : fallback;
}
