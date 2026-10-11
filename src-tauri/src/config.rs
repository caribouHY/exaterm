use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Emitter};

use crate::ai::{DEFAULT_AI_MODEL, DEFAULT_AI_PROVIDER};
use crate::terminal_control::TerminalControlState;

const CURRENT_CONFIG_VERSION: u32 = 7;
static CONFIG_FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
    #[serde(default)]
    pub terminal_clear_viewport: Option<ShortcutBinding>,
    #[serde(default)]
    pub terminal_clear_buffer: Option<ShortcutBinding>,
    #[serde(default = "default_terminal_mode_menu_shortcut")]
    pub terminal_mode_menu: Option<ShortcutBinding>,
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

fn default_terminal_mode_menu_shortcut() -> Option<ShortcutBinding> {
    shortcut("F8", true, false, true)
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
            terminal_clear_viewport: None,
            terminal_clear_buffer: None,
            terminal_mode_menu: default_terminal_mode_menu_shortcut(),
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
            &mut self.terminal_clear_viewport,
            &mut self.terminal_clear_buffer,
            &mut self.terminal_mode_menu,
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
            "terminal_clear_viewport",
            &shortcuts.terminal_clear_viewport,
        ),
        ("terminal_clear_buffer", &shortcuts.terminal_clear_buffer),
        ("terminal_mode_menu", &shortcuts.terminal_mode_menu),
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ExternalControlConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub connect_enabled: bool,
    #[serde(default)]
    pub direct_connect_enabled: bool,
    #[serde(default)]
    pub mcp_enabled: bool,
    #[serde(default)]
    pub cli_enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ExternalControlConfigInput {
    enabled: Option<bool>,
    connect_enabled: Option<bool>,
    direct_connect_enabled: Option<bool>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
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

fn default_ssh_algorithm_mode() -> String {
    "default".into()
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SshConfigInput {
    algorithm_mode: Option<String>,
    algorithms: Option<SshAlgorithmSelection>,
    allow_legacy_algorithms: Option<bool>,
    default_private_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SshConfig {
    pub algorithm_mode: String,
    pub algorithms: SshAlgorithmSelection,
    pub default_private_key_path: String,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            algorithm_mode: default_ssh_algorithm_mode(),
            algorithms: SshAlgorithmSelection::default(),
            default_private_key_path: String::new(),
        }
    }
}

impl<'de> Deserialize<'de> for SshConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = SshConfigInput::deserialize(deserializer)?;
        let default_private_key_path = input.default_private_key_path.unwrap_or_default();
        if let Some(algorithm_mode) = input.algorithm_mode {
            return Ok(Self {
                algorithm_mode,
                algorithms: input.algorithms.unwrap_or_default(),
                default_private_key_path,
            });
        }

        if input.allow_legacy_algorithms.unwrap_or(false) {
            return Ok(Self {
                algorithm_mode: "custom".into(),
                algorithms: crate::ssh::legacy_algorithm_selection(),
                default_private_key_path,
            });
        }

        Ok(Self {
            default_private_key_path,
            ..Self::default()
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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
                direct_connect_enabled: external_control.direct_connect_enabled.unwrap_or(false),
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

fn merge_edited_config(
    base: &AppConfig,
    edited: &AppConfig,
    latest: &AppConfig,
) -> Result<AppConfig, String> {
    let base = serde_json::to_value(base).map_err(|error| error.to_string())?;
    let edited = serde_json::to_value(edited).map_err(|error| error.to_string())?;
    let mut merged = serde_json::to_value(latest).map_err(|error| error.to_string())?;
    merge_changed_values(&base, &edited, &mut merged, &mut Vec::new());
    serde_json::from_value::<AppConfig>(merged)
        .map(AppConfig::migrate)
        .map_err(|error| error.to_string())
}

fn merge_changed_values(
    base: &serde_json::Value,
    edited: &serde_json::Value,
    latest: &mut serde_json::Value,
    path: &mut Vec<String>,
) {
    if base == edited {
        return;
    }

    if let (
        serde_json::Value::Object(base),
        serde_json::Value::Object(edited),
        serde_json::Value::Object(latest),
    ) = (base, edited, &mut *latest)
    {
        if !is_atomic_config_path(path) {
            for (key, edited_value) in edited {
                if path.is_empty() && matches!(key.as_str(), "config_version" | "saved_connections")
                {
                    continue;
                }
                let Some(base_value) = base.get(key) else {
                    continue;
                };
                let Some(latest_value) = latest.get_mut(key) else {
                    continue;
                };
                path.push(key.clone());
                merge_changed_values(base_value, edited_value, latest_value, path);
                path.pop();
            }
            return;
        }
    }

    *latest = edited.clone();
}

fn is_atomic_config_path(path: &[String]) -> bool {
    path.len() == 2 && path[0] == "shortcuts"
}

fn lock_config_file() -> Result<MutexGuard<'static, ()>, String> {
    CONFIG_FILE_LOCK
        .lock()
        .map_err(|_| "The configuration file lock is unavailable".to_string())
}

fn config_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join("config.json")
}

#[tauri::command]
pub fn config_load() -> Result<AppConfig, crate::command_error::BackendCommandError> {
    let _guard = lock_config_file().map_err(crate::command_error::BackendCommandError::from)?;
    config_load_from_path(&config_path()).map_err(crate::command_error::BackendCommandError::from)
}

pub(crate) fn config_read() -> Result<AppConfig, String> {
    let _guard = lock_config_file()?;
    config_read_from_path(&config_path()).map(|(config, _)| config)
}

fn config_read_from_path(path: &Path) -> Result<(AppConfig, bool), String> {
    if path.exists() {
        let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let stored: serde_json::Value = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        let cfg: AppConfig = serde_json::from_value(stored.clone()).map_err(|e| e.to_string())?;
        let cfg = cfg.migrate();
        crate::ssh::validate_algorithm_config(&cfg.ssh)?;
        validate_shortcut_config(&cfg.shortcuts)?;
        let normalized = serde_json::to_value(&cfg).map_err(|e| e.to_string())?;
        Ok((cfg, stored != normalized))
    } else {
        Ok((AppConfig::default(), true))
    }
}

fn config_load_from_path(path: &Path) -> Result<AppConfig, String> {
    let (config, should_write) = config_read_from_path(path)?;
    if should_write {
        config_write_to_path(path, &config)?;
    }
    Ok(config)
}

#[tauri::command]
pub fn config_save(
    app: AppHandle,
    terminals: tauri::State<'_, TerminalControlState>,
    base_config: AppConfig,
    edited_config: AppConfig,
) -> Result<AppConfig, crate::command_error::BackendCommandError> {
    let (config, changed) = {
        let _guard = lock_config_file().map_err(crate::command_error::BackendCommandError::from)?;
        let (latest, should_write) = config_read_from_path(&config_path())
            .map_err(crate::command_error::BackendCommandError::from)?;
        let config = merge_edited_config(&base_config.migrate(), &edited_config.migrate(), &latest)
            .map_err(crate::command_error::BackendCommandError::from)?;
        validate_config(&config)?;
        let changed = should_write || config != latest;
        if changed {
            config_write_to_path(&config_path(), &config)
                .map_err(crate::command_error::BackendCommandError::from)?;
        }
        (config, changed)
    };

    if changed {
        terminals.set_output_limit_from_scrollback(config.terminal.scrollback);
        emit_config_updated(&app);
    }
    Ok(config)
}

#[tauri::command]
pub fn config_saved_connection_upsert(
    app: AppHandle,
    previous_id: Option<String>,
    profile: SavedConnection,
) -> Result<AppConfig, crate::command_error::BackendCommandError> {
    validate_saved_connection_identity(&profile)?;

    let (config, changed) = {
        let _guard = lock_config_file().map_err(crate::command_error::BackendCommandError::from)?;
        let (mut config, should_write) = config_read_from_path(&config_path())
            .map_err(crate::command_error::BackendCommandError::from)?;

        let changed = upsert_saved_connection(&mut config, previous_id.as_deref(), profile)?;
        let changed = should_write || changed;
        if changed {
            validate_config(&config)?;
            config_write_to_path(&config_path(), &config)
                .map_err(crate::command_error::BackendCommandError::from)?;
        }
        (config, changed)
    };

    if changed {
        emit_config_updated(&app);
    }
    Ok(config)
}

#[tauri::command]
pub fn config_saved_connection_delete(
    app: AppHandle,
    connection_type: String,
    id: String,
) -> Result<AppConfig, crate::command_error::BackendCommandError> {
    validate_saved_connection_key(&connection_type, &id)?;

    let config = {
        let _guard = lock_config_file().map_err(crate::command_error::BackendCommandError::from)?;
        let (mut config, _) = config_read_from_path(&config_path())
            .map_err(crate::command_error::BackendCommandError::from)?;
        delete_saved_connection(&mut config, &connection_type, &id)?;
        config_write_to_path(&config_path(), &config)
            .map_err(crate::command_error::BackendCommandError::from)?;
        config
    };

    emit_config_updated(&app);
    Ok(config)
}

fn validate_saved_connection_identity(
    profile: &SavedConnection,
) -> Result<(), crate::command_error::BackendCommandError> {
    validate_saved_connection_key(&profile.connection_type, &profile.id)?;
    if profile.connection_type == "ssh"
        && profile.jump_profile_id.as_deref() == Some(profile.id.as_str())
    {
        return Err(crate::command_error::BackendCommandError::new(
            "ssh.jump_profile_self_reference",
            "An SSH jump profile cannot reference itself",
        ));
    }
    Ok(())
}

fn upsert_saved_connection(
    config: &mut AppConfig,
    previous_id: Option<&str>,
    profile: SavedConnection,
) -> Result<bool, crate::command_error::BackendCommandError> {
    if config.saved_connections.iter().any(|entry| {
        entry.connection_type == profile.connection_type
            && entry.id == profile.id
            && previous_id != Some(entry.id.as_str())
    }) {
        return Err(crate::command_error::BackendCommandError::new(
            "config.saved_connection_duplicate",
            "A saved connection profile with this name already exists",
        ));
    }

    if let Some(previous_id) = previous_id {
        let Some(existing) = config.saved_connections.iter_mut().find(|entry| {
            entry.connection_type == profile.connection_type && entry.id == previous_id
        }) else {
            return Err(crate::command_error::BackendCommandError::new(
                "config.saved_connection_not_found",
                "The saved connection profile no longer exists",
            ));
        };
        if existing == &profile {
            return Ok(false);
        }
        *existing = profile;
    } else {
        config.saved_connections.push(profile);
    }
    Ok(true)
}

fn delete_saved_connection(
    config: &mut AppConfig,
    connection_type: &str,
    id: &str,
) -> Result<(), crate::command_error::BackendCommandError> {
    let Some(index) = config
        .saved_connections
        .iter()
        .position(|entry| entry.connection_type == connection_type && entry.id == id)
    else {
        return Err(crate::command_error::BackendCommandError::new(
            "config.saved_connection_not_found",
            "The saved connection profile no longer exists",
        ));
    };
    config.saved_connections.remove(index);
    Ok(())
}

fn validate_saved_connection_key(
    connection_type: &str,
    id: &str,
) -> Result<(), crate::command_error::BackendCommandError> {
    if id.trim().is_empty() {
        return Err(crate::command_error::BackendCommandError::new(
            "config.saved_connection_name_required",
            "Enter a profile name",
        ));
    }
    if !matches!(connection_type, "ssh" | "telnet") {
        return Err(crate::command_error::BackendCommandError::new(
            "connection.unknown_type",
            format!("Unknown connection type: {connection_type}"),
        )
        .with_param("detail", connection_type));
    }
    Ok(())
}

fn emit_config_updated(app: &AppHandle) {
    if let Err(error) = app.emit("config://updated", ()) {
        eprintln!("Failed to emit config update: {error}");
    }
}

fn validate_config(config: &AppConfig) -> Result<(), crate::command_error::BackendCommandError> {
    crate::ssh::validate_algorithm_config(&config.ssh)
        .map_err(crate::command_error::BackendCommandError::from)?;
    validate_shortcut_config(&config.shortcuts)
        .map_err(crate::command_error::BackendCommandError::from)?;
    Ok(())
}

fn config_write_to_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
