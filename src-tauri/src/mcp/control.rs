#[cfg(not(test))]
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};

use rmcp::{model::ErrorCode, ErrorData as McpError};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
#[cfg(not(test))]
use tauri::{AppHandle, Emitter};
#[cfg(not(test))]
use tokio::time;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{oneshot, Mutex},
};
use uuid::Uuid;

use crate::mcp::backend::{InProcessMcpBackend, McpBackend, McpLogControlAck, McpRuntime};
#[cfg(not(test))]
use crate::mcp::backend::{McpCredentialRequestPayload, McpLogControlRequestPayload};

#[cfg(not(test))]
const CREDENTIAL_REQUEST_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
#[cfg(not(test))]
const LOG_CONTROL_REQUEST_TIMEOUT_MS: u64 = 30_000;

pub const CONTROL_PROTOCOL_VERSION: u32 = 1;
pub const CONTROL_UNAVAILABLE_MESSAGE: &str = "ExaTerm GUI control plane is unavailable";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlHandshakeRequest {
    pub protocol_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlHandshakeResponse {
    pub protocol_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ControlError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControlToolCallRequest {
    pub protocol_version: u32,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_nonce: Option<String>,
    pub tool_name: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ControlToolCallResponse {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ControlError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlError {
    pub code: i32,
    pub message: String,
}

impl ControlError {
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::INVALID_REQUEST.0,
            message: message.into(),
        }
    }

    fn from_mcp_error(error: McpError) -> Self {
        Self {
            code: error.code.0,
            message: error.message.into_owned(),
        }
    }

    pub fn into_mcp_error(self) -> McpError {
        match ErrorCode(self.code) {
            ErrorCode::INVALID_PARAMS => McpError::invalid_params(self.message, None),
            ErrorCode::INVALID_REQUEST => McpError::invalid_request(self.message, None),
            _ => McpError::internal_error(self.message, None),
        }
    }
}

pub async fn handle_control_connection<S>(
    control: McpControlService,
    stream: S,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);
    let handshake: ControlHandshakeRequest = read_json_line(&mut reader).await?;

    if handshake.protocol_version != CONTROL_PROTOCOL_VERSION {
        write_json_line(
            &mut write_half,
            &ControlHandshakeResponse {
                protocol_version: CONTROL_PROTOCOL_VERSION,
                session_nonce: None,
                error: Some(ControlError::invalid_request(
                    "Unsupported ExaTerm MCP control protocol version",
                )),
            },
        )
        .await?;
        return Ok(());
    }

    let session_nonce = Uuid::new_v4().to_string();
    write_json_line(
        &mut write_half,
        &ControlHandshakeResponse {
            protocol_version: CONTROL_PROTOCOL_VERSION,
            session_nonce: Some(session_nonce.clone()),
            error: None,
        },
    )
    .await?;

    loop {
        let request = match read_json_line::<ControlToolCallRequest, _>(&mut reader).await {
            Ok(request) => request,
            Err(error) if error == "control connection closed" => return Ok(()),
            Err(error) => return Err(error),
        };

        let response = if request.protocol_version != CONTROL_PROTOCOL_VERSION {
            ControlToolCallResponse {
                request_id: request.request_id,
                result: None,
                error: Some(ControlError::invalid_request(
                    "Unsupported ExaTerm MCP control protocol version",
                )),
            }
        } else if request.session_nonce.as_deref() != Some(session_nonce.as_str()) {
            ControlToolCallResponse {
                request_id: request.request_id,
                result: None,
                error: Some(ControlError::invalid_request(
                    "Invalid ExaTerm MCP control session nonce",
                )),
            }
        } else {
            match control.call_tool(&request.tool_name, request.args).await {
                Ok(result) => ControlToolCallResponse {
                    request_id: request.request_id,
                    result: Some(result),
                    error: None,
                },
                Err(error) => ControlToolCallResponse {
                    request_id: request.request_id,
                    result: None,
                    error: Some(ControlError::from_mcp_error(error)),
                },
            }
        };

        write_json_line(&mut write_half, &response).await?;
    }
}

pub async fn control_call_over_stream<S>(
    stream: S,
    tool_name: &str,
    args: Value,
) -> Result<Value, McpError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);
    write_json_line(
        &mut write_half,
        &ControlHandshakeRequest {
            protocol_version: CONTROL_PROTOCOL_VERSION,
        },
    )
    .await
    .map_err(|error| McpError::internal_error(error, None))?;
    let handshake: ControlHandshakeResponse = read_json_line(&mut reader)
        .await
        .map_err(|error| McpError::internal_error(error, None))?;

    if let Some(error) = handshake.error {
        return Err(error.into_mcp_error());
    }
    let Some(session_nonce) = handshake.session_nonce else {
        return Err(McpError::internal_error(
            "ExaTerm MCP control handshake did not return a session nonce",
            None,
        ));
    };

    let request_id = Uuid::new_v4().to_string();
    write_json_line(
        &mut write_half,
        &ControlToolCallRequest {
            protocol_version: CONTROL_PROTOCOL_VERSION,
            request_id: request_id.clone(),
            session_nonce: Some(session_nonce),
            tool_name: tool_name.to_string(),
            args,
        },
    )
    .await
    .map_err(|error| McpError::internal_error(error, None))?;
    let response: ControlToolCallResponse = read_json_line(&mut reader)
        .await
        .map_err(|error| McpError::internal_error(error, None))?;

    if response.request_id != request_id {
        return Err(McpError::internal_error(
            "ExaTerm MCP control response ID mismatch",
            None,
        ));
    }
    if let Some(error) = response.error {
        return Err(error.into_mcp_error());
    }
    response.result.ok_or_else(|| {
        McpError::internal_error("ExaTerm MCP control response omitted result", None)
    })
}

pub async fn control_probe_over_stream<S>(stream: S) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (read_half, mut write_half) = tokio::io::split(stream);
    let mut reader = BufReader::new(read_half);
    write_json_line(
        &mut write_half,
        &ControlHandshakeRequest {
            protocol_version: CONTROL_PROTOCOL_VERSION,
        },
    )
    .await?;
    let handshake: ControlHandshakeResponse = read_json_line(&mut reader).await?;
    if let Some(error) = handshake.error {
        return Err(error.message);
    }
    if handshake.session_nonce.is_none() {
        return Err("ExaTerm MCP control handshake did not return a session nonce".into());
    }
    Ok(())
}

async fn read_json_line<T, R>(reader: &mut R) -> Result<T, String>
where
    T: DeserializeOwned,
    R: AsyncBufRead + Unpin,
{
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .await
        .map_err(|error| format!("MCP control read error: {error}"))?;
    if bytes == 0 {
        return Err("control connection closed".into());
    }
    serde_json::from_str(line.trim_end())
        .map_err(|error| format!("MCP control JSON parse error: {error}"))
}

async fn write_json_line<T, W>(writer: &mut W, value: &T) -> Result<(), String>
where
    T: Serialize,
    W: AsyncWrite + Unpin,
{
    let data = serde_json::to_vec(value)
        .map_err(|error| format!("MCP control JSON serialize error: {error}"))?;
    writer
        .write_all(&data)
        .await
        .map_err(|error| format!("MCP control write error: {error}"))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|error| format!("MCP control write error: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("MCP control flush error: {error}"))
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
