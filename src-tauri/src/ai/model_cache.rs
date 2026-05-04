use super::types::{AiModelInfo, AiProvider};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const CACHE_FILE_NAME: &str = "ai_models_cache.json";
const CACHE_TTL_HOURS: i64 = 24;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ModelCache {
    #[serde(default)]
    providers: BTreeMap<String, ProviderModelCache>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderModelCache {
    fetched_at: DateTime<Utc>,
    models: Vec<AiModelInfo>,
}

pub fn cached_fresh_models_for(provider: &AiProvider) -> Option<Vec<AiModelInfo>> {
    cached_models_for(provider, true)
}

pub fn cached_any_models_for(provider: &AiProvider) -> Option<Vec<AiModelInfo>> {
    cached_models_for(provider, false)
}

pub fn store_models_for(provider: &AiProvider, models: &[AiModelInfo]) -> Result<(), String> {
    store_models_at(&cache_path(), provider, models, Utc::now())
}

pub fn is_cacheable_provider(provider: &AiProvider) -> bool {
    matches!(
        provider,
        AiProvider::OpenAi | AiProvider::Anthropic | AiProvider::Gemini | AiProvider::OpenRouter
    )
}

fn cached_models_for(provider: &AiProvider, require_fresh: bool) -> Option<Vec<AiModelInfo>> {
    if !is_cacheable_provider(provider) {
        return None;
    }

    let cache = read_cache_from(&cache_path()).ok()?;
    let entry = cache.providers.get(provider.id())?;
    if entry.models.is_empty() {
        return None;
    }

    if require_fresh && !is_fresh(entry.fetched_at, Utc::now()) {
        return None;
    }

    Some(entry.models.clone())
}

fn cache_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join(CACHE_FILE_NAME)
}

fn read_cache_from(path: &Path) -> Result<ModelCache, String> {
    if !path.exists() {
        return Ok(ModelCache::default());
    }

    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

fn store_models_at(
    path: &Path,
    provider: &AiProvider,
    models: &[AiModelInfo],
    fetched_at: DateTime<Utc>,
) -> Result<(), String> {
    if !is_cacheable_provider(provider) || models.is_empty() {
        return Ok(());
    }

    let mut cache = read_cache_from(path).unwrap_or_default();
    cache.providers.insert(
        provider.id().to_string(),
        ProviderModelCache {
            fetched_at,
            models: models.to_vec(),
        },
    );

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(&cache).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

fn is_fresh(fetched_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now.signed_duration_since(fetched_at) < Duration::hours(CACHE_TTL_HOURS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_cache_path() -> PathBuf {
        std::env::temp_dir().join(format!("exaterm-ai-model-cache-{}.json", Uuid::new_v4()))
    }

    fn sample_models(provider: &AiProvider) -> Vec<AiModelInfo> {
        vec![AiModelInfo {
            provider: provider.id().into(),
            model_id: "sample-model".into(),
            display_name: "Sample Model".into(),
        }]
    }

    #[test]
    fn cache_entry_within_ttl_is_fresh() {
        let now = Utc::now();

        assert!(is_fresh(now - Duration::hours(23), now));
    }

    #[test]
    fn cache_entry_after_ttl_is_stale() {
        let now = Utc::now();

        assert!(!is_fresh(now - Duration::hours(25), now));
    }

    #[test]
    fn stores_and_reads_models_by_provider() {
        let path = temp_cache_path();
        let provider = AiProvider::Gemini;
        let models = sample_models(&provider);

        store_models_at(&path, &provider, &models, Utc::now()).unwrap();
        let cache = read_cache_from(&path).unwrap();

        let entry = cache.providers.get(provider.id()).unwrap();
        assert_eq!(entry.models, models);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn ollama_is_not_cacheable() {
        assert!(!is_cacheable_provider(&AiProvider::Ollama));
    }

    #[test]
    fn azure_openai_is_not_cacheable() {
        assert!(!is_cacheable_provider(&AiProvider::AzureOpenAi));
    }
}
