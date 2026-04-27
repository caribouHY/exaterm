use reqwest::{Response, StatusCode};

use super::types::AiProvider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorLanguage {
    English,
    Japanese,
}

impl ErrorLanguage {
    pub fn from_language(language: &str) -> Self {
        if language.starts_with("ja") {
            Self::Japanese
        } else {
            Self::English
        }
    }

    pub fn is_ja(self) -> bool {
        matches!(self, Self::Japanese)
    }
}

pub fn missing_secret_message(provider: &AiProvider, language: &str) -> String {
    if ErrorLanguage::from_language(language).is_ja() {
        format!(
            "{} の API キーが設定されていません。Settings で API キーを保存してから再試行してください。",
            provider.display_name()
        )
    } else {
        format!(
            "{} API key is not configured. Save the API key in Settings, then try again.",
            provider.display_name()
        )
    }
}

pub async fn response_json(
    resp: Response,
    provider: &AiProvider,
    language: &str,
    operation: &str,
) -> Result<serde_json::Value, String> {
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format_network_error(provider, language, &e))?;

    if !status.is_success() {
        return Err(format_http_error(provider, language, status, &text));
    }

    serde_json::from_str(&text).map_err(|_| {
        if ErrorLanguage::from_language(language).is_ja() {
            format!(
                "{} の {} 応答を解析できませんでした。しばらくしてから再試行してください。",
                provider.display_name(),
                operation
            )
        } else {
            format!(
                "Could not parse the {} {} response. Try again later.",
                provider.display_name(),
                operation
            )
        }
    })
}

pub fn format_network_error(
    provider: &AiProvider,
    language: &str,
    error: &reqwest::Error,
) -> String {
    if ErrorLanguage::from_language(language).is_ja() {
        match provider {
            AiProvider::Ollama => format!(
                "Ollama に接続できませんでした。Ollama が起動していること、Base URL が正しいことを確認してください。詳細: {}",
                error
            ),
            _ => format!(
                "{} に接続できませんでした。ネットワーク接続、プロキシ、ファイアウォール設定を確認してください。詳細: {}",
                provider.display_name(),
                error
            ),
        }
    } else {
        match provider {
            AiProvider::Ollama => format!(
                "Could not connect to Ollama. Check that Ollama is running and the Base URL is correct. Details: {}",
                error
            ),
            _ => format!(
                "Could not connect to {}. Check your network connection, proxy, and firewall settings. Details: {}",
                provider.display_name(),
                error
            ),
        }
    }
}

pub fn format_http_error(
    provider: &AiProvider,
    language: &str,
    status: StatusCode,
    body: &str,
) -> String {
    let provider_message = extract_provider_error_message(body);
    let detail = if provider_message.is_empty() {
        String::new()
    } else {
        format!(" Provider message: {}", provider_message)
    };

    let ja = ErrorLanguage::from_language(language).is_ja();
    let base = match status.as_u16() {
        401 | 403 if ja => format!(
            "{} の認証に失敗しました。API キーが正しいこと、必要な権限があることを確認してください。",
            provider.display_name()
        ),
        401 | 403 => format!(
            "{} authentication failed. Check that the API key is correct and has the required permissions.",
            provider.display_name()
        ),
        404 if ja => format!(
            "{} のエンドポイントまたはモデルが見つかりません。選択したモデルが現在利用可能か確認してください。",
            provider.display_name()
        ),
        404 => format!(
            "{} endpoint or model was not found. Check that the selected model is currently available.",
            provider.display_name()
        ),
        429 if ja => format!(
            "{} のレート制限またはクォータに達しました。利用上限を確認するか、時間を置いて再試行してください。",
            provider.display_name()
        ),
        429 => format!(
            "{} rate limit or quota was reached. Check your usage limits or try again later.",
            provider.display_name()
        ),
        500..=599 if ja => format!(
            "{} 側で一時的な障害が発生しています。時間を置いて再試行してください。",
            provider.display_name()
        ),
        500..=599 => format!(
            "{} is currently returning a server error. Try again later.",
            provider.display_name()
        ),
        _ if ja => format!(
            "{} が HTTP {} を返しました。設定と選択したモデルを確認してください。",
            provider.display_name(),
            status.as_u16()
        ),
        _ => format!(
            "{} returned HTTP {}. Check your settings and selected model.",
            provider.display_name(),
            status.as_u16()
        ),
    };

    format!("{}{}", base, detail)
}

fn extract_provider_error_message(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.trim().chars().take(300).collect();
    };

    value["error"]["message"]
        .as_str()
        .or_else(|| value["error"]["error"]["message"].as_str())
        .or_else(|| value["error"]["status"].as_str())
        .or_else(|| value["message"].as_str())
        .unwrap_or("")
        .chars()
        .take(300)
        .collect()
}

pub fn malformed_models_response_message(
    provider: &AiProvider,
    language: &str,
    body: &serde_json::Value,
) -> String {
    if ErrorLanguage::from_language(language).is_ja() {
        format!(
            "{} の応答から利用可能なモデルを取得できませんでした。応答形式が変わった可能性があります。詳細: {}",
            provider.display_name(),
            body
        )
    } else {
        format!(
            "Could not read available models from the {} response. The response format may have changed. Details: {}",
            provider.display_name(),
            body
        )
    }
}

pub fn malformed_chat_response_message(
    provider: &AiProvider,
    language: &str,
    body: &serde_json::Value,
) -> String {
    if ErrorLanguage::from_language(language).is_ja() {
        format!(
            "{} のチャット応答を読み取れませんでした。時間を置いて再試行してください。詳細: {}",
            provider.display_name(),
            body
        )
    } else {
        format!(
            "Could not read the {} chat response. Try again later. Details: {}",
            provider.display_name(),
            body
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_openai_auth_error_in_japanese() {
        let body = r#"{"error":{"message":"Incorrect API key provided"}}"#;
        let message = format_http_error(&AiProvider::OpenAi, "ja", StatusCode::UNAUTHORIZED, body);

        assert!(message.contains("OpenAI の認証に失敗しました"));
        assert!(message.contains("Incorrect API key provided"));
    }

    #[test]
    fn formats_quota_error_in_english() {
        let body = r#"{"error":{"message":"You exceeded your current quota"}}"#;
        let message = format_http_error(
            &AiProvider::Gemini,
            "en",
            StatusCode::TOO_MANY_REQUESTS,
            body,
        );

        assert!(message.contains("rate limit or quota"));
        assert!(message.contains("You exceeded your current quota"));
    }

    #[test]
    fn missing_secret_error_is_actionable() {
        let message = missing_secret_message(&AiProvider::Anthropic, "en");

        assert!(message.contains("Anthropic API key is not configured"));
        assert!(message.contains("Settings"));
    }
}
