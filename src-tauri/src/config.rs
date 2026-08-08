use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

use crate::ai::{DEFAULT_AI_MODEL, DEFAULT_AI_PROVIDER};
use crate::terminal_control::TerminalControlState;

const CURRENT_CONFIG_VERSION: u32 = 6;

#[derive(Debug, Clone, Serialize)]
pub struct AppConfig {
    pub config_version: u32,
    pub language: String,
    pub updates: UpdateConfig,
    pub connection_history: ConnectionHistoryConfig,
    pub ai: AiConfig,
    pub external_control: ExternalControlConfig,
    pub shortcuts: ShortcutConfig,
    pub terminal: TerminalConfig,
    pub ssh: SshConfig,
    pub saved_connections: Vec<SavedConnection>,
}

fn default_language() -> String {
    "system".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateConfig {
    #[serde(default = "default_check_on_startup")]
    pub check_on_startup: bool,
}

fn default_check_on_startup() -> bool {
    true
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            check_on_startup: default_check_on_startup(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionHistoryConfig {
    #[serde(default = "default_connection_history_enabled")]
    pub enabled: bool,
}

fn default_connection_history_enabled() -> bool {
    true
}

impl Default for ConnectionHistoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_connection_history_enabled(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ShortcutBinding {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortcutConfig {
    #[serde(default = "default_new_connection_shortcut")]
    pub new_connection: Option<ShortcutBinding>,
    #[serde(default = "default_new_window_shortcut")]
    pub new_window: Option<ShortcutBinding>,
    #[serde(default = "default_open_settings_shortcut")]
    pub open_settings: Option<ShortcutBinding>,
    #[serde(default)]
    pub exit: Option<ShortcutBinding>,
    #[serde(default = "default_terminal_select_all_shortcut")]
    pub terminal_select_all: Option<ShortcutBinding>,
    #[serde(default = "default_terminal_copy_shortcut")]
    pub terminal_copy: Option<ShortcutBinding>,
    #[serde(default = "default_terminal_paste_shortcut")]
    pub terminal_paste: Option<ShortcutBinding>,
    #[serde(default = "default_terminal_log_start_overwrite_shortcut")]
    pub terminal_log_start_overwrite: Option<ShortcutBinding>,
    #[serde(default)]
    pub terminal_log_start_append: Option<ShortcutBinding>,
    #[serde(default = "default_terminal_log_stop_shortcut")]
    pub terminal_log_stop: Option<ShortcutBinding>,
    #[serde(default)]
    pub terminal_log_pause: Option<ShortcutBinding>,
    #[serde(default)]
    pub terminal_log_resume: Option<ShortcutBinding>,
}

fn shortcut(key: &str, ctrl: bool, alt: bool, shift: bool) -> Option<ShortcutBinding> {
    Some(ShortcutBinding {
        key: key.into(),
        ctrl,
        alt,
        shift,
    })
}

fn default_new_connection_shortcut() -> Option<ShortcutBinding> {
    shortcut("n", true, false, false)
}

fn default_new_window_shortcut() -> Option<ShortcutBinding> {
    shortcut("n", true, false, true)
}

fn default_open_settings_shortcut() -> Option<ShortcutBinding> {
    shortcut(",", true, false, false)
}

fn default_terminal_select_all_shortcut() -> Option<ShortcutBinding> {
    shortcut("a", true, false, true)
}

fn default_terminal_copy_shortcut() -> Option<ShortcutBinding> {
    shortcut("c", true, false, true)
}

fn default_terminal_paste_shortcut() -> Option<ShortcutBinding> {
    shortcut("v", true, false, true)
}

fn default_terminal_log_start_overwrite_shortcut() -> Option<ShortcutBinding> {
    shortcut("F9", true, false, true)
}

fn default_terminal_log_stop_shortcut() -> Option<ShortcutBinding> {
    shortcut("F10", true, false, true)
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            new_connection: default_new_connection_shortcut(),
            new_window: default_new_window_shortcut(),
            open_settings: default_open_settings_shortcut(),
            exit: None,
            terminal_select_all: default_terminal_select_all_shortcut(),
            terminal_copy: default_terminal_copy_shortcut(),
            terminal_paste: default_terminal_paste_shortcut(),
            terminal_log_start_overwrite: default_terminal_log_start_overwrite_shortcut(),
            terminal_log_start_append: None,
            terminal_log_stop: default_terminal_log_stop_shortcut(),
            terminal_log_pause: None,
            terminal_log_resume: None,
        }
    }
}

impl ShortcutBinding {
    fn normalize(&mut self) {
        if self.key == " " || self.key.eq_ignore_ascii_case("spacebar") {
            self.key = "Space".into();
        } else if is_function_key(&self.key) {
            self.key.make_ascii_uppercase();
        } else if self.key.len() == 1 && self.key.as_bytes()[0].is_ascii_uppercase() {
            self.key.make_ascii_lowercase();
        }
    }
}

impl ShortcutConfig {
    fn normalize(&mut self) {
        for binding in [
            &mut self.new_connection,
            &mut self.new_window,
            &mut self.open_settings,
            &mut self.exit,
            &mut self.terminal_select_all,
            &mut self.terminal_copy,
            &mut self.terminal_paste,
            &mut self.terminal_log_start_overwrite,
            &mut self.terminal_log_start_append,
            &mut self.terminal_log_stop,
            &mut self.terminal_log_pause,
            &mut self.terminal_log_resume,
        ]
        .into_iter()
        .flatten()
        {
            binding.normalize();
        }
    }
}

fn is_function_key(key: &str) -> bool {
    key.get(0..1)
        .filter(|prefix| prefix.eq_ignore_ascii_case("f"))
        .and_then(|_| key.get(1..))
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (1..=12).contains(&number))
}

fn validate_shortcut_config(shortcuts: &ShortcutConfig) -> Result<(), String> {
    let mut bindings = HashSet::new();
    for (action, binding) in [
        ("new_connection", &shortcuts.new_connection),
        ("new_window", &shortcuts.new_window),
        ("open_settings", &shortcuts.open_settings),
        ("exit", &shortcuts.exit),
        ("terminal_select_all", &shortcuts.terminal_select_all),
        ("terminal_copy", &shortcuts.terminal_copy),
        ("terminal_paste", &shortcuts.terminal_paste),
        (
            "terminal_log_start_overwrite",
            &shortcuts.terminal_log_start_overwrite,
        ),
        (
            "terminal_log_start_append",
            &shortcuts.terminal_log_start_append,
        ),
        ("terminal_log_stop", &shortcuts.terminal_log_stop),
        ("terminal_log_pause", &shortcuts.terminal_log_pause),
        ("terminal_log_resume", &shortcuts.terminal_log_resume),
    ] {
        let Some(binding) = binding else {
            continue;
        };

        let is_printable_key = binding.key == "Space"
            || (binding.key.chars().count() == 1
                && binding
                    .key
                    .chars()
                    .all(|character| !character.is_control() && !character.is_whitespace()));
        let function_key = is_function_key(&binding.key);
        if !is_printable_key && !function_key {
            return Err(format!("Invalid shortcut key for {action}"));
        }
        if !function_key && !binding.ctrl && !binding.alt {
            return Err(format!("Shortcut for {action} requires Ctrl or Alt"));
        }
        if binding.alt && binding.key == "F4" {
            return Err(format!("Alt+F4 cannot be assigned to {action}"));
        }
        if !bindings.insert(binding.clone()) {
            return Err("Shortcut assignments must be unique".into());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalControlConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub connect_enabled: bool,
    #[serde(default)]
    pub mcp_enabled: bool,
    #[serde(default)]
    pub cli_enabled: bool,
}

impl Default for ExternalControlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            connect_enabled: false,
            mcp_enabled: false,
            cli_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ExternalControlConfigInput {
    enabled: Option<bool>,
    connect_enabled: Option<bool>,
    mcp_enabled: Option<bool>,
    cli_enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LegacyMcpConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    connect_enabled: bool,
    #[serde(default)]
    stdio_enabled: bool,
    #[serde(default)]
    cli_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub azure_openai_enabled: bool,
    #[serde(default)]
    pub azure_openai_endpoint: String,
    #[serde(default)]
    pub azure_openai_deployment: String,
    #[serde(default)]
    pub ollama_enabled: bool,
    #[serde(default = "default_ollama_url")]
    pub ollama_base_url: String,
    #[serde(default = "default_ai_provider")]
    pub default_provider: String,
    #[serde(default = "default_ai_model")]
    pub default_model: String,
    #[serde(default)]
    pub debug_log_enabled: bool,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            azure_openai_enabled: false,
            azure_openai_endpoint: String::new(),
            azure_openai_deployment: String::new(),
            ollama_enabled: false,
            ollama_base_url: default_ollama_url(),
            default_provider: DEFAULT_AI_PROVIDER.into(),
            default_model: DEFAULT_AI_MODEL.into(),
            debug_log_enabled: false,
        }
    }
}

fn default_ai_provider() -> String {
    DEFAULT_AI_PROVIDER.into()
}

fn default_ai_model() -> String {
    DEFAULT_AI_MODEL.into()
}

fn default_ollama_url() -> String {
    "http://localhost:11434".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshAlgorithmSelection {
    #[serde(default)]
    pub kex: Vec<String>,
    #[serde(default)]
    pub host_key: Vec<String>,
    #[serde(default)]
    pub cipher: Vec<String>,
    #[serde(default)]
    pub mac: Vec<String>,
    #[serde(default)]
    pub compression: Vec<String>,
}

impl Default for SshAlgorithmSelection {
    fn default() -> Self {
        Self {
            kex: Vec::new(),
            host_key: Vec::new(),
            cipher: Vec::new(),
            mac: Vec::new(),
            compression: Vec::new(),
        }
    }
}

fn default_ssh_algorithm_mode() -> String {
    "default".into()
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SshConfigInput {
    algorithm_mode: Option<String>,
    algorithms: Option<SshAlgorithmSelection>,
    allow_legacy_algorithms: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SshConfig {
    pub algorithm_mode: String,
    pub algorithms: SshAlgorithmSelection,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            algorithm_mode: default_ssh_algorithm_mode(),
            algorithms: SshAlgorithmSelection::default(),
        }
    }
}

impl<'de> Deserialize<'de> for SshConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = SshConfigInput::deserialize(deserializer)?;
        if let Some(algorithm_mode) = input.algorithm_mode {
            return Ok(Self {
                algorithm_mode,
                algorithms: input.algorithms.unwrap_or_default(),
            });
        }

        if input.allow_legacy_algorithms.unwrap_or(false) {
            return Ok(Self {
                algorithm_mode: "custom".into(),
                algorithms: crate::ssh::legacy_algorithm_selection(),
            });
        }

        Ok(Self::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    #[serde(default = "default_terminal_font_size")]
    pub font_size: u32,
    #[serde(default = "default_terminal_font_family")]
    pub font_family: String,
    #[serde(default = "default_terminal_cursor_style")]
    pub cursor_style: String,
    #[serde(default = "default_terminal_scrollback")]
    pub scrollback: u32,
    #[serde(default)]
    pub auto_session_log: bool,
    #[serde(default = "default_terminal_log_format")]
    pub log_format: String,
    #[serde(default = "default_terminal_include_log_header")]
    pub include_log_header: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            font_size: default_terminal_font_size(),
            font_family: default_terminal_font_family(),
            cursor_style: default_terminal_cursor_style(),
            scrollback: default_terminal_scrollback(),
            auto_session_log: false,
            log_format: default_terminal_log_format(),
            include_log_header: default_terminal_include_log_header(),
        }
    }
}

fn default_terminal_font_size() -> u32 {
    14
}

fn default_terminal_font_family() -> String {
    "Consolas, 'Courier New', monospace".into()
}

fn default_terminal_cursor_style() -> String {
    "block".into()
}

fn default_terminal_scrollback() -> u32 {
    10000
}

fn default_terminal_log_format() -> String {
    "display".into()
}

fn default_terminal_include_log_header() -> bool {
    false
}

fn default_saved_connection_external_control_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedConnection {
    pub id: String,
    pub connection_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub encoding: Option<String>,
    pub terminal_mode: Option<String>,
    pub auth_method: Option<String>,
    pub private_key_path: Option<String>,
    pub jump_profile_id: Option<String>,
    pub memo: Option<String>,
    pub external_control_enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SavedConnectionInput {
    id: Option<String>,
    connection_type: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    encoding: Option<String>,
    terminal_mode: Option<String>,
    auth_method: Option<String>,
    private_key_path: Option<String>,
    jump_profile_id: Option<String>,
    memo: Option<String>,
    external_control_enabled: Option<bool>,
    #[serde(rename = "mcp_enabled")]
    legacy_mcp_enabled: Option<bool>,
}

impl Default for SavedConnection {
    fn default() -> Self {
        Self {
            id: String::new(),
            connection_type: String::new(),
            host: None,
            port: None,
            username: None,
            encoding: None,
            terminal_mode: None,
            auth_method: None,
            private_key_path: None,
            jump_profile_id: None,
            memo: None,
            external_control_enabled: default_saved_connection_external_control_enabled(),
        }
    }
}

impl<'de> Deserialize<'de> for SavedConnection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = SavedConnectionInput::deserialize(deserializer)?;
        Ok(Self {
            id: input.id.unwrap_or_default(),
            connection_type: input.connection_type.unwrap_or_default(),
            host: input.host,
            port: input.port,
            username: input.username,
            encoding: input.encoding,
            terminal_mode: input.terminal_mode,
            auth_method: input.auth_method,
            private_key_path: input.private_key_path,
            jump_profile_id: input.jump_profile_id,
            memo: input.memo,
            external_control_enabled: input
                .external_control_enabled
                .or(input.legacy_mcp_enabled)
                .unwrap_or_else(default_saved_connection_external_control_enabled),
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct AppConfigInput {
    config_version: Option<u32>,
    language: Option<String>,
    updates: Option<UpdateConfig>,
    connection_history: Option<ConnectionHistoryConfig>,
    ai: Option<AiConfig>,
    external_control: Option<ExternalControlConfigInput>,
    #[serde(rename = "mcp")]
    legacy_mcp: Option<LegacyMcpConfig>,
    shortcuts: Option<ShortcutConfig>,
    terminal: Option<TerminalConfig>,
    ssh: Option<SshConfig>,
    saved_connections: Option<Vec<SavedConnection>>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            language: default_language(),
            updates: UpdateConfig::default(),
            connection_history: ConnectionHistoryConfig::default(),
            ai: AiConfig::default(),
            external_control: ExternalControlConfig::default(),
            shortcuts: ShortcutConfig::default(),
            terminal: TerminalConfig::default(),
            ssh: SshConfig::default(),
            saved_connections: Vec::new(),
        }
    }
}

impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = AppConfigInput::deserialize(deserializer)?;
        let defaults = AppConfig::default();
        let legacy_mcp = input.legacy_mcp.unwrap_or_default();
        let external_control = input.external_control.unwrap_or_default();

        Ok(Self {
            config_version: input.config_version.unwrap_or(0),
            language: input.language.unwrap_or_else(default_language),
            updates: input.updates.unwrap_or(defaults.updates),
            connection_history: input
                .connection_history
                .unwrap_or(defaults.connection_history),
            ai: input.ai.unwrap_or(defaults.ai),
            external_control: ExternalControlConfig {
                enabled: external_control.enabled.unwrap_or(legacy_mcp.enabled),
                connect_enabled: external_control
                    .connect_enabled
                    .unwrap_or(legacy_mcp.connect_enabled),
                mcp_enabled: external_control
                    .mcp_enabled
                    .unwrap_or(legacy_mcp.stdio_enabled),
                cli_enabled: external_control
                    .cli_enabled
                    .unwrap_or(legacy_mcp.cli_enabled),
            },
            shortcuts: input.shortcuts.unwrap_or(defaults.shortcuts),
            terminal: input.terminal.unwrap_or(defaults.terminal),
            ssh: input.ssh.unwrap_or(defaults.ssh),
            saved_connections: input.saved_connections.unwrap_or_default(),
        })
    }
}

