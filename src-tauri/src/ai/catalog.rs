use super::types::{AiModelInfo, AiProvider};

pub const OLLAMA_DEFAULT_BASE_URL: &str = "http://localhost:11434";
pub const DEFAULT_AI_PROVIDER: &str = "OpenAi";
pub const DEFAULT_AI_MODEL: &str = "gpt-4o";

pub fn fallback_models_for(provider: &AiProvider) -> Vec<AiModelInfo> {
    let rows: &[(&str, &str)] = match provider {
        AiProvider::OpenAi => &[(DEFAULT_AI_MODEL, "GPT-4o"), ("gpt-4o-mini", "GPT-4o Mini")],
        AiProvider::AzureOpenAi => &[],
        AiProvider::Anthropic => &[
            ("claude-sonnet-4-20250514", "Claude Sonnet 4"),
            ("claude-3-5-haiku-20241022", "Claude 3.5 Haiku"),
        ],
        AiProvider::Gemini => &[
            ("gemini-2.5-pro", "Gemini 2.5 Pro"),
            ("gemini-2.5-flash", "Gemini 2.5 Flash"),
        ],
        AiProvider::Ollama => &[],
    };

    rows.iter()
        .map(|(model_id, display_name)| AiModelInfo {
            provider: provider.id().into(),
            model_id: (*model_id).into(),
            display_name: (*display_name).into(),
        })
        .collect()
}

pub fn fallback_cloud_models() -> Vec<AiModelInfo> {
    [
        AiProvider::OpenAi,
        AiProvider::Anthropic,
        AiProvider::Gemini,
    ]
    .iter()
    .flat_map(fallback_models_for)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_catalog_contains_default_model() {
        let models = fallback_models_for(&AiProvider::OpenAi);

        assert!(models
            .iter()
            .any(|model| model.model_id == DEFAULT_AI_MODEL));
    }
}
