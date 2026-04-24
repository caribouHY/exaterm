use keyring::{Entry, Error as KeyringError};

use super::errors::missing_secret_message;
use super::types::AiProvider;

const KEYRING_SERVICE: &str = "com.caribouhy.exaterm";
pub const KEY_OPENAI: &str = "openai_api_key";
pub const KEY_ANTHROPIC: &str = "anthropic_api_key";
pub const KEY_GEMINI: &str = "gemini_api_key";

fn keyring_entry(key_name: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, key_name).map_err(|e| e.to_string())
}

pub fn provider_secret_key(provider: &str) -> Option<&'static str> {
    match provider {
        "OpenAi" => Some(KEY_OPENAI),
        "Anthropic" => Some(KEY_ANTHROPIC),
        "Gemini" => Some(KEY_GEMINI),
        _ => None,
    }
}

pub fn is_secret_present(key_name: &str) -> Result<bool, String> {
    let entry = keyring_entry(key_name)?;
    match entry.get_password() {
        Ok(secret) => Ok(!secret.is_empty()),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

fn secret_key_for_provider(provider: &AiProvider) -> Option<&'static str> {
    match provider {
        AiProvider::OpenAi => Some(KEY_OPENAI),
        AiProvider::Anthropic => Some(KEY_ANTHROPIC),
        AiProvider::Gemini => Some(KEY_GEMINI),
        AiProvider::Ollama => None,
    }
}

pub fn load_provider_secret_optional(provider: &AiProvider) -> Result<Option<String>, String> {
    let Some(key_name) = secret_key_for_provider(provider) else {
        return Ok(None);
    };

    let entry = keyring_entry(key_name)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok(Some(secret)),
        Ok(_) | Err(KeyringError::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn load_provider_secret(provider: &AiProvider, language: &str) -> Result<String, String> {
    let Some(key_name) = secret_key_for_provider(provider) else {
        return Err(missing_secret_message(provider, language));
    };

    let entry = keyring_entry(key_name)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok(secret),
        Ok(_) | Err(KeyringError::NoEntry) => Err(missing_secret_message(provider, language)),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_secret(key_name: &str, value: &str) -> Result<(), String> {
    let entry = keyring_entry(key_name)?;
    entry.set_password(value).map_err(|e| e.to_string())
}

pub fn clear_secret(key_name: &str) -> Result<(), String> {
    let entry = keyring_entry(key_name)?;
    match entry.delete_credential() {
        Ok(_) | Err(KeyringError::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