impl AppConfig {
    fn migrate(mut self) -> Self {
        if self.config_version < CURRENT_CONFIG_VERSION {
            self.config_version = CURRENT_CONFIG_VERSION;
        }
        self.shortcuts.normalize();
        self
    }
}

fn config_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join("config.json")
}

#[tauri::command]
pub fn config_load() -> Result<AppConfig, crate::command_error::BackendCommandError> {
    let cfg = config_read().map_err(crate::command_error::BackendCommandError::from)?;
    config_write(&cfg).map_err(crate::command_error::BackendCommandError::from)?;
    Ok(cfg)
}

pub(crate) fn config_read() -> Result<AppConfig, String> {
    let path = config_path();
    if path.exists() {
        let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let cfg: AppConfig = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        let cfg = cfg.migrate();
        crate::ssh::validate_algorithm_config(&cfg.ssh)?;
        validate_shortcut_config(&cfg.shortcuts)?;
        Ok(cfg)
    } else {
        Ok(AppConfig::default())
    }
}

#[tauri::command]
pub fn config_save(
    app: AppHandle,
    terminals: tauri::State<'_, TerminalControlState>,
    config: AppConfig,
) -> Result<(), crate::command_error::BackendCommandError> {
    let config = config.migrate();
    crate::ssh::validate_algorithm_config(&config.ssh)
        .map_err(crate::command_error::BackendCommandError::from)?;
    validate_shortcut_config(&config.shortcuts)
        .map_err(crate::command_error::BackendCommandError::from)?;
    config_write(&config).map_err(crate::command_error::BackendCommandError::from)?;
    terminals.set_output_limit_from_scrollback(config.terminal.scrollback);
    if let Err(error) = app.emit("config://updated", ()) {
        eprintln!("Failed to emit config update: {error}");
    }
    Ok(())
}

