#[cfg(not(test))]
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
#[cfg(not(test))]
use tauri::{AppHandle, Emitter};
#[cfg(not(test))]
use tokio::time;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{oneshot, Mutex},
};
use uuid::Uuid;

#[cfg(not(test))]
use crate::external_control::service::{
    ExternalControlCredentialRequestPayload, ExternalControlLogControlRequestPayload,
};
use crate::external_control::service::{
    ExternalControlError, ExternalControlLogControlAck, ExternalControlRequest,
    ExternalControlResponse, ExternalControlService,
};

#[cfg(not(test))]
const CREDENTIAL_REQUEST_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
#[cfg(not(test))]
const LOG_CONTROL_REQUEST_TIMEOUT_MS: u64 = 30_000;

pub const CONTROL_PROTOCOL_VERSION: u32 = 2;
pub const CONTROL_UNAVAILABLE_MESSAGE: &str = "ExaTerm GUI control plane is unavailable";

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
    pub error: Option<ExternalControlError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExternalControlRequestEnvelope {
    pub protocol_version: u32,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_nonce: Option<String>,
    pub request: ExternalControlRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExternalControlResponseEnvelope {
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ExternalControlResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExternalControlError>,
}

pub async fn handle_control_connection<S>(
    service: ExternalControlService,
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
                error: Some(ExternalControlError::InvalidArguments(
                    "Unsupported ExaTerm external control protocol version".into(),
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
        let request = match read_json_line::<ExternalControlRequestEnvelope, _>(&mut reader).await {
            Ok(request) => request,
            Err(error) if error == "control connection closed" => return Ok(()),
            Err(error) => return Err(error),
        };

        let response = if request.protocol_version != CONTROL_PROTOCOL_VERSION {
            ExternalControlResponseEnvelope {
                request_id: request.request_id,
                response: None,
                error: Some(ExternalControlError::InvalidArguments(
                    "Unsupported ExaTerm external control protocol version".into(),
                )),
            }
        } else if request.session_nonce.as_deref() != Some(session_nonce.as_str()) {
            ExternalControlResponseEnvelope {
                request_id: request.request_id,
                response: None,
                error: Some(ExternalControlError::PermissionDenied(
                    "Invalid ExaTerm external control session nonce".into(),
                )),
            }
        } else {
            match service.execute(request.request).await {
                Ok(response) => ExternalControlResponseEnvelope {
                    request_id: request.request_id,
                    response: Some(response),
                    error: None,
                },
                Err(error) => ExternalControlResponseEnvelope {
                    request_id: request.request_id,
                    response: None,
                    error: Some(error),
                },
            }
        };

        write_json_line(&mut write_half, &response).await?;
    }
}

pub async fn external_control_call_over_stream<S>(
    stream: S,
    request: ExternalControlRequest,
) -> Result<ExternalControlResponse, ExternalControlError>
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
    .map_err(ExternalControlError::Internal)?;
    let handshake: ControlHandshakeResponse = read_json_line(&mut reader)
        .await
        .map_err(ExternalControlError::Internal)?;

    if let Some(error) = handshake.error {
        return Err(error);
    }
    let Some(session_nonce) = handshake.session_nonce else {
        return Err(ExternalControlError::Internal(
            "ExaTerm external control handshake did not return a session nonce".into(),
        ));
    };

    let request_id = Uuid::new_v4().to_string();
    write_json_line(
        &mut write_half,
        &ExternalControlRequestEnvelope {
            protocol_version: CONTROL_PROTOCOL_VERSION,
            request_id: request_id.clone(),
            session_nonce: Some(session_nonce),
            request,
        },
    )
    .await
    .map_err(ExternalControlError::Internal)?;
    let response: ExternalControlResponseEnvelope = read_json_line(&mut reader)
        .await
        .map_err(ExternalControlError::Internal)?;

    if response.request_id != request_id {
        return Err(ExternalControlError::Internal(
            "ExaTerm external control response ID mismatch".into(),
        ));
    }
    if let Some(error) = response.error {
        return Err(error);
    }
    response.response.ok_or_else(|| {
        ExternalControlError::Internal("ExaTerm external control response omitted result".into())
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
        return Err(error.message().into());
    }
    if handshake.session_nonce.is_none() {
        return Err("ExaTerm external control handshake did not return a session nonce".into());
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
        .map_err(|error| format!("External control read error: {error}"))?;
    if bytes == 0 {
        return Err("control connection closed".into());
    }
    serde_json::from_str(line.trim_end())
        .map_err(|error| format!("External control JSON parse error: {error}"))
}

async fn write_json_line<T, W>(writer: &mut W, value: &T) -> Result<(), String>
where
    T: Serialize,
    W: AsyncWrite + Unpin,
{
    let data = serde_json::to_vec(value)
        .map_err(|error| format!("External control JSON serialize error: {error}"))?;
    writer
        .write_all(&data)
        .await
        .map_err(|error| format!("External control write error: {error}"))?;
    writer
        .write_all(b"\n")
        .await
        .map_err(|error| format!("External control write error: {error}"))?;
    writer
        .flush()
        .await
        .map_err(|error| format!("External control flush error: {error}"))
}

#[derive(Clone, Default)]
pub struct ExternalControlCredentialState {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<Option<String>>>>>,
}

impl ExternalControlCredentialState {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(test))]
    pub(crate) async fn request_ssh_credential(
        &self,
        app: &AppHandle,
        mut payload: ExternalControlCredentialRequestPayload,
    ) -> Result<Option<String>, String> {
        let request_id = Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), sender);
        payload.request_id = request_id.clone();

        if let Err(error) = app.emit("external-control://credential-request", &payload) {
            self.pending.lock().await.remove(&request_id);
            return Err(format!(
                "Failed to send the external control credential prompt request: {error}"
            ));
        }

        match time::timeout(
            Duration::from_millis(CREDENTIAL_REQUEST_TIMEOUT_MS),
            receiver,
        )
        .await
        {
            Ok(Ok(credential)) => Ok(credential),
            Ok(Err(_)) => {
                Err("The external control credential prompt request did not complete".into())
            }
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err("The external control credential prompt timed out".into())
            }
        }
    }

    async fn submit(&self, request_id: String, credential: Option<String>) -> Result<(), String> {
        let sender = self
            .pending
            .lock()
            .await
            .remove(&request_id)
            .ok_or_else(|| {
                "The external control credential prompt request was not found".to_string()
            })?;
        sender.send(credential).map_err(|_| {
            "The external control credential prompt request has already finished".to_string()
        })
    }
}

