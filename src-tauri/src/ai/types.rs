use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiProvider {
    OpenAi,
    AzureOpenAi,
    Anthropic,
    Gemini,
    OpenRouter,
    Ollama,
}

impl AiProvider {
    pub fn id(&self) -> &'static str {
        match self {
            AiProvider::OpenAi => "OpenAi",
            AiProvider::AzureOpenAi => "AzureOpenAi",
            AiProvider::Anthropic => "Anthropic",
            AiProvider::Gemini => "Gemini",
            AiProvider::OpenRouter => "OpenRouter",
            AiProvider::Ollama => "Ollama",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            AiProvider::OpenAi => "OpenAI",
            AiProvider::AzureOpenAi => "Azure OpenAI",
            AiProvider::Anthropic => "Anthropic",
            AiProvider::Gemini => "Gemini",
            AiProvider::OpenRouter => "OpenRouter",
            AiProvider::Ollama => "Ollama",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiModelInfo {
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSecretStatus {
    pub openai: bool,
    pub azure_openai: bool,
    pub anthropic: bool,
    pub gemini: bool,
    pub openrouter: bool,
}
