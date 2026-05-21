use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use crate::terminal_control::{TerminalControlState, TerminalProtocol};
use crate::{logger, logger::LoggerState};

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

const OPT_ECHO: u8 = 1;
const OPT_SUPPRESS_GO_AHEAD: u8 = 3;
const OPT_TERMINAL_TYPE: u8 = 24;
const OPT_NAWS: u8 = 31;

const TERMINAL_TYPE_IS: u8 = 0;
const TERMINAL_TYPE_SEND: u8 = 1;

struct TelnetSession {
    writer: mpsc::Sender<Vec<u8>>,
    read_task: JoinHandle<()>,
    write_task: JoinHandle<()>,
}

#[derive(Clone)]
pub struct TelnetState {
    sessions: Arc<Mutex<HashMap<String, TelnetSession>>>,
}

impl TelnetState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParseState {
    Data,
    Iac,
    Negotiation(u8),
    Subnegotiation,
    SubnegotiationIac,
}

#[derive(Debug, Clone)]
struct TelnetParser {
    state: ParseState,
    subnegotiation: Vec<u8>,
    cols: u16,
    rows: u16,
}

impl TelnetParser {
    fn new(cols: u32, rows: u32) -> Self {
        Self {
            state: ParseState::Data,
            subnegotiation: Vec::new(),
            cols: clamp_dimension(cols),
            rows: clamp_dimension(rows),
        }
    }

    fn parse(&mut self, input: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let mut data = Vec::new();
        let mut response = Vec::new();

        for &byte in input {
            match self.state.clone() {
                ParseState::Data => {
                    if byte == IAC {
                        self.state = ParseState::Iac;
                    } else {
                        data.push(byte);
                    }
                }
                ParseState::Iac => match byte {
                    IAC => {
                        data.push(IAC);
                        self.state = ParseState::Data;
                    }
                    DO | DONT | WILL | WONT => {
                        self.state = ParseState::Negotiation(byte);
                    }
                    SB => {
                        self.subnegotiation.clear();
                        self.state = ParseState::Subnegotiation;
                    }
                    SE => {
                        self.state = ParseState::Data;
                    }
                    _ => {
                        self.state = ParseState::Data;
                    }
                },
                ParseState::Negotiation(command) => {
                    response.extend(negotiate(command, byte, self.cols, self.rows));
                    self.state = ParseState::Data;
                }
                ParseState::Subnegotiation => {
                    if byte == IAC {
                        self.state = ParseState::SubnegotiationIac;
                    } else {
                        self.subnegotiation.push(byte);
                    }
                }
                ParseState::SubnegotiationIac => {
                    if byte == IAC {
                        self.subnegotiation.push(IAC);
                        self.state = ParseState::Subnegotiation;
                    } else if byte == SE {
                        response.extend(handle_subnegotiation(
                            &self.subnegotiation,
                            self.cols,
                            self.rows,
                        ));
                        self.subnegotiation.clear();
                        self.state = ParseState::Data;
                    } else {
                        self.state = ParseState::Data;
                    }
                }
            }
        }

        (data, response)
    }
}

fn clamp_dimension(value: u32) -> u16 {
    u16::try_from(value.max(1).min(u16::MAX as u32)).unwrap_or(u16::MAX)
}

fn command_response(command: u8, option: u8) -> Vec<u8> {
    vec![IAC, command, option]
}

fn negotiate(command: u8, option: u8, cols: u16, rows: u16) -> Vec<u8> {
    match command {
        WILL => match option {
            OPT_ECHO | OPT_SUPPRESS_GO_AHEAD => command_response(DO, option),
            _ => command_response(DONT, option),
        },
        DO => match option {
            OPT_SUPPRESS_GO_AHEAD | OPT_TERMINAL_TYPE => command_response(WILL, option),
            OPT_NAWS => {
                let mut response = command_response(WILL, option);
                response.extend(build_naws(cols, rows));
                response
            }
            _ => command_response(WONT, option),
        },
        WONT => command_response(DONT, option),
        DONT => command_response(WONT, option),
        _ => Vec::new(),
    }
}

fn handle_subnegotiation(data: &[u8], cols: u16, rows: u16) -> Vec<u8> {
    if data == [OPT_TERMINAL_TYPE, TERMINAL_TYPE_SEND] {
        build_terminal_type()
    } else if data.first() == Some(&OPT_NAWS) {
        build_naws(cols, rows)
    } else {
        Vec::new()
    }
}

