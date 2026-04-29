use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::ai::{DEFAULT_AI_MODEL, DEFAULT_AI_PROVIDER};

const CURRENT_CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub config_version: u32,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub terminal: TerminalConfig,
    #[serde(default)]
    pub ssh: SshConfig,
    #[serde(default)]
    pub saved_connections: Vec<SavedConnection>,
}

fn default_language() -> String {
    "en".into()
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    #[serde(default)]
    pub allow_legacy_algorithms: bool,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            allow_legacy_algorithms: false,
        }
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
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            font_size: default_terminal_font_size(),
            font_family: default_terminal_font_family(),
            cursor_style: default_terminal_cursor_style(),
            scrollback: default_terminal_scrollback(),
            auto_session_log: false,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub connection_type: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub serial_port: Option<String>,
    #[serde(default)]
    pub baud_rate: Option<u32>,
}

impl Default for SavedConnection {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            connection_type: String::new(),
            host: None,
            port: None,
            username: None,
            serial_port: None,
            baud_rate: None,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            language: default_language(),
            ai: AiConfig::default(),
            terminal: TerminalConfig::default(),
            ssh: SshConfig::default(),
            saved_connections: Vec::new(),
        }
    }
}

impl AppConfig {
    fn migrate(mut self) -> Self {
        if self.config_version < CURRENT_CONFIG_VERSION {
            self.config_version = CURRENT_CONFIG_VERSION;
        }
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
pub fn config_load() -> Result<AppConfig, String> {
    let path = config_path();
    if path.exists() {
        let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let cfg: AppConfig = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        let cfg = cfg.migrate();
        config_save(cfg.clone())?;
        Ok(cfg)
    } else {
        let cfg = AppConfig::default();
        config_save(cfg.clone())?;
        Ok(cfg)
    }
}

#[tauri::command]
pub fn config_save(config: AppConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_uses_defaults_and_migrates_version() {
        let cfg: AppConfig = serde_json::from_str(r#"{"language":"ja"}"#).unwrap();
        let cfg = cfg.migrate();

        assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
        assert_eq!(cfg.language, "ja");
        assert_eq!(cfg.ai.default_provider, DEFAULT_AI_PROVIDER);
        assert_eq!(cfg.ai.default_model, DEFAULT_AI_MODEL);
        assert!(!cfg.ai.azure_openai_enabled);
        assert_eq!(cfg.ai.azure_openai_endpoint, "");
        assert_eq!(cfg.ai.azure_openai_deployment, "");
        assert_eq!(cfg.terminal.font_size, 14);
        assert_eq!(cfg.terminal.scrollback, 10000);
        assert!(!cfg.ssh.allow_legacy_algorithms);
        assert!(cfg.saved_connections.is_empty());
    }

    #[test]
    fn partial_nested_config_uses_field_defaults() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{
                "config_version": 1,
                "ai": {"default_provider": "Ollama"},
                "ssh": {"allow_legacy_algorithms": true},
                "terminal": {"auto_session_log": true},
                "saved_connections": [{"name": "dev box"}]
            }"#,
        )
        .unwrap();

        assert_eq!(cfg.ai.default_provider, "Ollama");
        assert!(!cfg.ai.azure_openai_enabled);
        assert_eq!(cfg.ai.azure_openai_endpoint, "");
        assert_eq!(cfg.ai.azure_openai_deployment, "");
        assert_eq!(cfg.ai.ollama_base_url, "http://localhost:11434");
        assert_eq!(cfg.ai.default_model, DEFAULT_AI_MODEL);
        assert_eq!(cfg.terminal.font_size, 14);
        assert!(cfg.terminal.auto_session_log);
        assert!(cfg.ssh.allow_legacy_algorithms);
        assert_eq!(cfg.saved_connections[0].name, "dev box");
        assert_eq!(cfg.saved_connections[0].id, "");
    }
}
