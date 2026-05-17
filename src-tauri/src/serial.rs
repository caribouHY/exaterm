use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::terminal_control::{TerminalControlState, TerminalProtocol};

const SERIAL_IO_TIMEOUT: Duration = Duration::from_millis(5);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    pub flow_control: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub name: String,
    pub port_type: String,
}

struct SerialSession {
    running: Arc<AtomicBool>,
    writer: mpsc::Sender<Vec<u8>>,
}

#[derive(Clone)]
pub struct SerialState {
    sessions: Arc<Mutex<HashMap<String, SerialSession>>>,
}

impl SerialState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
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

#[tauri::command]
pub fn serial_list_ports() -> Result<Vec<PortInfo>, String> {
    let ports =
        serialport::available_ports().map_err(|e| format!("ポート一覧取得エラー: {}", e))?;
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

#[tauri::command]
pub async fn serial_connect(
    app: AppHandle,
    state: tauri::State<'_, SerialState>,
    terminals: tauri::State<'_, TerminalControlState>,
    port: String,
    config: SerialConfig,
) -> Result<String, String> {
    connect(&app, &state, &terminals, port, config).await
}

pub async fn connect(
    app: &AppHandle,
    state: &SerialState,
    terminals: &TerminalControlState,
    port: String,
    config: SerialConfig,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let running = Arc::new(AtomicBool::new(true));

    let serial_port = serialport::new(&port, config.baud_rate)
        .data_bits(to_data_bits(config.data_bits))
        .parity(to_parity(&config.parity))
        .stop_bits(to_stop_bits(config.stop_bits))
        .flow_control(to_flow_control(&config.flow_control))
        .timeout(SERIAL_IO_TIMEOUT)
        .open()
        .map_err(|e| format!("シリアルポートオープンエラー: {}", e))?;

    let mut writer_port = serial_port
        .try_clone()
        .map_err(|e| format!("ポート複製エラー: {}", e))?;
    let (writer, write_rx) = mpsc::channel::<Vec<u8>>();

    let session = SerialSession {
        running: running.clone(),
        writer: writer.clone(),
    };
    state
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), session);

    let write_sid = session_id.clone();
    let write_app = app.clone();
    tokio::task::spawn_blocking(move || {
        while let Ok(data) = write_rx.recv() {
            if let Err(e) = writer_port.write_all(&data) {
                let _ = write_app.emit(&format!("serial://error/{}", write_sid), e.to_string());
                break;
            }
        }
    });

    // Background read loop
    let sid = session_id.clone();
    let app_clone = app.clone();
    let run_flag = running.clone();
    let (output_tx, mut output_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    let output_terminals = terminals.clone();
    let output_sid = session_id.clone();
    tokio::spawn(async move {
        while let Some(data) = output_rx.recv().await {
            output_terminals.append_output(&output_sid, &data).await;
        }
    });

    tokio::task::spawn_blocking(move || {
        let mut port = serial_port;
        let mut buf = [0u8; 4096];
        loop {
            if !run_flag.load(Ordering::SeqCst) {
                break;
            }
            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let data = buf[..n].to_vec();
                    let _ = output_tx.send(data.clone());
                    let _ = app_clone.emit(&format!("serial://data/{}", sid), data);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => {
                    let _ = app_clone.emit(&format!("serial://error/{}", sid), e.to_string());
                    break;
                }
                _ => {}
            }
        }
    });

    terminals
        .register_session(session_id.clone(), TerminalProtocol::Serial, port)
        .await;

    let _ = app.emit("serial://connected", &session_id);
    Ok(session_id)
}

#[tauri::command]
pub async fn serial_write(
    state: tauri::State<'_, SerialState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    write_data(&state, &session_id, data).await
}

pub async fn write_data(state: &SerialState, session_id: &str, data: String) -> Result<(), String> {
    let writer = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(session_id)
            .ok_or("セッションが見つかりません")?
            .writer
            .clone()
    };

    writer
        .send(data.into_bytes())
        .map_err(|e| format!("送信エラー: {}", e))
}

#[tauri::command]
pub async fn serial_disconnect(
    app: AppHandle,
    state: tauri::State<'_, SerialState>,
    terminals: tauri::State<'_, TerminalControlState>,
    session_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    if let Some(session) = sessions.remove(&session_id) {
        session.running.store(false, Ordering::SeqCst);
    }
    terminals.mark_disconnected(&session_id).await;
    let _ = app.emit("serial://disconnected", &session_id);
    Ok(())
}
