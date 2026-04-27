mod catalog;
mod errors;
mod providers;
mod secrets;
mod types;

pub use catalog::{DEFAULT_AI_MODEL, DEFAULT_AI_PROVIDER};
pub use types::{AiModelInfo, AiProvider, AiSecretStatus, ChatMessage};

use catalog::{fallback_cloud_models, fallback_models_for};
use providers::{fetch_provider_models, send_chat_request};
use secrets::{
    is_secret_present, load_provider_secret_optional, provider_secret_key, KEY_ANTHROPIC,
    KEY_AZURE_OPENAI, KEY_GEMINI, KEY_OPENAI,
};

#[tauri::command]
pub async fn ai_get_models() -> Result<Vec<AiModelInfo>, String> {
    let client = reqwest::Client::new();
    let mut models = Vec::new();

    for provider in [
        AiProvider::OpenAi,
        AiProvider::Anthropic,
        AiProvider::Gemini,
    ] {
        let provider_models = match load_provider_secret_optional(&provider) {
            Ok(Some(api_key)) => {
                fetch_provider_models(&client, &provider, Some(&api_key), None, "en")
                    .await
                    .unwrap_or_else(|_| fallback_models_for(&provider))
            }
            Ok(None) | Err(_) => fallback_models_for(&provider),
        };
        models.extend(provider_models);
    }

    if models.is_empty() {
        Ok(fallback_cloud_models())
    } else {
        Ok(models)
    }
}

#[tauri::command]
pub async fn ai_get_ollama_models(base_url: String) -> Result<Vec<AiModelInfo>, String> {
    let client = reqwest::Client::new();
    fetch_provider_models(&client, &AiProvider::Ollama, None, Some(&base_url), "en").await
}

fn build_system_prompt(terminal_context: &Option<String>, language: &str) -> String {
    let base = if language == "ja" {
        "あなたはExaTermのAIアシスタントです。ネットワーク操作を支援します。日本語で回答してください。"
    } else {
        "You are the AI assistant for ExaTerm, a network terminal. Help the user with their network operations. Please respond in English."
    };
    let mut s = base.to_string();
    if let Some(ctx) = terminal_context {
        let ctx_label = if language == "ja" {
            "【ターミナル出力】"
        } else {
            "[Terminal Output]"
        };
        s.push_str(&format!("\n\n{}\n```\n{}\n```", ctx_label, ctx));
    }
    s
}

#[tauri::command]
pub fn ai_secret_status() -> Result<AiSecretStatus, String> {
    Ok(AiSecretStatus {
        openai: is_secret_present(KEY_OPENAI)?,
        azure_openai: is_secret_present(KEY_AZURE_OPENAI)?,
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

    secrets::set_secret(key_name, value.trim())
}

#[tauri::command]
pub fn ai_secret_clear(provider: String) -> Result<(), String> {
    let key_name = provider_secret_key(provider.trim())
        .ok_or_else(|| "Unsupported provider for secure secret storage".to_string())?;

    secrets::clear_secret(key_name)
}

#[tauri::command]
pub async fn ai_chat(
    provider: AiProvider,
    model: String,
    messages: Vec<ChatMessage>,
    terminal_context: Option<String>,
    language: String,
    ollama_base_url: Option<String>,
    azure_openai_endpoint: Option<String>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let system_prompt = build_system_prompt(&terminal_context, &language);

    send_chat_request(
        &client,
        provider,
        &model,
        &messages,
        &system_prompt,
        &language,
        ollama_base_url.as_deref(),
        azure_openai_endpoint.as_deref(),
    )
    .await
}
