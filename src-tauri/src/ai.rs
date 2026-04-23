use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

const KEYRING_SERVICE: &str = "com.caribouhy.exaterm";
const KEY_OPENAI: &str = "openai_api_key";
const KEY_ANTHROPIC: &str = "anthropic_api_key";
const KEY_GEMINI: &str = "gemini_api_key";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiProvider {
    OpenAi,
    Anthropic,
    Gemini,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModelInfo {
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSecretStatus {
    pub openai: bool,
    pub anthropic: bool,
    pub gemini: bool,
}

#[tauri::command]
pub fn ai_get_models() -> Vec<AiModelInfo> {
    vec![
        AiModelInfo { provider: "OpenAi".into(), model_id: "gpt-4o".into(), display_name: "GPT-4o".into() },
        AiModelInfo { provider: "OpenAi".into(), model_id: "gpt-4o-mini".into(), display_name: "GPT-4o Mini".into() },
        AiModelInfo { provider: "Anthropic".into(), model_id: "claude-sonnet-4-20250514".into(), display_name: "Claude Sonnet 4".into() },
        AiModelInfo { provider: "Anthropic".into(), model_id: "claude-3-5-haiku-20241022".into(), display_name: "Claude 3.5 Haiku".into() },
        AiModelInfo { provider: "Gemini".into(), model_id: "gemini-2.5-pro".into(), display_name: "Gemini 2.5 Pro".into() },
        AiModelInfo { provider: "Gemini".into(), model_id: "gemini-2.5-flash".into(), display_name: "Gemini 2.5 Flash".into() },
    ]
}

#[tauri::command]
pub async fn ai_get_ollama_models(base_url: String) -> Result<Vec<AiModelInfo>, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));

    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let mut models = Vec::new();
    if let Some(models_arr) = body["models"].as_array() {
        for m in models_arr {
            if let Some(name) = m["name"].as_str() {
                models.push(AiModelInfo {
                    provider: "Ollama".into(),
                    model_id: name.into(),
                    display_name: name.into(),
                });
            }
        }
    }

    Ok(models)
}

fn build_system_prompt(terminal_context: &Option<String>, language: &str) -> String {
    let base = if language == "ja" {
        "あなたはExaTermのAIアシスタントです。ネットワーク操作を支援します。日本語で回答してください。"
    } else {
        "You are the AI assistant for ExaTerm, a network terminal. Help the user with their network operations. Please respond in English."
    };
    let mut s = base.to_string();
    if let Some(ctx) = terminal_context {
        let ctx_label = if language == "ja" { "【ターミナル出力】" } else { "[Terminal Output]" };
        s.push_str(&format!("\n\n{}\n```\n{}\n```", ctx_label, ctx));
    }
    s
}

fn keyring_entry(key_name: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, key_name).map_err(|e| e.to_string())
}

fn provider_secret_key(provider: &str) -> Option<&'static str> {
    match provider {
        "OpenAi" => Some(KEY_OPENAI),
        "Anthropic" => Some(KEY_ANTHROPIC),
        "Gemini" => Some(KEY_GEMINI),
        _ => None,
    }
}

