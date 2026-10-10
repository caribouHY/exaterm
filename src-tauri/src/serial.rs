use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::connect_attempt::{run_with_attempt, ConnectAttempt, ConnectAttemptState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol};
use crate::workspace::{emit_workspace_updated, WorkspaceState};
use crate::{logger, logger::LoggerState};

mod lifecycle;
mod writer;

use lifecycle::{
    enqueue_data, request_shutdown, spawn_shutdown_coordinator, RegistrationGuard, SerialSession,
    SerialSessions, SerialShutdown,
};
use writer::{cancel_synchronous_write, finish_serial_workers, spawn_serial_writer};

pub fn list_ports() -> Result<Vec<PortInfo>, String> {
    let ports =
        serialport::available_ports().map_err(|e| format!("Failed to list serial ports: {}", e))?;
    Ok(ports
        .into_iter()
        .map(|p| {
            let port_type_str = match &p.port_type {
                serialport::SerialPortType::UsbPort(info) => {
                    info.product.clone().unwrap_or_else(|| "USB".to_string())
                }
                serialport::SerialPortType::PciPort => "PCI".to_string(),
                serialport::SerialPortType::BluetoothPort => "Bluetooth".to_string(),
                serialport::SerialPortType::Unknown => "Unknown".to_string(),
            };
            PortInfo {
                name: p.port_name,
                port_type: port_type_str,
            }
        })
        .collect())
}

const SERIAL_IO_TIMEOUT: Duration = Duration::from_millis(5);
const SERIAL_CONNECT_CANCELLED: &str = "The Serial connection attempt was cancelled";
const SERIAL_CONNECT_DUPLICATE: &str =
    "A Serial connection attempt with this request ID already exists";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    pub flow_control: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialConnectInput {
    pub port: String,
    pub config: SerialConfig,
    pub encoding: Option<String>,
    pub request_id: Option<String>,
}

pub(crate) struct SerialConnectRuntime<'a> {
    pub app: &'a AppHandle,
    pub state: &'a SerialState,
    pub terminals: &'a TerminalControlState,
    pub workspace: &'a WorkspaceState,
    pub logger: Option<&'a LoggerState>,
}

pub(crate) struct SerialConnectRequest {
    pub port: String,
    pub config: SerialConfig,
    pub encoding: Option<String>,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            baud_rate: 9600,
            data_bits: 8,
            parity: "none".into(),
            stop_bits: 1,
            flow_control: "none".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortInfo {
    pub name: String,
    pub port_type: String,
}

#[derive(Clone)]
pub struct SerialState {
    sessions: SerialSessions,
    connect_attempts: ConnectAttemptState,
}

impl SerialState {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Arc::new(Mutex::new(HashMap::new())),
            connect_attempts: ConnectAttemptState::new(
                SERIAL_CONNECT_CANCELLED,
                SERIAL_CONNECT_DUPLICATE,
            ),
        }
    }
}

fn to_data_bits(b: u8) -> serialport::DataBits {
    match b {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        _ => serialport::DataBits::Eight,
    }
}
fn to_parity(p: &str) -> serialport::Parity {
    match p {
        "odd" => serialport::Parity::Odd,
        "even" => serialport::Parity::Even,
        _ => serialport::Parity::None,
    }
}
fn to_stop_bits(b: u8) -> serialport::StopBits {
    match b {
        2 => serialport::StopBits::Two,
        _ => serialport::StopBits::One,
    }
}
fn to_flow_control(f: &str) -> serialport::FlowControl {
    match f {
        "software" => serialport::FlowControl::Software,
        "hardware" => serialport::FlowControl::Hardware,
        _ => serialport::FlowControl::None,
    }
}

async fn shutdown_session(sessions: &SerialSessions, session_id: &str) -> Result<(), String> {
    if let Some(shutdown) = request_shutdown(sessions, session_id).await {
        shutdown.wait().await?;
    }
    Ok(())
}

#[tauri::command]
pub fn serial_list_ports() -> Result<Vec<PortInfo>, crate::command_error::BackendCommandError> {
    list_ports().map_err(Into::into)
}

#[tauri::command]
pub async fn serial_connect(
    app: AppHandle,
    state: tauri::State<'_, SerialState>,
    terminals: tauri::State<'_, TerminalControlState>,
    workspace: tauri::State<'_, WorkspaceState>,
    logger: tauri::State<'_, LoggerState>,
    input: SerialConnectInput,
) -> Result<String, crate::command_error::BackendCommandError> {
    let SerialConnectInput {
        port,
        config,
        encoding,
        request_id,
    } = input;
    let request_id = request_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::command_error::BackendCommandError::new(
                "serial.connect_request_id_required",
                "A Serial connection request ID is required",
            )
        })?
        .to_string();
    let attempt = state
        .connect_attempts
        .register(request_id)
        .map_err(crate::command_error::BackendCommandError::from)?;
    connect(
        SerialConnectRuntime {
            app: &app,
            state: &state,
            terminals: &terminals,
            workspace: &workspace,
            logger: Some(&logger),
        },
        SerialConnectRequest {
            port,
            config,
            encoding,
        },
        Some(attempt),
    )
    .await
    .map_err(Into::into)
}