fn build_terminal_type() -> Vec<u8> {
    let mut response = vec![IAC, SB, OPT_TERMINAL_TYPE, TERMINAL_TYPE_IS];
    response.extend(b"xterm-256color");
    response.extend([IAC, SE]);
    response
}

fn build_naws(cols: u16, rows: u16) -> Vec<u8> {
    let [cols_hi, cols_lo] = cols.to_be_bytes();
    let [rows_hi, rows_lo] = rows.to_be_bytes();
    vec![
        IAC, SB, OPT_NAWS, cols_hi, cols_lo, rows_hi, rows_lo, IAC, SE,
    ]
}

fn escape_user_data(data: String) -> Vec<u8> {
    escape_iac_bytes(data.into_bytes())
}

fn escape_iac_bytes(data: Vec<u8>) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(data.len());
    for byte in data {
        escaped.push(byte);
        if byte == IAC {
            escaped.push(IAC);
        }
    }
    escaped
}

async fn remove_session(
    app: &AppHandle,
    terminals: &TerminalControlState,
    logger_state: Option<&LoggerState>,
    sessions: &Arc<Mutex<HashMap<String, TelnetSession>>>,
    session_id: &str,
) -> Option<TelnetSession> {
    let session = sessions.lock().await.remove(session_id);
    if session.is_some() {
        terminals.mark_disconnected(session_id).await;
        let _ = app.emit("telnet://disconnected", session_id);
    }
    if let Some(logger_state) = logger_state {
        logger::clear_session_logs(logger_state, session_id).await;
    }
    session
}

async fn mark_disconnected(
    app: &AppHandle,
    terminals: &TerminalControlState,
    logger_state: Option<&LoggerState>,
    sessions: &Arc<Mutex<HashMap<String, TelnetSession>>>,
    session_id: &str,
) {
    let _ = remove_session(app, terminals, logger_state, sessions, session_id).await;
}

#[tauri::command]
pub async fn telnet_connect(
    app: AppHandle,
    state: tauri::State<'_, TelnetState>,
    terminals: tauri::State<'_, TerminalControlState>,
    logger: tauri::State<'_, LoggerState>,
    host: String,
    port: u16,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    connect(
        &app,
        &state,
        &terminals,
        Some(&logger),
        host,
        port,
        cols,
        rows,
    )
    .await
}

pub async fn connect(
    app: &AppHandle,
    state: &TelnetState,
    terminals: &TerminalControlState,
    logger_state: Option<&LoggerState>,
    host: String,
    port: u16,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let stream = TcpStream::connect((host.as_str(), port))
        .await
        .map_err(|e| format!("Telnet接続エラー: {}", e))?;
    let (mut reader, mut writer_stream) = stream.into_split();
    let (writer, mut write_rx) = mpsc::channel::<Vec<u8>>(64);

    let write_sid = session_id.clone();
    let write_app = app.clone();
    let write_sessions = state.sessions.clone();
    let write_terminals = terminals.clone();
    let write_logger = logger_state.cloned();
    let write_task = tokio::spawn(async move {
        while let Some(data) = write_rx.recv().await {
            if let Err(e) = writer_stream.write_all(&data).await {
                let _ = write_app.emit(&format!("telnet://error/{}", write_sid), e.to_string());
                mark_disconnected(
                    &write_app,
                    &write_terminals,
                    write_logger.as_ref(),
                    &write_sessions,
                    &write_sid,
                )
                .await;
                break;
            }
        }
    });

    let read_sid = session_id.clone();
    let read_app = app.clone();
    let read_sessions = state.sessions.clone();
    let read_terminals = terminals.clone();
    let read_logger = logger_state.cloned();
    let read_task = tokio::spawn(async move {
        let mut parser = TelnetParser::new(cols, rows);
        let mut buf = [0u8; 4096];

        loop {
            match reader.read(&mut buf).await {
                Ok(0) => {
                    mark_disconnected(
                        &read_app,
                        &read_terminals,
                        read_logger.as_ref(),
                        &read_sessions,
                        &read_sid,
                    )
                    .await;
                    break;
                }
                Ok(n) => {
                    let (data, response) = parser.parse(&buf[..n]);
                    if !data.is_empty() {
                        read_terminals.append_output(&read_sid, &data).await;
                        let _ = read_app.emit(&format!("telnet://data/{}", read_sid), data);
                    }
                    if !response.is_empty() {
                        let writer = {
                            let sessions = read_sessions.lock().await;
                            sessions
                                .get(&read_sid)
                                .map(|session| session.writer.clone())
                        };
                        if let Some(writer) = writer {
                            let _ = writer.send(response).await;
                        }
                    }
                }
                Err(e) => {
                    let _ = read_app.emit(&format!("telnet://error/{}", read_sid), e.to_string());
                    mark_disconnected(
                        &read_app,
                        &read_terminals,
                        read_logger.as_ref(),
                        &read_sessions,
                        &read_sid,
                    )
                    .await;
                    break;
                }
            }
        }
    });

    state.sessions.lock().await.insert(
        session_id.clone(),
        TelnetSession {
            writer: writer.clone(),
            read_task,
            write_task,
        },
    );

    terminals
        .register_session(
            session_id.clone(),
            TerminalProtocol::Telnet,
            format!("{}:{}", host, port),
        )
        .await;

    let _ = app.emit("telnet://connected", &session_id);
    Ok(session_id)
}

