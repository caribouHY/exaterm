use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

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

#[tauri::command]
pub fn ai_get_models() -> Vec<AiModelInfo> {
    vec![
        AiModelInfo { provider: "OpenAi".into(), model_id: "gpt-4o".into(), display_name: "GPT-4o".into() },
        AiModelInfo { provider: "OpenAi".into(), model_id: "gpt-4o-mini".into(), display_name: "GPT-4o Mini".into() },
        AiModelInfo { provider: "Anthropic".into(), model_id: "claude-sonnet-4-20250514".into(), display_name: "Claude Sonnet 4".into() },
        AiModelInfo { provider: "Anthropic".into(), model_id: "claude-3-5-haiku-20241022".into(), display_name: "Claude 3.5 Haiku".into() },
        AiModelInfo { provider: "Gemini".into(), model_id: "gemini-2.5-pro".into(), display_name: "Gemini 2.5 Pro".into() },
        AiModelInfo { provider: "Gemini".into(), model_id: "gemini-2.0-flash".into(), display_name: "Gemini 2.0 Flash".into() },
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

fn build_system_prompt(terminal_context: &Option<String>) -> String {
    let mut s = "あなたはExaTermのAIアシスタントです。ネットワーク操作を支援します。日本語で回答してください。".to_string();
    if let Some(ctx) = terminal_context {
        s.push_str(&format!("\n\n【ターミナル出力】\n```\n{}\n```", ctx));
    }
    s
}

#[tauri::command]
pub async fn ai_chat(
    provider: AiProvider, model: String, messages: Vec<ChatMessage>,
    terminal_context: Option<String>, api_key: String,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let sys = build_system_prompt(&terminal_context);

    match provider {
        AiProvider::OpenAi => {
            let mut msgs = vec![serde_json::json!({"role":"system","content":sys})];
            for m in &messages { msgs.push(serde_json::json!({"role":&m.role,"content":&m.content})); }
            let resp = client.post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&serde_json::json!({"model":model,"messages":msgs,"temperature":0.7}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["choices"][0]["message"]["content"].as_str().map(|s|s.to_string()).ok_or_else(||format!("応答エラー: {}",body))
        }
        AiProvider::Anthropic => {
            let mut msgs = Vec::new();
            for m in &messages { msgs.push(serde_json::json!({"role":&m.role,"content":&m.content})); }
            let resp = client.post("https://api.anthropic.com/v1/messages")
                .header("x-api-key",&api_key).header("anthropic-version","2023-06-01")
                .header("content-type","application/json")
                .json(&serde_json::json!({"model":model,"max_tokens":4096,"system":sys,"messages":msgs}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["content"][0]["text"].as_str().map(|s|s.to_string()).ok_or_else(||format!("応答エラー: {}",body))
        }
        AiProvider::Gemini => {
            let mut parts = vec![serde_json::json!({"text":sys})];
            for m in &messages { parts.push(serde_json::json!({"text":format!("{}:{}",m.role,m.content)})); }
            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",model,api_key);
            let resp = client.post(&url)
                .json(&serde_json::json!({"contents":[{"parts":parts}],"generationConfig":{"temperature":0.7,"maxOutputTokens":4096}}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["candidates"][0]["content"]["parts"][0]["text"].as_str().map(|s|s.to_string()).ok_or_else(||format!("応答エラー: {}",body))
        }
        AiProvider::Ollama => {
            let mut msgs = vec![serde_json::json!({"role":"system","content":sys})];
            for m in &messages { msgs.push(serde_json::json!({"role":&m.role,"content":&m.content})); }
            let url = format!("{}/api/chat", api_key.trim_end_matches('/'));
            let resp = client.post(&url)
                .json(&serde_json::json!({"model":model,"messages":msgs,"stream":false}))
                .send().await.map_err(|e| e.to_string())?;
            let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            body["message"]["content"].as_str().map(|s|s.to_string()).ok_or_else(||format!("応答エラー: {}",body))
        }
    }
}

#[tauri::command]
pub async fn ai_chat_stream(
    app: AppHandle, provider: AiProvider, model: String, messages: Vec<ChatMessage>,
    terminal_context: Option<String>, api_key: String, request_id: String,
) -> Result<(), String> {
    let result = ai_chat(provider, model, messages, terminal_context, api_key).await?;
    let _ = app.emit(&format!("ai://chunk/{}", request_id), &result);
    let _ = app.emit(&format!("ai://done/{}", request_id), "");
    Ok(())
}
