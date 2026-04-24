use super::catalog::OLLAMA_DEFAULT_BASE_URL;
use super::errors::{
    format_network_error, malformed_chat_response_message, malformed_models_response_message,
    missing_secret_message, response_json, ErrorLanguage,
};
use super::secrets::load_provider_secret;
use super::types::{AiModelInfo, AiProvider, ChatMessage};

const OPENAI_MODELS_URL: &str = "https://api.openai.com/v1/models";
const OPENAI_CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";
const ANTHROPIC_MODELS_URL: &str = "https://api.anthropic.com/v1/models";
const ANTHROPIC_MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const GEMINI_MODELS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const GEMINI_GENERATE_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub async fn fetch_provider_models(
    client: &reqwest::Client,
    provider: &AiProvider,
    api_key: Option<&str>,
    ollama_base_url: Option<&str>,
    language: &str,
) -> Result<Vec<AiModelInfo>, String> {
    let body = match provider {
        AiProvider::OpenAi => {
            let api_key = api_key.ok_or_else(|| missing_secret_message(provider, language))?;
            let resp = client
                .get(OPENAI_MODELS_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await
                .map_err(|e| format_network_error(provider, language, &e))?;
            response_json(resp, provider, language, "model list").await?
        }
        AiProvider::Anthropic => {
            let api_key = api_key.ok_or_else(|| missing_secret_message(provider, language))?;
            let resp = client
                .get(ANTHROPIC_MODELS_URL)
                .header("x-api-key", api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .send()
                .await
                .map_err(|e| format_network_error(provider, language, &e))?;
            response_json(resp, provider, language, "model list").await?
        }
        AiProvider::Gemini => {
            let api_key = api_key.ok_or_else(|| missing_secret_message(provider, language))?;
            let url = format!("{}?key={}", GEMINI_MODELS_URL, api_key);
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format_network_error(provider, language, &e))?;
            response_json(resp, provider, language, "model list").await?
        }
        AiProvider::Ollama => {
            let base_url = normalized_ollama_base_url(ollama_base_url);
            let url = format!("{}/api/tags", base_url);
            let resp = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format_network_error(provider, language, &e))?;
            response_json(resp, provider, language, "model list").await?
        }
    };

    let models = parse_models(provider, &body);
    if models.is_empty() {
        Err(malformed_models_response_message(provider, language, &body))
    } else {
        Ok(models)
    }
}

pub async fn send_chat_request(
    client: &reqwest::Client,
    provider: AiProvider,
    model: &str,
    messages: &[ChatMessage],
    system_prompt: &str,
    language: &str,
    ollama_base_url: Option<&str>,
) -> Result<String, String> {
    match provider {
        AiProvider::OpenAi => {
            send_openai_chat(client, model, messages, system_prompt, language).await
        }
        AiProvider::Anthropic => {
            send_anthropic_chat(client, model, messages, system_prompt, language).await
        }
        AiProvider::Gemini => {
            send_gemini_chat(client, model, messages, system_prompt, language).await
        }
        AiProvider::Ollama => {
            send_ollama_chat(
                client,
                model,
                messages,
                system_prompt,
                language,
                ollama_base_url,
            )
            .await
        }
    }
}

async fn send_openai_chat(
    client: &reqwest::Client,
    model: &str,
    messages: &[ChatMessage],
    system_prompt: &str,
    language: &str,
) -> Result<String, String> {
    let provider = AiProvider::OpenAi;
    let api_key = load_provider_secret(&provider, language)?;
    validate_model(client, &provider, model, Some(&api_key), None, language).await?;

    let mut payload_messages = vec![serde_json::json!({"role":"system","content":system_prompt})];
    for message in messages {
        payload_messages.push(serde_json::json!({"role":&message.role,"content":&message.content}));
    }

    let resp = client
        .post(OPENAI_CHAT_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({"model":model,"messages":payload_messages,"temperature":0.7}))
        .send()
        .await
        .map_err(|e| format_network_error(&provider, language, &e))?;
    let body = response_json(resp, &provider, language, "chat").await?;

    body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| malformed_chat_response_message(&provider, language, &body))
}

