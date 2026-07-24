mod catalog;
mod debug_log;
mod errors;
mod model_cache;
mod providers;
mod secrets;
mod types;

pub use catalog::{DEFAULT_AI_MODEL, DEFAULT_AI_PROVIDER};
pub use types::{AiModelInfo, AiProvider, AiSecretStatus, ChatMessage};

use catalog::{fallback_cloud_models, fallback_models_for};
use debug_log::{append_chat_debug_log, ChatDebugLogInput, ChatDebugLogOutcome};
use providers::{fetch_provider_models, send_chat_request};
use secrets::{
    is_secret_present, load_provider_secret_optional, provider_secret_key, KEY_ANTHROPIC,
    KEY_AZURE_OPENAI, KEY_GEMINI, KEY_OPENAI, KEY_OPENROUTER,
};
use std::time::Instant;

#[tauri::command]
pub async fn ai_get_models() -> Result<Vec<AiModelInfo>, String> {
    let client = reqwest::Client::new();
    let mut models = Vec::new();

    for provider in [
        AiProvider::OpenAi,
        AiProvider::Anthropic,
        AiProvider::Gemini,
        AiProvider::OpenRouter,
    ] {
        let provider_models = match load_provider_secret_optional(&provider) {
            Ok(Some(api_key)) => {
                if let Some(cached_models) = model_cache::cached_fresh_models_for(&provider) {
                    cached_models
                } else {
                    match fetch_provider_models(&client, &provider, Some(&api_key), None, "en")
                        .await
                    {
                        Ok(fetched_models) => {
                            if let Err(e) =
                                model_cache::store_models_for(&provider, &fetched_models)
                            {
                                log::warn!(
                                    "Failed to store AI model cache for {}: {}",
                                    provider.display_name(),
                                    e
                                );
                            }
                            fetched_models
                        }
                        Err(_) => model_cache::cached_any_models_for(&provider)
                            .unwrap_or_else(|| fallback_models_for(&provider)),
                    }
                }
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
    let response_language = if language.starts_with("ja") {
        "Japanese"
    } else {
        "English"
    };

    let mut s = format!(
        r#"You are the AI assistant for ExaTerm, a network terminal. Help the user with network operations and diagnostics.

Response language:
- Respond in {response_language}.
- Keep provider/API error wording separate from your diagnostic reasoning.

Fact handling:
- Treat terminal output, logs, and configuration files as the highest-priority evidence.
- Treat user statements as hypotheses or supplemental context until they are checked against the provided evidence.
- If evidence conflicts, rely on terminal output, logs, and configuration values over user assumptions.

Scope clarification:
- Clearly distinguish the local device from peer, remote, or external systems.
- Determine the local device context from hostnames, prompts, command outputs, and configuration values.
- Do not assume information about a peer, neighbor, remote host, or external system also applies to the local device.

Reasoning and explanation:
- For diagnostic answers, present conclusion, evidence, reasoning or hypothesis, and remediation when useful.
- Cite concrete observed values from the provided terminal context, logs, or configuration when available.
- If evidence is insufficient, or if the subject is omitted or ambiguous, avoid definitive statements and ask for the missing data.
- Do not claim settings are matching, consistent, or equivalent unless both sides were explicitly compared.

Command suggestions:
- When you suggest executable commands, put each candidate in a fenced code block labeled bash, sh, powershell, ps1, cmd, bat, terminal, or console.
- The user will review commands before running them, so do not imply that commands execute automatically."#
    );

    if let Some(ctx) = terminal_context {
        s.push_str(&format!("\n\n[Terminal Output]\n```\n{}\n```", ctx));
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
        openrouter: is_secret_present(KEY_OPENROUTER)?,
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
    let config = crate::config::config_read()
        .map_err(|e| {
            log::warn!("Failed to load config for AI settings: {}", e);
            e
        })
        .ok();
    let debug_log_enabled = config
        .as_ref()
        .map(|cfg| cfg.ai.debug_log_enabled)
        .unwrap_or(false);
    let effective_azure_openai_endpoint = if matches!(provider, AiProvider::AzureOpenAi) {
        effective_azure_openai_endpoint(
            azure_openai_endpoint.as_deref(),
            config
                .as_ref()
                .map(|cfg| cfg.ai.azure_openai_endpoint.as_str()),
        )
    } else {
        azure_openai_endpoint
    };
    let started = Instant::now();

    let result = send_chat_request(
        &client,
        provider.clone(),
        &model,
        &messages,
        &system_prompt,
        &language,
        ollama_base_url.as_deref(),
        effective_azure_openai_endpoint.as_deref(),
    )
    .await;

    if debug_log_enabled {
        let outcome = match &result {
            Ok(response) => ChatDebugLogOutcome::Success { response },
            Err(error) => ChatDebugLogOutcome::Error { error },
        };
        if let Err(e) = append_chat_debug_log(ChatDebugLogInput {
            provider: &provider,
            model: &model,
            language: &language,
            terminal_context_included: terminal_context.is_some(),
            messages: &messages,
            system_prompt: &system_prompt,
            duration_ms: started.elapsed().as_millis(),
            outcome,
        }) {
            log::warn!("Failed to write AI debug log: {}", e);
        }
    }

    result
}

fn effective_azure_openai_endpoint(
    requested_endpoint: Option<&str>,
    configured_endpoint: Option<&str>,
) -> Option<String> {
    requested_endpoint
        .filter(|endpoint| !endpoint.trim().is_empty())
        .or_else(|| configured_endpoint.filter(|endpoint| !endpoint.trim().is_empty()))
        .map(|endpoint| endpoint.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_uses_english_instructions_for_japanese_responses() {
        let prompt = build_system_prompt(&None, "ja");

        assert!(prompt.contains("You are the AI assistant for ExaTerm"));
        assert!(prompt.contains("Respond in Japanese"));
        assert!(prompt.contains("highest-priority evidence"));
        assert!(prompt.contains("local device from peer, remote, or external systems"));
        assert!(prompt.contains("avoid definitive statements and ask for the missing data"));
        assert!(prompt.contains("Do not claim settings are matching"));
        assert!(!prompt.contains("You are ExaTerm"));
    }

    #[test]
    fn system_prompt_requests_english_responses_for_non_japanese_language() {
        let prompt = build_system_prompt(&None, "en");

        assert!(prompt.contains("Respond in English"));
        assert!(!prompt.contains("Respond in Japanese"));
    }

    #[test]
    fn system_prompt_includes_terminal_context_with_english_label() {
        let prompt = build_system_prompt(&Some("router# show ip ospf neighbor".into()), "ja-JP");

        assert!(prompt.contains("[Terminal Output]\n```\nrouter# show ip ospf neighbor\n```"));
        assert!(!prompt.contains("[Terminal Output in Japanese]"));
    }

    #[test]
    fn system_prompt_keeps_command_block_guidance() {
        let prompt = build_system_prompt(&None, "en");

        assert!(prompt.contains("fenced code block labeled bash, sh, powershell"));
        assert!(prompt.contains("do not imply that commands execute automatically"));
    }

    #[test]
    fn azure_openai_endpoint_uses_requested_value_first() {
        let endpoint = effective_azure_openai_endpoint(
            Some(" https://request.example/openai/v1/chat/completions "),
            Some("https://config.example/openai/v1/chat/completions"),
        );

        assert_eq!(
            endpoint.as_deref(),
            Some("https://request.example/openai/v1/chat/completions")
        );
    }

    #[test]
    fn azure_openai_endpoint_falls_back_to_config() {
        let endpoint = effective_azure_openai_endpoint(
            Some("   "),
            Some(" https://config.example/openai/v1/chat/completions "),
        );

        assert_eq!(
            endpoint.as_deref(),
            Some("https://config.example/openai/v1/chat/completions")
        );
    }
}
