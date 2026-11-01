use reqwest::{Response, StatusCode};

use super::types::AiProvider;

pub fn missing_secret_message(provider: &AiProvider, language: &str) -> String {
    let _ = language;
    format!(
        "{} API key is not configured. Save the API key in Settings, then try again.",
        provider.display_name()
    )
}

pub async fn response_json(
    resp: Response,
    provider: &AiProvider,
    language: &str,
    operation: &str,
) -> Result<serde_json::Value, String> {
    let status = resp.status();
    // Provider/proxy error bodies can echo credentials, even when authentication uses headers.
    if !status.is_success() {
        return Err(format_http_error(provider, language, status));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| format_network_error(provider, language, &e))?;

    serde_json::from_str(&text).map_err(|_| {
        let _ = language;
        format!(
            "Could not parse the {} {} response. Try again later.",
            provider.display_name(),
            operation
        )
    })
}

pub fn format_network_error(
    provider: &AiProvider,
    language: &str,
    error: &reqwest::Error,
) -> String {
    // Do not format reqwest errors or their sources: URLs and builder errors may contain secrets.
    let detail = if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_builder() {
        "invalid request configuration"
    } else if error.is_decode() {
        "response decoding failed"
    } else if error.is_body() {
        "response body transfer failed"
    } else if error.is_redirect() {
        "redirect failed"
    } else {
        "request failed"
    };
    let message = match provider {
        AiProvider::Ollama => format!(
            "Could not connect to Ollama. Check that Ollama is running and the Base URL is correct. Details: {}",
            detail
        ),
        _ => format!(
            "Could not connect to {}. Check your network connection, proxy, and firewall settings. Details: {}",
            provider.display_name(),
            detail
        ),
    };
    let _ = language;
    message
}

pub fn format_http_error(provider: &AiProvider, language: &str, status: StatusCode) -> String {
    let base = match status.as_u16() {
        401 | 403 => format!(
            "{} authentication failed. Check that the API key is correct and has the required permissions.",
            provider.display_name()
        ),
        404 => format!(
            "{} endpoint or model was not found. Check that the selected model is currently available.",
            provider.display_name()
        ),
        429 => format!(
            "{} rate limit or quota was reached. Check your usage limits or try again later.",
            provider.display_name()
        ),
        500..=599 => format!(
            "{} is currently returning a server error. Try again later.",
            provider.display_name()
        ),
        _ => format!(
            "{} returned HTTP {}. Check your settings and selected model.",
            provider.display_name(),
            status.as_u16()
        ),
    };

    let _ = language;
    format!("{} Provider message: HTTP {}", base, status.as_u16())
}

pub fn malformed_models_response_message(provider: &AiProvider, language: &str) -> String {
    let _ = language;
    format!(
        "Could not read available models from the {} response. The response format may have changed.",
        provider.display_name()
    )
}

pub fn malformed_chat_response_message(provider: &AiProvider, language: &str) -> String {
    let _ = language;
    format!(
        "Could not read the {} chat response. Try again later.",
        provider.display_name()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_openai_auth_error_as_stable_english_fallback() {
        let message = format_http_error(&AiProvider::OpenAi, "ja", StatusCode::UNAUTHORIZED);

        assert!(message.contains("authentication failed"));
        assert!(message.contains("HTTP 401"));
    }

    #[test]
    fn formats_quota_error_in_english() {
        let message = format_http_error(&AiProvider::Gemini, "en", StatusCode::TOO_MANY_REQUESTS);

        assert!(message.contains("rate limit or quota"));
        assert!(message.contains("HTTP 429"));
    }

    #[test]
    fn missing_secret_error_is_actionable() {
        let message = missing_secret_message(&AiProvider::Anthropic, "en");

        assert!(message.contains("Anthropic API key is not configured"));
        assert!(message.contains("Settings"));
    }
}