async fn send_anthropic_chat(
    client: &reqwest::Client,
    model: &str,
    messages: &[ChatMessage],
    system_prompt: &str,
    language: &str,
) -> Result<String, String> {
    let provider = AiProvider::Anthropic;
    let api_key = load_provider_secret(&provider, language)?;
    validate_model(client, &provider, model, Some(&api_key), None, language).await?;

    let payload_messages: Vec<_> = messages
        .iter()
        .map(|message| serde_json::json!({"role":&message.role,"content":&message.content}))
        .collect();

    let resp = client
        .post(ANTHROPIC_MESSAGES_URL)
        .header("x-api-key", &api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&serde_json::json!({"model":model,"max_tokens":4096,"system":system_prompt,"messages":payload_messages}))
        .send()
        .await
        .map_err(|e| format_network_error(&provider, language, &e))?;
    let body = response_json(resp, &provider, language, "chat").await?;

    body["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| malformed_chat_response_message(&provider, language, &body))
}

async fn send_gemini_chat(
    client: &reqwest::Client,
    model: &str,
    messages: &[ChatMessage],
    system_prompt: &str,
    language: &str,
) -> Result<String, String> {
    let provider = AiProvider::Gemini;
    let api_key = load_provider_secret(&provider, language)?;
    validate_model(client, &provider, model, Some(&api_key), None, language).await?;

    let mut parts = vec![serde_json::json!({"text":system_prompt})];
    for message in messages {
        parts.push(serde_json::json!({"text":format!("{}:{}", message.role, message.content)}));
    }

    let url = format!(
        "{}/{}:generateContent?key={}",
        GEMINI_GENERATE_BASE_URL, model, api_key
    );
    let resp = client
        .post(&url)
        .json(&serde_json::json!({"contents":[{"parts":parts}],"generationConfig":{"temperature":0.7,"maxOutputTokens":4096}}))
        .send()
        .await
        .map_err(|e| format_network_error(&provider, language, &e))?;
    let body = response_json(resp, &provider, language, "chat").await?;

    body["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| malformed_chat_response_message(&provider, language, &body))
}

async fn send_ollama_chat(
    client: &reqwest::Client,
    model: &str,
    messages: &[ChatMessage],
    system_prompt: &str,
    language: &str,
    ollama_base_url: Option<&str>,
) -> Result<String, String> {
    let provider = AiProvider::Ollama;
    let base_url = normalized_ollama_base_url(ollama_base_url);
    validate_model(client, &provider, model, None, Some(&base_url), language).await?;

    let mut payload_messages = vec![serde_json::json!({"role":"system","content":system_prompt})];
    for message in messages {
        payload_messages.push(serde_json::json!({"role":&message.role,"content":&message.content}));
    }

    let url = format!("{}/api/chat", base_url);
    let resp = client
        .post(&url)
        .json(&serde_json::json!({"model":model,"messages":payload_messages,"stream":false}))
        .send()
        .await
        .map_err(|e| format_network_error(&provider, language, &e))?;
    let body = response_json(resp, &provider, language, "chat").await?;

    body["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| malformed_chat_response_message(&provider, language, &body))
}

fn parse_models(provider: &AiProvider, body: &serde_json::Value) -> Vec<AiModelInfo> {
    match provider {
        AiProvider::OpenAi => body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| model["id"].as_str())
            .map(|id| model_info(provider, id, &model_display_name(id)))
            .collect(),
        AiProvider::Anthropic => body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| {
                let id = model["id"].as_str()?;
                let display = model["display_name"].as_str().unwrap_or(id);
                Some(model_info(provider, id, display))
            })
            .collect(),
        AiProvider::Gemini => body["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|model| {
                model["supportedGenerationMethods"]
                    .as_array()
                    .map(|methods| {
                        methods
                            .iter()
                            .any(|method| method.as_str() == Some("generateContent"))
                    })
                    .unwrap_or(true)
            })
            .filter_map(|model| {
                let raw_name = model["name"].as_str()?;
                let id = normalize_gemini_model_id(raw_name);
                let display = model["displayName"].as_str().unwrap_or(&id);
                Some(model_info(provider, &id, display))
            })
            .collect(),
        AiProvider::Ollama => body["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| model["name"].as_str())
            .map(|name| model_info(provider, name, name))
            .collect(),
    }
}

fn model_info(provider: &AiProvider, model_id: &str, display_name: &str) -> AiModelInfo {
    AiModelInfo {
        provider: provider.id().into(),
        model_id: model_id.into(),
        display_name: display_name.into(),
    }
}

fn model_display_name(model_id: &str) -> String {
    model_id
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_gemini_model_id(raw_name: &str) -> String {
    raw_name
        .strip_prefix("models/")
        .unwrap_or(raw_name)
        .to_string()
}

fn normalized_ollama_base_url(ollama_base_url: Option<&str>) -> String {
    ollama_base_url
        .filter(|url| !url.trim().is_empty())
        .unwrap_or(OLLAMA_DEFAULT_BASE_URL)
        .trim_end_matches('/')
        .to_string()
}

fn model_is_available(available: &[AiModelInfo], model: &str) -> bool {
    available
        .iter()
        .any(|available_model| available_model.model_id == model)
}

async fn validate_model(
    client: &reqwest::Client,
    provider: &AiProvider,
    model: &str,
    api_key: Option<&str>,
    ollama_base_url: Option<&str>,
    language: &str,
) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err(if ErrorLanguage::from_language(language).is_ja() {
            "AI モデルが選択されていません。モデルを選択してから再試行してください。".into()
        } else {
            "No AI model is selected. Choose a model, then try again.".into()
        });
    }

    let available =
        fetch_provider_models(client, provider, api_key, ollama_base_url, language).await?;
    if model_is_available(&available, model) {
        return Ok(());
    }

    Err(if ErrorLanguage::from_language(language).is_ja() {
        format!(
            "{} ではモデル '{}' を利用できません。モデル一覧を更新し、利用可能なモデルを選択してください。",
            provider.display_name(),
            model
        )
    } else {
        format!(
            "Model '{}' is not available for {}. Refresh the model list and choose an available model.",
            model,
            provider.display_name()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::catalog::fallback_models_for;

    #[test]
    fn parses_and_normalizes_gemini_models() {
        let body = serde_json::json!({
            "models": [
                {
                    "name": "models/gemini-2.5-flash",
                    "displayName": "Gemini 2.5 Flash",
                    "supportedGenerationMethods": ["generateContent"]
                }
            ]
        });

        let models = parse_models(&AiProvider::Gemini, &body);

        assert_eq!(models.len(), 1);
        assert_eq!(models[0].provider, "Gemini");
        assert_eq!(models[0].model_id, "gemini-2.5-flash");
        assert_eq!(models[0].display_name, "Gemini 2.5 Flash");
    }

    #[test]
    fn excludes_gemini_models_without_generate_content() {
        let body = serde_json::json!({
            "models": [
                {
                    "name": "models/gemini-embedding-001",
                    "supportedGenerationMethods": ["embedContent"]
                }
            ]
        });

        let models = parse_models(&AiProvider::Gemini, &body);

        assert!(models.is_empty());
    }

    #[test]
    fn parses_ollama_models() {
        let body = serde_json::json!({
            "models": [
                { "name": "llama3.2:latest" }
            ]
        });

        let models = parse_models(&AiProvider::Ollama, &body);

        assert_eq!(models[0].model_id, "llama3.2:latest");
        assert_eq!(models[0].display_name, "llama3.2:latest");
    }

    #[test]
    fn model_availability_accepts_existing_model_and_rejects_bad_model() {
        let models = fallback_models_for(&AiProvider::Anthropic);

        assert!(model_is_available(&models, "claude-sonnet-4-20250514"));
        assert!(!model_is_available(&models, "not-a-real-model"));
    }
}