fn config_write(config: &AppConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_uses_defaults_and_migrates_version() {
        let cfg: AppConfig =
            serde_json::from_str(r#"{"config_version":4,"language":"ja"}"#).unwrap();
        let cfg = cfg.migrate();

        assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
        assert_eq!(cfg.language, "ja");
        assert!(cfg.updates.check_on_startup);
        assert!(cfg.connection_history.enabled);
        assert_eq!(cfg.ai.default_provider, DEFAULT_AI_PROVIDER);
        assert_eq!(cfg.ai.default_model, DEFAULT_AI_MODEL);
        assert!(!cfg.ai.debug_log_enabled);
        assert!(!cfg.ai.azure_openai_enabled);
        assert!(!cfg.external_control.enabled);
        assert!(!cfg.external_control.connect_enabled);
        assert!(!cfg.external_control.mcp_enabled);
        assert!(!cfg.external_control.cli_enabled);
        assert_eq!(cfg.shortcuts, ShortcutConfig::default());
        assert_eq!(cfg.ai.azure_openai_endpoint, "");
        assert_eq!(cfg.ai.azure_openai_deployment, "");
        assert_eq!(cfg.terminal.font_size, 14);
        assert_eq!(cfg.terminal.scrollback, 10000);
        assert!(!cfg.terminal.auto_session_log);
        assert_eq!(cfg.terminal.log_format, "display");
        assert!(!cfg.terminal.include_log_header);
        assert_eq!(cfg.ssh.algorithm_mode, "default");
        assert_eq!(cfg.ssh.algorithms, SshAlgorithmSelection::default());
        assert!(cfg.saved_connections.is_empty());
    }

    #[test]
    fn update_preference_round_trips_when_disabled() {
        let cfg = serde_json::from_str::<AppConfig>(
            r#"{
                "config_version": 5,
                "updates": {"check_on_startup": false}
            }"#,
        )
        .unwrap()
        .migrate();

        assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
        assert!(!cfg.updates.check_on_startup);
        let value = serde_json::to_value(cfg).unwrap();
        assert_eq!(value["updates"]["check_on_startup"], false);
    }

    #[test]
    fn connection_history_preference_round_trips_when_disabled() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "config_version": 6,
                "connection_history": { "enabled": false }
            }"#,
        )
        .unwrap();

        assert!(!cfg.connection_history.enabled);
        let value = serde_json::to_value(&cfg).unwrap();
        assert_eq!(value["connection_history"]["enabled"], false);
    }

    #[test]
    fn partial_nested_config_uses_field_defaults() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "config_version": 1,
                "ai": {"default_provider": "Ollama"},
                "ssh": {"allow_legacy_algorithms": true},
                "terminal": {"auto_session_log": true},
                "saved_connections": [{"id": "dev box", "connection_type": "ssh"}]
            }"#,
        )
        .unwrap();

        assert_eq!(cfg.ai.default_provider, "Ollama");
        assert!(!cfg.ai.azure_openai_enabled);
        assert_eq!(cfg.ai.azure_openai_endpoint, "");
        assert_eq!(cfg.ai.azure_openai_deployment, "");
        assert_eq!(cfg.ai.ollama_base_url, "http://localhost:11434");
        assert_eq!(cfg.ai.default_model, DEFAULT_AI_MODEL);
        assert!(!cfg.ai.debug_log_enabled);
        assert_eq!(cfg.terminal.font_size, 14);
        assert!(cfg.terminal.auto_session_log);
        assert_eq!(cfg.terminal.log_format, "display");
        assert!(!cfg.terminal.include_log_header);
        assert_eq!(cfg.ssh.algorithm_mode, "custom");
        assert!(cfg
            .ssh
            .algorithms
            .kex
            .contains(&"diffie-hellman-group1-sha1".to_string()));
        let serialized = serde_json::to_value(&cfg).unwrap();
        assert!(serialized["ssh"].get("allow_legacy_algorithms").is_none());
        assert_eq!(serialized["ssh"]["algorithm_mode"], "custom");
        assert_eq!(cfg.saved_connections[0].id, "dev box");
        assert_eq!(cfg.saved_connections[0].encoding, None);
        assert_eq!(cfg.saved_connections[0].terminal_mode, None);
        assert_eq!(cfg.saved_connections[0].auth_method, None);
        assert_eq!(cfg.saved_connections[0].private_key_path, None);
        assert_eq!(cfg.saved_connections[0].jump_profile_id, None);
        assert_eq!(cfg.saved_connections[0].memo, None);
        assert!(cfg.saved_connections[0].external_control_enabled);
    }

    #[test]
    fn legacy_mcp_config_is_migrated_to_external_control() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "config_version": 1,
                "mcp": {
                    "enabled": true,
                    "connect_enabled": true,
                    "stdio_enabled": true,
                    "cli_enabled": false
                }
            }"#,
        )
        .unwrap();

        let cfg = cfg.migrate();

        assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
        assert!(cfg.external_control.enabled);
        assert!(cfg.external_control.connect_enabled);
        assert!(cfg.external_control.mcp_enabled);
        assert!(!cfg.external_control.cli_enabled);
    }

    #[test]
    fn new_external_control_config_takes_priority_over_legacy_mcp() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "config_version": 1,
                "mcp": {
                    "enabled": false,
                    "connect_enabled": false,
                    "stdio_enabled": true,
                    "cli_enabled": false
                },
                "external_control": {
                    "enabled": true,
                    "cli_enabled": true
                }
            }"#,
        )
        .unwrap();

        assert!(cfg.external_control.enabled);
        assert!(!cfg.external_control.connect_enabled);
        assert!(cfg.external_control.mcp_enabled);
        assert!(cfg.external_control.cli_enabled);
    }

    #[test]
    fn saved_connection_preserves_encoding() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "legacy-router",
                    "connection_type": "ssh",
                    "encoding": "shift-jis"
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(
            cfg.saved_connections[0].encoding.as_deref(),
            Some("shift-jis")
        );
    }

    #[test]
    fn saved_connection_preserves_terminal_mode() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "ios-router",
                    "connection_type": "ssh",
                    "terminal_mode": "cisco_ios"
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(
            cfg.saved_connections[0].terminal_mode.as_deref(),
            Some("cisco_ios")
        );
    }

    #[test]
    fn saved_connection_preserves_telnet_profile_fields() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "legacy-telnet",
                    "connection_type": "telnet",
                    "host": "192.168.1.10",
                    "port": 23,
                    "encoding": "euc-jp"
                }]
            }"#,
        )
        .unwrap();

        let profile = &cfg.saved_connections[0];
        assert_eq!(profile.id, "legacy-telnet");
        assert_eq!(profile.connection_type, "telnet");
        assert_eq!(profile.host.as_deref(), Some("192.168.1.10"));
        assert_eq!(profile.port, Some(23));
        assert_eq!(profile.encoding.as_deref(), Some("euc-jp"));
    }

    #[test]
    fn saved_connection_preserves_public_key_auth_path() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "key-router",
                    "connection_type": "ssh",
                    "auth_method": "public_key",
                    "private_key_path": "C:\\Users\\me\\.ssh\\id_ed25519"
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(
            cfg.saved_connections[0].auth_method.as_deref(),
            Some("public_key")
        );
        assert_eq!(
            cfg.saved_connections[0].private_key_path.as_deref(),
            Some("C:\\Users\\me\\.ssh\\id_ed25519")
        );
    }

    #[test]
    fn saved_connection_preserves_jump_profile_id() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "inside",
                    "connection_type": "ssh",
                    "jump_profile_id": "bastion"
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(
            cfg.saved_connections[0].jump_profile_id.as_deref(),
            Some("bastion")
        );
    }

    #[test]
    fn saved_connection_preserves_memo() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "memo": "Cisco ISR at branch A"
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(
            cfg.saved_connections[0].memo.as_deref(),
            Some("Cisco ISR at branch A")
        );
    }

    #[test]
    fn saved_connection_defaults_external_control_enabled_to_true() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh"
                }]
            }"#,
        )
        .unwrap();

        assert!(cfg.saved_connections[0].external_control_enabled);
    }

    #[test]
    fn saved_connection_migrates_legacy_mcp_enabled_false() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "mcp_enabled": false
                }]
            }"#,
        )
        .unwrap();

        assert!(!cfg.saved_connections[0].external_control_enabled);
    }

    #[test]
    fn saved_connection_new_flag_takes_priority_over_legacy_flag() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "external_control_enabled": true,
                    "mcp_enabled": false
                }]
            }"#,
        )
        .unwrap();

        assert!(cfg.saved_connections[0].external_control_enabled);
    }

    #[test]
    fn serialization_omits_legacy_mcp_fields() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "config_version": 1,
                "mcp": {
                    "enabled": true,
                    "connect_enabled": true,
                    "stdio_enabled": true,
                    "cli_enabled": true
                },
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "mcp_enabled": false
                }]
            }"#,
        )
        .unwrap();

        let cfg = cfg.migrate();
        let value = serde_json::to_value(&cfg).unwrap();

        assert_eq!(value["config_version"], CURRENT_CONFIG_VERSION);
        assert!(value.get("mcp").is_none());
        assert_eq!(value["external_control"]["mcp_enabled"], true);
        assert!(value["saved_connections"][0].get("mcp_enabled").is_none());
        assert_eq!(
            value["saved_connections"][0]["external_control_enabled"],
            false
        );
    }

    #[test]
    fn default_config_uses_system_language() {
        let cfg = AppConfig::default();

        assert_eq!(cfg.language, "system");
        assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
    }

    #[test]
    fn shortcut_config_preserves_explicit_null_and_defaults_missing_actions() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "shortcuts": {
                    "new_connection": null,
                    "new_window": {"key": "F2"},
                    "terminal_copy": null,
                    "terminal_log_start_append": null,
                    "new_tab": {"key": "t", "ctrl": true}
                }
            }"#,
        )
        .unwrap();

        assert_eq!(cfg.shortcuts.new_connection, None);
        assert_eq!(
            cfg.shortcuts.new_window,
            Some(ShortcutBinding {
                key: "F2".into(),
                ctrl: false,
                alt: false,
                shift: false,
            })
        );
        assert_eq!(
            cfg.shortcuts.open_settings,
            default_open_settings_shortcut()
        );
        assert_eq!(cfg.shortcuts.exit, None);
        assert_eq!(
            cfg.shortcuts.terminal_select_all,
            default_terminal_select_all_shortcut()
        );
        assert_eq!(cfg.shortcuts.terminal_copy, None);
        assert_eq!(
            cfg.shortcuts.terminal_paste,
            default_terminal_paste_shortcut()
        );
        assert_eq!(
            cfg.shortcuts.terminal_log_start_overwrite,
            default_terminal_log_start_overwrite_shortcut()
        );
        assert_eq!(cfg.shortcuts.terminal_log_start_append, None);
        assert_eq!(
            cfg.shortcuts.terminal_log_stop,
            default_terminal_log_stop_shortcut()
        );
        assert_eq!(cfg.shortcuts.terminal_log_pause, None);
        assert_eq!(cfg.shortcuts.terminal_log_resume, None);
        let serialized = serde_json::to_value(cfg).unwrap();
        assert!(serialized["shortcuts"].get("new_tab").is_none());
    }

    #[test]
    fn shortcut_config_normalizes_keys_during_migration() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "shortcuts": {
                    "new_connection": {"key": "N", "ctrl": true},
                    "new_window": {"key": "f2"},
                    "open_settings": null,
                    "exit": {"key": "Q", "ctrl": true, "shift": true},
                    "terminal_select_all": {"key": "A", "ctrl": true, "shift": true},
                    "terminal_log_stop": {"key": "f10", "ctrl": true, "shift": true}
                }
            }"#,
        )
        .unwrap();
        let cfg = cfg.migrate();

        assert_eq!(cfg.shortcuts.new_connection.as_ref().unwrap().key, "n");
        assert_eq!(cfg.shortcuts.new_window.as_ref().unwrap().key, "F2");
        assert_eq!(cfg.shortcuts.exit.as_ref().unwrap().key, "q");
        assert_eq!(cfg.shortcuts.terminal_select_all.as_ref().unwrap().key, "a");
        assert_eq!(cfg.shortcuts.terminal_log_stop.as_ref().unwrap().key, "F10");
        assert!(validate_shortcut_config(&cfg.shortcuts).is_ok());
    }

    #[test]
    fn shortcut_config_preserves_explicit_null_exit() {
        let cfg: AppConfig = serde_json::from_str(r#"{"shortcuts":{"exit":null}}"#).unwrap();

        assert_eq!(cfg.shortcuts.exit, None);
        assert!(serde_json::to_value(cfg).unwrap()["shortcuts"]["exit"].is_null());
    }

    #[test]
    fn shortcut_config_rejects_duplicate_assignments() {
        let duplicate = Some(ShortcutBinding {
            key: "n".into(),
            ctrl: true,
            alt: false,
            shift: false,
        });
        let shortcuts = ShortcutConfig {
            new_connection: duplicate.clone(),
            new_window: None,
            open_settings: None,
            exit: None,
            terminal_select_all: None,
            terminal_copy: duplicate,
            terminal_paste: None,
            terminal_log_start_overwrite: None,
            terminal_log_start_append: None,
            terminal_log_stop: None,
            terminal_log_pause: None,
            terminal_log_resume: None,
        };

        assert_eq!(
            validate_shortcut_config(&shortcuts),
            Err("Shortcut assignments must be unique".into())
        );
    }

    #[test]
    fn shortcut_config_validates_modifier_and_function_key_rules() {
        let mut shortcuts = ShortcutConfig {
            new_connection: Some(ShortcutBinding {
                key: "x".into(),
                ctrl: false,
                alt: false,
                shift: false,
            }),
            new_window: None,
            open_settings: None,
            exit: None,
            terminal_select_all: None,
            terminal_copy: None,
            terminal_paste: None,
            terminal_log_start_overwrite: None,
            terminal_log_start_append: None,
            terminal_log_stop: None,
            terminal_log_pause: None,
            terminal_log_resume: None,
        };
        assert!(validate_shortcut_config(&shortcuts).is_err());

        shortcuts.new_connection = Some(ShortcutBinding {
            key: "F12".into(),
            ctrl: false,
            alt: false,
            shift: false,
        });
        assert!(validate_shortcut_config(&shortcuts).is_ok());

        shortcuts.new_connection = Some(ShortcutBinding {
            key: "F4".into(),
            ctrl: false,
            alt: true,
            shift: false,
        });
        assert!(validate_shortcut_config(&shortcuts).is_err());
    }
}
