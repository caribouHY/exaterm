use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_language")]
    pub language: String,
    pub ai: AiConfig,
    pub terminal: TerminalConfig,
    pub saved_connections: Vec<SavedConnection>,
}

fn default_language() -> String { "en".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub openai_api_key: String,
    pub anthropic_api_key: String,
    pub gemini_api_key: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_base_url: String,
    pub default_provider: String,
    pub default_model: String,
}

fn default_ollama_url() -> String { "http://localhost:11434".into() }


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    pub font_size: u32,
    pub font_family: String,
    pub cursor_style: String,
    pub scrollback: u32,
    #[serde(default = "default_true")]
    pub auto_session_log: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedConnection {
    pub id: String,
    pub name: String,
    pub connection_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub serial_port: Option<String>,
    pub baud_rate: Option<u32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            ai: AiConfig {
                openai_api_key: String::new(), anthropic_api_key: String::new(),
                gemini_api_key: String::new(), ollama_base_url: default_ollama_url(),
                default_provider: "OpenAi".into(),
                default_model: "gpt-4o".into(),
            },
            terminal: TerminalConfig {
                font_size: 14, font_family: "Consolas, 'Courier New', monospace".into(),
                cursor_style: "block".into(), scrollback: 10000,
                auto_session_log: true,
            },
            saved_connections: Vec::new(),
        }
    }
}

fn config_path() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("ExaTerm").join("config.json")
}

#[tauri::command]
pub fn config_load() -> Result<AppConfig, String> {
    let path = config_path();
    if path.exists() {
        let data = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).map_err(|e| e.to_string())
    } else {
        let cfg = AppConfig::default();
        config_save(cfg.clone())?;
        Ok(cfg)
    }
}

#[tauri::command]
pub fn config_save(config: AppConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() { let _ = fs::create_dir_all(parent); }
    let data = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&path, data).map_err(|e| e.to_string())
}