#[tauri::command]
pub async fn telnet_write(
    state: tauri::State<'_, TelnetState>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    write_data(&state, &session_id, data).await
}

pub async fn write_data(state: &TelnetState, session_id: &str, data: String) -> Result<(), String> {
    let writer = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(session_id)
            .ok_or("セッションが見つかりません")?
            .writer
            .clone()
    };

    writer
        .send(escape_user_data(data))
        .await
        .map_err(|e| format!("送信エラー: {}", e))
}

#[tauri::command]
pub async fn telnet_resize(
    state: tauri::State<'_, TelnetState>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let writer = {
        let sessions = state.sessions.lock().await;
        sessions
            .get(&session_id)
            .ok_or("セッションが見つかりません")?
            .writer
            .clone()
    };

    writer
        .send(build_naws(clamp_dimension(cols), clamp_dimension(rows)))
        .await
        .map_err(|e| format!("リサイズ送信エラー: {}", e))
}

#[tauri::command]
pub async fn telnet_disconnect(
    app: AppHandle,
    state: tauri::State<'_, TelnetState>,
    terminals: tauri::State<'_, TerminalControlState>,
    logger: tauri::State<'_, LoggerState>,
    session_id: String,
) -> Result<(), String> {
    if let Some(session) = remove_session(
        &app,
        &terminals,
        Some(&logger),
        &state.sessions,
        &session_id,
    )
    .await
    {
        session.read_task.abort();
        session.write_task.abort();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_passes_plain_data_through() {
        let mut parser = TelnetParser::new(120, 30);
        let (data, response) = parser.parse(b"login: ");

        assert_eq!(data, b"login: ");
        assert!(response.is_empty());
    }

    #[test]
    fn parser_negotiates_supported_and_unsupported_options() {
        let mut parser = TelnetParser::new(120, 30);
        let (_data, response) = parser.parse(&[
            IAC, WILL, OPT_ECHO, IAC, DO, OPT_NAWS, IAC, DO, 99, IAC, WILL, 100,
        ]);

        let mut expected = vec![IAC, DO, OPT_ECHO, IAC, WILL, OPT_NAWS];
        expected.extend(build_naws(120, 30));
        expected.extend([IAC, WONT, 99, IAC, DONT, 100]);
        assert_eq!(response, expected);
    }

    #[test]
    fn parser_responds_to_terminal_type_send() {
        let mut parser = TelnetParser::new(120, 30);
        let (_data, response) =
            parser.parse(&[IAC, SB, OPT_TERMINAL_TYPE, TERMINAL_TYPE_SEND, IAC, SE]);

        assert_eq!(response, build_terminal_type());
    }

    #[test]
    fn builds_naws_payload() {
        assert_eq!(
            build_naws(120, 30),
            vec![IAC, SB, OPT_NAWS, 0, 120, 0, 30, IAC, SE]
        );
    }

    #[test]
    fn user_data_escapes_iac_bytes() {
        assert_eq!(
            escape_iac_bytes(vec![b'a', IAC, b'b']),
            vec![b'a', IAC, IAC, b'b']
        );
    }
}
