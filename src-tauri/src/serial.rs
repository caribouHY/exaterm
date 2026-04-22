use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    pub flow_control: String,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self { baud_rate: 9600, data_bits: 8, parity: "none".into(), stop_bits: 1, flow_control: "none".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub name: String,
    pub port_type: String,
}

struct SerialSession {
    session_id: String,
    running: Arc<Mutex<bool>>,
    writer: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
}

pub struct SerialState {
    sessions: Arc<Mutex<HashMap<String, SerialSession>>>,
}

impl SerialState {
    pub fn new() -> Self {
        Self { sessions: Arc::new(Mutex::new(HashMap::new())) }
    }
}

fn to_data_bits(b: u8) -> serialport::DataBits {
    match b { 5 => serialport::DataBits::Five, 6 => serialport::DataBits::Six, 7 => serialport::DataBits::Seven, _ => serialport::DataBits::Eight }
}
fn to_parity(p: &str) -> serialport::Parity {
    match p { "odd" => serialport::Parity::Odd, "even" => serialport::Parity::Even, _ => serialport::Parity::None }
}
fn to_stop_bits(b: u8) -> serialport::StopBits {
    match b { 2 => serialport::StopBits::Two, _ => serialport::StopBits::One }
}
fn to_flow_control(f: &str) -> serialport::FlowControl {
    match f { "software" => serialport::FlowControl::Software, "hardware" => serialport::FlowControl::Hardware, _ => serialport::FlowControl::None }
}

#[tauri::command]
pub fn serial_list_ports() -> Result<Vec<PortInfo>, String> {
    let ports = serialport::available_ports().map_err(|e| format!("ポート一覧取得エラー: {}", e))?;
    Ok(ports.into_iter().map(|p| PortInfo { name: p.port_name, port_type: format!("{:?}", p.port_type) }).collect())
}

#[tauri::command]
pub async fn serial_connect(
    app: AppHandle, state: tauri::State<'_, SerialState>, port: String, config: SerialConfig,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let running = Arc::new(Mutex::new(true));

    let serial_port = serialport::new(&port, config.baud_rate)
        .data_bits(to_data_bits(config.data_bits))
        .parity(to_parity(&config.parity))
        .stop_bits(to_stop_bits(config.stop_bits))
        .flow_control(to_flow_control(&config.flow_control))
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| format!("シリアルポートオープンエラー: {}", e))?;

    let writer = Arc::new(Mutex::new(serial_port.try_clone().map_err(|e| format!("ポート複製エラー: {}", e))?));

    let session = SerialSession {
        session_id: session_id.clone(),
        running: running.clone(),
        writer: writer.clone(),
    };
    state.sessions.lock().await.insert(session_id.clone(), session);

    // Background read loop
    let sid = session_id.clone();
    let app_clone = app.clone();
    let run_flag = running.clone();
    tokio::task::spawn_blocking(move || {
        let mut port = serial_port;
        let mut buf = [0u8; 4096];
        loop {
            let rt = tokio::runtime::Handle::current();
            let is_running = rt.block_on(async { *run_flag.lock().await });
            if !is_running { break; }
            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    let _ = app_clone.emit(&format!("serial://data/{}", sid), buf[..n].to_vec());
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

    let _ = app.emit("serial://connected", &session_id);
    Ok(session_id)
}

#[tauri::command]
pub async fn serial_write(
    state: tauri::State<'_, SerialState>, session_id: String, data: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().await;
    let session = sessions.get(&session_id).ok_or("セッションが見つかりません")?;
    let mut writer = session.writer.lock().await;
    writer.write_all(data.as_bytes()).map_err(|e| format!("送信エラー: {}", e))?;
    writer.flush().map_err(|e| format!("フラッシュエラー: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn serial_disconnect(
    app: AppHandle, state: tauri::State<'_, SerialState>, session_id: String,
) -> Result<(), String> {
    let mut sessions = state.sessions.lock().await;
    if let Some(session) = sessions.remove(&session_id) {
        *session.running.lock().await = false;
    }
    let _ = app.emit("serial://disconnected", &session_id);
    Ok(())
}