fn is_secret_present(key_name: &str) -> Result<bool, String> {
    let entry = keyring_entry(key_name)?;
    match entry.get_password() {
        Ok(secret) => Ok(!secret.is_empty()),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

fn load_provider_secret(provider: &AiProvider) -> Result<String, String> {
    let key_name = match provider {
        AiProvider::OpenAi => KEY_OPENAI,
        AiProvider::Anthropic => KEY_ANTHROPIC,
        AiProvider::Gemini => KEY_GEMINI,
        AiProvider::Ollama => return Err("Ollama does not use API keys".into()),
    };

    let entry = keyring_entry(key_name)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok(secret),
        Ok(_) => Err("API key is empty. Please save it in Settings.".into()),
        Err(KeyringError::NoEntry) => Err("API key not found. Please save it in Settings.".into()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn ai_secret_status() -> Result<AiSecretStatus, String> {
    Ok(AiSecretStatus {
        openai: is_secret_present(KEY_OPENAI)?,
        anthropic: is_secret_present(KEY_ANTHROPIC)?,
        gemini: is_secret_present(KEY_GEMINI)?,
    })
}

#[tauri::command]
pub fn ai_secret_set(provider: String, value: String) -> Result<(), String> {
    let key_name = provider_secret_key(provider.trim())
        .ok_or_else(|| "Unsupported provider for secure secret storage".to_string())?;

    if value.trim().is_empty() {
        return Err("Secret value cannot be empty".into());
    }

    let entry = keyring_entry(key_name)?;
    entry.set_password(value.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_secret_clear(provider: String) -> Result<(), String> {
    let key_name = provider_secret_key(provider.trim())
        .ok_or_else(|| "Unsupported provider for secure secret storage".to_string())?;

    let entry = keyring_entry(key_name)?;
    match entry.delete_credential() {
        Ok(_) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn ai_chat(
    provider: AiProvider,
    model: String,
    messages: Vec<ChatMessage>,
    terminal_context: Option<String>,
    language: String,
    ollama_base_url: Option<String>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let sys = build_system_prompt(&terminal_context, &language);

    match provider {
        AiProvider::OpenAi => {
            let api_key = load_provider_secret(&AiProvider::OpenAi)?;
            let mut msgs = vec![serde_json::json!({"role":"system","content":sys})];
            for m in &messages { msgs.push(serde_json::json!({"role":&m.role,"content":&m.content})); }
            let resp = client.post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&serde_json::json!({"model":model,"messages":msgs,"temperature":0.7}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["choices"][0]["message"]["content"].as_str().map(|s| s.to_string()).ok_or_else(|| format!("応答エラー: {}", body))
        }
        AiProvider::Anthropic => {
            let api_key = load_provider_secret(&AiProvider::Anthropic)?;
            let mut msgs = Vec::new();
            for m in &messages { msgs.push(serde_json::json!({"role":&m.role,"content":&m.content})); }
            let resp = client.post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &api_key).header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&serde_json::json!({"model":model,"max_tokens":4096,"system":sys,"messages":msgs}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["content"][0]["text"].as_str().map(|s| s.to_string()).ok_or_else(|| format!("応答エラー: {}", body))
        }
        AiProvider::Gemini => {
            let api_key = load_provider_secret(&AiProvider::Gemini)?;
            let mut parts = vec![serde_json::json!({"text":sys})];
            for m in &messages { parts.push(serde_json::json!({"text":format!("{}:{}",m.role,m.content)})); }
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);
            let resp = client.post(&url)
                .json(&serde_json::json!({"contents":[{"parts":parts}],"generationConfig":{"temperature":0.7,"maxOutputTokens":4096}}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["candidates"][0]["content"]["parts"][0]["text"].as_str().map(|s| s.to_string()).ok_or_else(|| format!("応答エラー: {}", body))
        }
        AiProvider::Ollama => {
            let base_url = ollama_base_url
                .unwrap_or_else(|| "http://localhost:11434".to_string())
                .trim_end_matches('/')
                .to_string();
            let mut msgs = vec![serde_json::json!({"role":"system","content":sys})];
            for m in &messages { msgs.push(serde_json::json!({"role":&m.role,"content":&m.content})); }
            let url = format!("{}/api/chat", base_url);
            let resp = client.post(&url)
                .json(&serde_json::json!({"model":model,"messages":msgs,"stream":false}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["message"]["content"].as_str().map(|s| s.to_string()).ok_or_else(|| format!("応答エラー: {}", body))
        }
    }
}

#[tauri::command]
pub async fn ai_chat_stream(
    app: AppHandle,
    provider: AiProvider,
    model: String,
    messages: Vec<ChatMessage>,
    terminal_context: Option<String>,
    request_id: String,
    language: String,
    ollama_base_url: Option<String>,
) -> Result<(), String> {
    let result = ai_chat(
        provider,
        model,
        messages,
        terminal_context,
        language,
        ollama_base_url,
    )
    .await?;
    let _ = app.emit(&format!("ai://chunk/{}", request_id), &result);
    let _ = app.emit(&format!("ai://done/{}", request_id), "");
    Ok(())
}