#[tauri::command]
pub async fn external_control_credential_submit(
    state: tauri::State<'_, ExternalControlCredentialState>,
    request_id: String,
    credential: Option<String>,
) -> Result<(), String> {
    state.submit(request_id, credential).await
}

#[derive(Clone, Default)]
pub struct ExternalControlLogControlState {
    pending:
        Arc<Mutex<HashMap<String, oneshot::Sender<Result<ExternalControlLogControlAck, String>>>>>,
}

impl ExternalControlLogControlState {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(test))]
    pub(crate) async fn request(
        &self,
        app: &AppHandle,
        event: &str,
        payload: ExternalControlLogControlRequestPayload,
    ) -> Result<ExternalControlLogControlAck, String> {
        let request_id = payload.request_id.clone();
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id.clone(), sender);

        if let Err(error) = app.emit(event, &payload) {
            self.pending.lock().await.remove(&request_id);
            return Err(format!(
                "Failed to send the external control log control request: {error}"
            ));
        }

        match time::timeout(
            Duration::from_millis(LOG_CONTROL_REQUEST_TIMEOUT_MS),
            receiver,
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("The external control log control request did not complete".into()),
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                Err("The external control log control request timed out".into())
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
            .ok_or_else(|| "The external control log control request was not found".to_string())?;
        let result = match error {
            Some(error) => Err(error),
            None => Ok(ExternalControlLogControlAck { file_path }),
        };
        sender.send(result).map_err(|_| {
            "The external control log control request has already finished".to_string()
        })
    }
}

#[tauri::command]
pub async fn external_control_log_control_submit(
    state: tauri::State<'_, ExternalControlLogControlState>,
    request_id: String,
    file_path: Option<String>,
    error: Option<String>,
) -> Result<(), String> {
    state.submit(request_id, file_path, error).await
}