pub(crate) async fn connect(
    runtime: SerialConnectRuntime<'_>,
    request: SerialConnectRequest,
    mut attempt: Option<ConnectAttempt>,
) -> Result<String, String> {
    let SerialConnectRuntime {
        app,
        state,
        terminals,
        workspace,
        logger: logger_state,
    } = runtime;
    let SerialConnectRequest {
        port,
        config,
        encoding,
    } = request;
    let session_id = Uuid::new_v4().to_string();

    let open_port = port.clone();
    let open_config = config.clone();
    let open_operation = async move {
        tokio::task::spawn_blocking(move || open_serial_port_pair(open_port, open_config))
            .await
            .map_err(|error| format!("Failed to open the serial port: {}", error))?
    };
    // Dropping the join handle cannot stop every platform driver, but it ensures a late result is
    // dropped without registering a session after cancellation.
    let (serial_port, writer_port) =
        run_with_attempt(attempt.as_ref(), Box::pin(open_operation)).await?;
    let control_port = serial_port
        .try_clone()
        .map_err(|error| format!("Failed to clone the serial port handle: {error}"))?;
    if attempt
        .as_mut()
        .is_some_and(|attempt| !attempt.begin_completion())
    {
        return Err(SERIAL_CONNECT_CANCELLED.to_string());
    }
    let shutdown = SerialShutdown::new();
    let running = shutdown.running.clone();
    let (writer, write_rx) = mpsc::channel::<Vec<u8>>();
    let write_sid = session_id.clone();
    let write_sessions = state.sessions.clone();
    let write_app = app.clone();
    let write_runtime = tokio::runtime::Handle::current();
    let writer_thread = spawn_serial_writer(writer_port, write_rx, running.clone(), move |error| {
        let _ = write_app.emit(&format!("serial://error/{write_sid}"), error);
        write_runtime.spawn(async move {
            request_shutdown(&write_sessions, &write_sid).await;
        });
    })
    .map_err(|error| format!("Failed to start the Serial writer: {error}"))?;

    let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();
    let output_app = app.clone();
    let output_terminals = terminals.clone();
    let output_sessions = state.sessions.clone();
    let output_sid = session_id.clone();
    let output_worker = tokio::spawn(async move {
        let app = &output_app;
        let terminals = &output_terminals;
        let sid = &output_sid;
        process_serial_output(
            output_rx,
            |data| async move {
                if let Some(output) = terminals.append_output(sid, &data).await {
                    let _ = app.emit(&format!("serial://data/{sid}"), output);
                }
            },
            |error| async move {
                let _ = app.emit(&format!("serial://error/{sid}"), error);
                // This worker must return before the coordinator can finish shutdown.
                request_shutdown(&output_sessions, sid).await;
            },
        )
        .await;
    });

    let (start_reader, reader_ready) = mpsc::channel();
    let read_running = running.clone();
    let read_sessions = state.sessions.clone();
    let read_sid = session_id.clone();
    let read_runtime = tokio::runtime::Handle::current();
    let reader_worker = tokio::task::spawn_blocking(move || {
        let mut port = serial_port;
        let mut buf = [0u8; 4096];
        if reader_ready.recv().is_err() {
            return;
        }
        while read_running.load(Ordering::SeqCst) {
            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let _ = output_tx.send(SerialReadEvent::Data(buf[..n].to_vec()));
                }
                Err(ref error) if error.kind() == std::io::ErrorKind::TimedOut => {}
                Err(error) => {
                    if read_running.load(Ordering::SeqCst) {
                        let _ = output_tx.send(SerialReadEvent::Error(error.to_string()));
                        // Stop transmission without waiting for the receive FIFO to drain.
                        read_runtime.spawn(async move {
                            request_shutdown(&read_sessions, &read_sid).await;
                        });
                    }
                    break;
                }
                _ => {}
            }
        }
    });

    let cleanup = finish_serial_workers(
        writer_thread,
        cancel_synchronous_write,
        move || {
            let result = control_port
                .clear(serialport::ClearBuffer::Output)
                .map_err(|error| format!("Failed to discard Serial output: {error}"));
            drop(control_port);
            result
        },
        reader_worker,
        output_worker,
    );
    let finalize_app = app.clone();
    let finalize_terminals = terminals.clone();
    let finalize_workspace = workspace.clone();
    let finalize_logger = logger_state.cloned();
    let finalize_sid = session_id.clone();
    spawn_shutdown_coordinator(
        state.sessions.clone(),
        session_id.clone(),
        shutdown.clone(),
        cleanup,
        move || async move {
            finalize_terminals.mark_disconnected(&finalize_sid).await;
            if let Some(snapshot) = finalize_workspace.mark_disconnected(&finalize_sid).await {
                emit_workspace_updated(&finalize_app, &snapshot);
            }
            if let Some(logger) = finalize_logger {
                logger::clear_session_logs(&logger, &finalize_sid).await;
            }
            let _ = finalize_app.emit("serial://disconnected", &finalize_sid);
            Ok(())
        },
    );
    let mut registration = RegistrationGuard(Some(shutdown.clone()));
    terminals
        .register_session_with_encoding(
            session_id.clone(),
            TerminalProtocol::Serial,
            port,
            encoding,
        )
        .await;
    state.sessions.lock().await.insert(
        session_id.clone(),
        SerialSession {
            shutdown,
            writer: Some(writer),
        },
    );
    registration.0 = None;
    let _ = start_reader.send(());

    let _ = app.emit("serial://connected", &session_id);
    Ok(session_id)
}

