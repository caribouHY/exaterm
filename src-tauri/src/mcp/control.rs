#[cfg(not(test))]
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};

use rmcp::ErrorData as McpError;
use serde_json::Value;
#[cfg(not(test))]
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
#[cfg(not(test))]
use tokio::time;
#[cfg(not(test))]
use uuid::Uuid;

use crate::mcp::backend::{InProcessMcpBackend, McpBackend, McpLogControlAck, McpRuntime};
#[cfg(not(test))]
use crate::mcp::backend::{McpCredentialRequestPayload, McpLogControlRequestPayload};

#[cfg(not(test))]
const CREDENTIAL_REQUEST_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
#[cfg(not(test))]
const LOG_CONTROL_REQUEST_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone)]
pub struct McpControlService {
    backend: Arc<dyn McpBackend>,
}

impl McpControlService {
    pub fn new<B>(backend: B) -> Self
    where
        B: McpBackend + 'static,
    {
        Self {
            backend: Arc::new(backend),
        }
    }

    pub fn in_process(runtime: McpRuntime) -> Self {
        Self::new(InProcessMcpBackend::new(runtime))
    }

    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        self.backend.call_tool(name, args).await
    }
}

#[derive(Clone, Default)]
pub struct McpCredentialState {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Option<String>>>>>,
}

impl McpCredentialState {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(test))]
    pub(super) async fn request_ssh_credential(
        &self,
        app: &AppHandle,
        mut payload: McpCredentialRequestPayload,
    ) -> Result<Option<String>, String> {
        let request_id = Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), sender);
        payload.request_id = request_id.clone();

        if let Err(error) = app.emit("mcp://credential-request", &payload) {
            self.pending.lock().await.remove(&request_id);
            return Err(format!("MCP認証入力リクエスト送信エラー: {error}"));
        }

        match time::timeout(
            Duration::from_millis(CREDENTIAL_REQUEST_TIMEOUT_MS),
            receiver,
        )
        .await
        {
            Ok(Ok(credential)) => Ok(credential),
            Ok(Err(_)) => Err("MCP認証入力リクエストが完了しませんでした".into()),
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err("MCP認証入力がタイムアウトしました".into())
            }
        }
    }

    async fn submit(&self, request_id: String, credential: Option<String>) -> Result<(), String> {
        let sender = self
            .pending
            .lock()
            .await
            .remove(&request_id)
            .ok_or_else(|| "MCP認証入力リクエストが見つかりません".to_string())?;
        sender
            .send(credential)
            .map_err(|_| "MCP認証入力リクエストはすでに終了しています".to_string())
    }
}

#[tauri::command]
pub async fn mcp_credential_submit(
    state: tauri::State<'_, McpCredentialState>,
    request_id: String,
    credential: Option<String>,
) -> Result<(), String> {
    state.submit(request_id, credential).await
}

#[derive(Clone, Default)]
pub struct McpLogControlState {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Result<McpLogControlAck, String>>>>>,
}

impl McpLogControlState {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(test))]
    pub(super) async fn request(
        &self,
        app: &AppHandle,
        event: &str,
        payload: McpLogControlRequestPayload,
    ) -> Result<McpLogControlAck, String> {
        let request_id = payload.request_id.clone();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), sender);

        if let Err(error) = app.emit(event, &payload) {
            self.pending.lock().await.remove(&request_id);
            return Err(format!("MCPログ制御リクエスト送信エラー: {error}"));
        }

        match time::timeout(
            Duration::from_millis(LOG_CONTROL_REQUEST_TIMEOUT_MS),
            receiver,
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("MCPログ制御リクエストが完了しませんでした".into()),
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err("MCPログ制御リクエストがタイムアウトしました".into())
            }
        }
    }

    async fn submit(
        &self,
        request_id: String,
        file_path: Option<String>,
        error: Option<String>,
    ) -> Result<(), String> {
        let sender = self
            .pending
            .lock()
            .await
            .remove(&request_id)
            .ok_or_else(|| "MCPログ制御リクエストが見つかりません".to_string())?;
        let result = match error {
            Some(error) => Err(error),
            None => Ok(McpLogControlAck { file_path }),
        };
        sender
            .send(result)
            .map_err(|_| "MCPログ制御リクエストはすでに終了しています".to_string())
    }
}

#[tauri::command]
pub async fn mcp_log_control_submit(
    state: tauri::State<'_, McpLogControlState>,
    request_id: String,
    file_path: Option<String>,
    error: Option<String>,
) -> Result<(), String> {
    state.submit(request_id, file_path, error).await
}