enum SerialReadEvent {
    Data(Vec<u8>),
    Error(String),
}

async fn process_serial_output<Data, DataFuture, Error, ErrorFuture>(
    mut output: tokio::sync::mpsc::UnboundedReceiver<SerialReadEvent>,
    mut on_data: Data,
    on_error: Error,
) where
    Data: FnMut(Vec<u8>) -> DataFuture,
    DataFuture: std::future::Future<Output = ()>,
    Error: FnOnce(String) -> ErrorFuture,
    ErrorFuture: std::future::Future<Output = ()>,
{
    while let Some(event) = output.recv().await {
        match event {
            SerialReadEvent::Data(data) => on_data(data).await,
            SerialReadEvent::Error(error) => {
                // Finish queued output before error notification and session/log teardown.
                on_error(error).await;
                break;
            }
        }
    }
}

type SerialPortPair = (
    Box<dyn serialport::SerialPort>,
    Box<dyn serialport::SerialPort>,
);

fn open_serial_port_pair(port: String, config: SerialConfig) -> Result<SerialPortPair, String> {
    let serial_port = serialport::new(&port, config.baud_rate)
        .data_bits(to_data_bits(config.data_bits))
        .parity(to_parity(&config.parity))
        .stop_bits(to_stop_bits(config.stop_bits))
        .flow_control(to_flow_control(&config.flow_control))
        .timeout(SERIAL_IO_TIMEOUT)
        .open()
        .map_err(|error| format!("Failed to open the serial port: {}", error))?;
    let writer_port = serial_port
        .try_clone()
        .map_err(|error| format!("Failed to clone the serial port handle: {}", error))?;
    Ok((serial_port, writer_port))
}

#[tauri::command]
pub async fn serial_connect_cancel(
    state: tauri::State<'_, SerialState>,
    request_id: String,
) -> Result<bool, crate::command_error::BackendCommandError> {
    Ok(state.connect_attempts.cancel(&request_id))
}

#[tauri::command]
pub async fn serial_write(
    state: tauri::State<'_, SerialState>,
    terminals: tauri::State<'_, TerminalControlState>,
    session_id: String,
    data: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    write_data(&state, terminals.inner(), &session_id, data)
        .await
        .map_err(Into::into)
}

pub async fn write_data(
    state: &SerialState,
    terminals: &TerminalControlState,
    session_id: &str,
    data: String,
) -> Result<(), String> {
    let encoded = terminals.encode_input(session_id, &data).await?;
    enqueue_data(&state.sessions, session_id, encoded).await
}

#[tauri::command]
pub async fn serial_disconnect(
    app: AppHandle,
    state: tauri::State<'_, SerialState>,
    terminals: tauri::State<'_, TerminalControlState>,
    workspace: tauri::State<'_, WorkspaceState>,
    logger: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<(), crate::command_error::BackendCommandError> {
    disconnect(
        &app,
        &state,
        &terminals,
        &workspace,
        Some(&logger),
        &session_id,
    )
    .await
    .map_err(Into::into)
}

pub(crate) async fn disconnect(
    app: &AppHandle,
    state: &SerialState,
    terminals: &TerminalControlState,
    workspace: &WorkspaceState,
    logger: Option<&LoggerState>,
    session_id: &str,
) -> Result<(), String> {
    let _ = (app, terminals, workspace, logger);
    shutdown_session(&state.sessions, session_id).await
}

#[cfg(test)]
mod tests;
