use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rmcp::ServiceExt;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    time,
};
use uuid::Uuid;

use super::service::*;
use crate::config::{AppConfig, SavedConnection};
use crate::external_control::protocol::{
    external_control_call_over_stream, handle_control_connection, ExternalControlResponseEnvelope,
    CONTROL_PROTOCOL_VERSION,
};
use crate::external_control::service::*;
use crate::external_control::{
    ExternalControlLogControlState, ExternalControlRuntime, ExternalControlService,
};
use crate::logger::{self, LoggerState};
use crate::serial::{self};
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};
use crate::workspace::{
    WorkspaceConnectionInfo, WorkspaceSnapshot, WorkspaceState, WorkspaceTabMetadataPatch,
    WorkspaceTabRegisterInput,
};

type McpTerminalService = ExternalControlService;
type McpSerialParity = ExternalControlSerialParity;
type McpSerialFlowControl = ExternalControlSerialFlowControl;
type McpTerminalMode = ExternalControlTerminalMode;

#[derive(Default)]
struct TestConfigIo {
    config: Mutex<AppConfig>,
    ports: Mutex<Vec<serial::PortInfo>>,
    calls: Mutex<Vec<&'static str>>,
    trace: Arc<Mutex<Vec<&'static str>>>,
}

impl ExternalControlConfigIo for TestConfigIo {
    fn load_config(&self) -> Result<AppConfig, String> {
        self.calls.lock().unwrap().push("load_config");
        self.trace.lock().unwrap().push("load_config");
        Ok(self.config.lock().unwrap().clone())
    }

    fn list_serial_ports(&self) -> Result<Vec<serial::PortInfo>, String> {
        self.calls.lock().unwrap().push("list_serial_ports");
        self.trace.lock().unwrap().push("list_serial_ports");
        Ok(self.ports.lock().unwrap().clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestProtocolCall {
    ConnectSsh(ExternalControlSshConnectRequest),
    ConnectTelnet(ExternalControlTelnetConnectRequest),
    ConnectSerial(ExternalControlSerialConnectRequest),
    Write(TerminalProtocol, String, String),
}

struct TestProtocolIo {
    calls: Mutex<Vec<TestProtocolCall>>,
    trace: Arc<Mutex<Vec<&'static str>>>,
    ssh_result: Mutex<Result<String, String>>,
    telnet_result: Mutex<Result<String, String>>,
    serial_result: Mutex<Result<String, String>>,
    write_result: Mutex<Result<(), String>>,
}

impl TestProtocolIo {
    fn new(trace: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            trace,
            ssh_result: Mutex::new(Ok("ssh-session".into())),
            telnet_result: Mutex::new(Ok("telnet-session".into())),
            serial_result: Mutex::new(Ok("serial-session".into())),
            write_result: Mutex::new(Ok(())),
        }
    }
}

#[async_trait]
impl ExternalControlProtocolIo for TestProtocolIo {
    async fn connect_ssh(
        &self,
        request: ExternalControlSshConnectRequest,
    ) -> Result<String, String> {
        self.trace.lock().unwrap().push("connect_ssh");
        self.calls
            .lock()
            .unwrap()
            .push(TestProtocolCall::ConnectSsh(request));
        self.ssh_result.lock().unwrap().clone()
    }

    async fn connect_telnet(
        &self,
        request: ExternalControlTelnetConnectRequest,
    ) -> Result<String, String> {
        self.trace.lock().unwrap().push("connect_telnet");
        self.calls
            .lock()
            .unwrap()
            .push(TestProtocolCall::ConnectTelnet(request));
        self.telnet_result.lock().unwrap().clone()
    }

    async fn connect_serial(
        &self,
        request: ExternalControlSerialConnectRequest,
    ) -> Result<String, String> {
        self.trace.lock().unwrap().push("connect_serial");
        self.calls
            .lock()
            .unwrap()
            .push(TestProtocolCall::ConnectSerial(request));
        self.serial_result.lock().unwrap().clone()
    }

    async fn write_terminal(
        &self,
        protocol: TerminalProtocol,
        session_id: &str,
        data: String,
    ) -> Result<(), String> {
        self.trace.lock().unwrap().push("write_terminal");
        self.calls
            .lock()
            .unwrap()
            .push(TestProtocolCall::Write(protocol, session_id.into(), data));
        self.write_result.lock().unwrap().clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestUiCall {
    Credential(ExternalControlCredentialRequestPayload),
    LogControl(String, String, ExternalControlLogControlRequestPayload),
    WorkspaceUpdated(String),
}

struct TestUiIo {
    calls: Mutex<Vec<TestUiCall>>,
    trace: Arc<Mutex<Vec<&'static str>>>,
    credential_result: Mutex<Result<Option<String>, String>>,
    private_key_requires_passphrase: Mutex<Result<bool, String>>,
    log_result: Mutex<Option<Result<ExternalControlLogControlAck, String>>>,
    logger: Option<LoggerState>,
    log_control: ExternalControlLogControlState,
}

impl TestUiIo {
    fn new(logger: Option<LoggerState>, trace: Arc<Mutex<Vec<&'static str>>>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            trace,
            credential_result: Mutex::new(Ok(None)),
            private_key_requires_passphrase: Mutex::new(Ok(false)),
            log_result: Mutex::new(None),
            logger,
            log_control: ExternalControlLogControlState::new(),
        }
    }
}

#[async_trait]
impl ExternalControlUiIo for TestUiIo {
    fn private_key_requires_passphrase(&self, _path: &str) -> Result<bool, String> {
        self.private_key_requires_passphrase.lock().unwrap().clone()
    }

    async fn request_ssh_credential(
        &self,
        payload: ExternalControlCredentialRequestPayload,
    ) -> Result<Option<String>, String> {
        self.trace.lock().unwrap().push("request_credential");
        self.calls
            .lock()
            .unwrap()
            .push(TestUiCall::Credential(payload));
        self.credential_result.lock().unwrap().clone()
    }

    async fn request_log_control(
        &self,
        window_id: &str,
        event: &str,
        payload: ExternalControlLogControlRequestPayload,
    ) -> Result<ExternalControlLogControlAck, String> {
        self.trace.lock().unwrap().push("request_log_control");
        self.calls.lock().unwrap().push(TestUiCall::LogControl(
            window_id.into(),
            event.into(),
            payload.clone(),
        ));
        if let Some(result) = self.log_result.lock().unwrap().clone() {
            return result;
        }
        let logger = self.logger.clone().ok_or_else(|| {
            "Logger state required for external control logging is unavailable".to_string()
        })?;
        let state = self.log_control.clone();
        let event = event.to_string();
        self.log_control
            .request_with_sender(payload, Duration::from_secs(1), move |payload| {
                let payload = payload.clone();
                tokio::spawn(async move {
                    let result = match event.as_str() {
                        "external-control://log-start-request" => logger::start_manual_log(
                            &logger,
                            payload.session_id,
                            payload.connection_type,
                            payload.target,
                            payload.file_path,
                            payload.write_mode.map(|mode| mode.as_str().to_string()),
                        )
                        .await
                        .map(Some),
                        "external-control://log-stop-request" => {
                            logger::stop_manual_log(&logger, &payload.session_id)
                                .await
                                .map(|_| None)
                        }
                        _ => Ok(None),
                    };
                    let (file_path, error) = match result {
                        Ok(file_path) => (file_path, None),
                        Err(error) => (None, Some(error)),
                    };
                    let _ = state.submit(payload.request_id, file_path, error).await;
                });
                Ok(())
            })
            .await
    }

    fn emit_workspace_updated(&self, snapshot: &WorkspaceSnapshot) {
        self.trace.lock().unwrap().push("emit_workspace_updated");
        self.calls
            .lock()
            .unwrap()
            .push(TestUiCall::WorkspaceUpdated(snapshot.window_id.clone()));
    }
}

struct TestLogIo {
    state: Option<LoggerState>,
    trace: Arc<Mutex<Vec<&'static str>>>,
    start_result: Mutex<Option<Result<String, String>>>,
}

#[async_trait]
impl ExternalControlLogIo for TestLogIo {
    fn is_available(&self) -> bool {
        self.state.is_some()
    }

    async fn active_log_session(&self, session_id: &str) -> Option<logger::LogSession> {
        let state = self.state.as_ref()?;
        logger::active_log_session(state, session_id).await
    }

    async fn start_auto_log(
        &self,
        session_id: String,
        connection_type: String,
        target: String,
    ) -> Result<String, String> {
        self.trace.lock().unwrap().push("start_auto_log");
        if let Some(result) = self.start_result.lock().unwrap().clone() {
            return result;
        }
        let state = self.state.as_ref().ok_or_else(|| {
            "Logger state required for external control is unavailable".to_string()
        })?;
        logger::start_log_on_connection(state, session_id, connection_type, target).await
    }
}

fn test_runtime_parts(
    config: AppConfig,
    ports: Vec<serial::PortInfo>,
    logger: Option<LoggerState>,
) -> (
    ExternalControlRuntime,
    Arc<TestConfigIo>,
    Arc<TestProtocolIo>,
    Arc<TestUiIo>,
    Arc<TestLogIo>,
    Arc<Mutex<Vec<&'static str>>>,
) {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let config_io = Arc::new(TestConfigIo {
        config: Mutex::new(config),
        ports: Mutex::new(ports),
        calls: Mutex::new(Vec::new()),
        trace: trace.clone(),
    });
    let protocol_io = Arc::new(TestProtocolIo::new(trace.clone()));
    let ui_io = Arc::new(TestUiIo::new(logger.clone(), trace.clone()));
    let log_io = Arc::new(TestLogIo {
        state: logger,
        trace: trace.clone(),
        start_result: Mutex::new(None),
    });
    let runtime = ExternalControlRuntime {
        io: ExternalControlIo::new(
            config_io.clone(),
            protocol_io.clone(),
            ui_io.clone(),
            log_io.clone(),
        ),
        terminals: TerminalControlState::new(),
        workspace: WorkspaceState::new(),
    };
    (runtime, config_io, protocol_io, ui_io, log_io, trace)
}

fn test_logger() -> LoggerState {
    let log_dir = std::env::temp_dir().join(format!("exaterm_mcp_log_test_{}", Uuid::new_v4()));
    let index_path = log_dir.join("index.json");
    LoggerState::with_paths(log_dir, index_path)
}

fn test_runtime() -> ExternalControlRuntime {
    test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger())).0
}

fn test_runtime_with_app_config(mut app_config: AppConfig) -> ExternalControlRuntime {
    app_config.external_control.connect_enabled = true;
    test_runtime_parts(app_config, Vec::new(), Some(test_logger())).0
}

fn test_runtime_with_serial_ports(ports: Vec<serial::PortInfo>) -> ExternalControlRuntime {
    let mut config = AppConfig::default();
    config.external_control.connect_enabled = true;
    test_runtime_parts(config, ports, Some(test_logger())).0
}

async fn register_test_terminal(
    runtime: &ExternalControlRuntime,
    session_id: &str,
    protocol: TerminalProtocol,
    target: &str,
) {
    runtime
        .terminals
        .register_session(session_id.into(), protocol, target.into())
        .await;
    runtime
        .workspace
        .register_tab(WorkspaceTabRegisterInput {
            window_id: None,
            tab_id: None,
            session_id: session_id.into(),
            connection_type: protocol,
            title: target.into(),
            encoding: "utf-8".into(),
            terminal_mode: "general".into(),
            connection_info: None,
            is_manual_logging: false,
            manual_log_file_path: None,
        })
        .await;
}

fn assert_response_contract(
    response: ExternalControlResponse,
    operation: &str,
    expected_result: Value,
) {
    let expected_response = json!({
        "operation": operation,
        "result": expected_result,
    });
    assert_eq!(serde_json::to_value(&response).unwrap(), expected_response);
    assert_eq!(response.clone().into_value().unwrap(), expected_result);
    assert_eq!(
        serde_json::to_value(ExternalControlResponseEnvelope {
            request_id: "request-1".into(),
            response: Some(response),
            error: None,
        })
        .unwrap(),
        json!({
            "request_id": "request-1",
            "response": expected_response,
        })
    );
}

#[test]
fn all_external_control_results_preserve_the_public_json_contract() {
    assert_response_contract(
        ExternalControlResponse::ListTerminalSessions(ListTerminalSessionsResult {
            sessions: vec![crate::terminal_control::TerminalSessionInfo {
                session_id: "s1".into(),
                protocol: TerminalProtocol::Ssh,
                target: "admin@example.test:22".into(),
                encoding: "utf-8".into(),
                status: TerminalStatus::Connected,
            }],
        }),
        "list_terminal_sessions",
        json!({
            "sessions": [{
                "session_id": "s1",
                "protocol": "ssh",
                "target": "admin@example.test:22",
                "encoding": "utf-8",
                "status": "connected"
            }]
        }),
    );
    assert_response_contract(
        ExternalControlResponse::ListConnectionProfiles(ListConnectionProfilesResult {
            profiles: vec![ExternalControlConnectionProfile {
                id: "p1".into(),
                connection_type: "telnet".into(),
                host: "example.test".into(),
                port: 23,
                username: None,
                auth_method: None,
                encoding: Some("utf-8".into()),
                terminal_mode: Some("general".into()),
                private_key_configured: None,
                jump_profile_id: None,
                memo: None,
            }],
        }),
        "list_connection_profiles",
        json!({
            "profiles": [{
                "id": "p1",
                "connection_type": "telnet",
                "host": "example.test",
                "port": 23,
                "encoding": "utf-8",
                "terminal_mode": "general"
            }]
        }),
    );

    let connection_result = ConnectionCreatedResult {
        session_id: "s2".into(),
        connection_type: "ssh".into(),
        target: "admin@example.test:22".into(),
        title: "admin@example.test".into(),
        encoding: "utf-8".into(),
        terminal_mode: "general".into(),
        auto_logging: false,
    };
    let expected_connection = json!({
        "session_id": "s2",
        "connection_type": "ssh",
        "target": "admin@example.test:22",
        "title": "admin@example.test",
        "encoding": "utf-8",
        "terminal_mode": "general",
        "auto_logging": false
    });
    for (response, operation) in [
        (
            ExternalControlResponse::ConnectSavedProfile(connection_result.clone()),
            "connect_saved_profile",
        ),
        (
            ExternalControlResponse::ConnectSsh(connection_result.clone()),
            "connect_ssh",
        ),
        (
            ExternalControlResponse::ConnectTelnet(connection_result.clone()),
            "connect_telnet",
        ),
        (
            ExternalControlResponse::ConnectSerialConsole(connection_result),
            "connect_serial_console",
        ),
    ] {
        assert_response_contract(response, operation, expected_connection.clone());
    }
    assert_response_contract(
        ExternalControlResponse::ListSerialPorts(ListSerialPortsResult {
            ports: vec![serial::PortInfo {
                name: "COM3".into(),
                port_type: "USB".into(),
            }],
        }),
        "list_serial_ports",
        json!({ "ports": [{ "name": "COM3", "port_type": "USB" }] }),
    );

    let output = TerminalOutputResult {
        session_id: "s1".into(),
        output: "tail".into(),
        truncated: true,
        available_chars: 9,
        start_cursor: 5,
        cursor: 9,
    };
    assert_response_contract(
        ExternalControlResponse::ReadTerminalOutput(ReadTerminalOutputResult::Recent(
            output.clone(),
        )),
        "read_terminal_output",
        json!({
            "mode": "recent", "session_id": "s1", "output": "tail",
            "truncated": true, "available_chars": 9, "start_cursor": 5, "cursor": 9
        }),
    );
    assert_response_contract(
        ExternalControlResponse::ReadTerminalOutput(ReadTerminalOutputResult::Delta(output)),
        "read_terminal_output",
        json!({
            "mode": "delta", "session_id": "s1", "output": "tail",
            "truncated": true, "available_chars": 9, "start_cursor": 5, "cursor": 9
        }),
    );
    assert_response_contract(
        ExternalControlResponse::ReadTerminalOutput(ReadTerminalOutputResult::Wait(
            WaitTerminalOutputResult {
                session_id: "s1".into(),
                matched: false,
                timed_out: true,
                output: "partial".into(),
                truncated: false,
                available_chars: 7,
                start_cursor: 0,
                cursor: 7,
            },
        )),
        "read_terminal_output",
        json!({
            "mode": "wait", "session_id": "s1", "matched": false,
            "timed_out": true, "output": "partial", "truncated": false,
            "available_chars": 7, "start_cursor": 0, "cursor": 7
        }),
    );
    assert_response_contract(
        ExternalControlResponse::SendTerminalInput(SendTerminalInputResult {
            session_id: "s1".into(),
            sent: true,
        }),
        "send_terminal_input",
        json!({ "session_id": "s1", "sent": true }),
    );
    assert_response_contract(
        ExternalControlResponse::StartTerminalLog(StartTerminalLogResult {
            session_id: "s1".into(),
            started: true,
            already_active: false,
            file_path: "C:\\logs\\s1.log".into(),
            log_mode: "manual".into(),
        }),
        "start_terminal_log",
        json!({
            "session_id": "s1", "started": true, "already_active": false,
            "file_path": "C:\\logs\\s1.log", "log_mode": "manual"
        }),
    );
    assert_response_contract(
        ExternalControlResponse::StopTerminalLog(StopTerminalLogResult {
            session_id: "s1".into(),
            stopped: false,
            already_inactive: true,
        }),
        "stop_terminal_log",
        json!({ "session_id": "s1", "stopped": false, "already_inactive": true }),
    );
    assert_response_contract(
        ExternalControlResponse::GetTerminalLogStatus(TerminalLogStatusResult {
            session_id: "s1".into(),
            state: TerminalLogState::Inactive,
            file_path: None,
            log_mode: None,
        }),
        "get_terminal_log_status",
        json!({
            "session_id": "s1", "state": "inactive", "file_path": null, "log_mode": null
        }),
    );
    assert_response_contract(
        ExternalControlResponse::PauseTerminalLog(SetTerminalLogPausedResult {
            session_id: "s1".into(),
            changed: true,
            state: TerminalLogState::Paused,
            file_path: Some("C:\\logs\\s1.log".into()),
            log_mode: Some("auto".into()),
        }),
        "pause_terminal_log",
        json!({
            "session_id": "s1", "changed": true, "state": "paused",
            "file_path": "C:\\logs\\s1.log", "log_mode": "auto"
        }),
    );
    assert_response_contract(
        ExternalControlResponse::RunTerminalCommand(RunTerminalCommandResult {
            session_id: "s1".into(),
            sent: true,
            matched: false,
            timed_out: true,
            output: "tail".into(),
            truncated: true,
            available_chars: 20,
            start_cursor: 10,
            cursor: 30,
        }),
        "run_terminal_command",
        json!({
            "session_id": "s1", "sent": true, "matched": false,
            "timed_out": true, "output": "tail", "truncated": true,
            "available_chars": 20, "start_cursor": 10, "cursor": 30
        }),
    );
}

#[tokio::test]
async fn connection_tools_require_connect_enabled() {
    let service = McpTerminalService::new(test_runtime());

    let error = service
        .list_connection_profiles(ListConnectionProfilesArgs::default())
        .await
        .unwrap_err();
    assert!(matches!(&error, ExternalControlError::PermissionDenied(_)));
    assert!(error.message().contains("connect_enabled"));

    let error = service
        .connect_saved_profile(ConnectSavedProfileArgs {
            profile_id: "dev".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("connect_enabled"));

    let error = service.list_serial_ports().await.unwrap_err();
    assert!(error.message().contains("connect_enabled"));

    let error = service
        .connect_serial_console(ConnectSerialConsoleArgs {
            port: "COM1".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("connect_enabled"));
}

#[tokio::test]
async fn direct_connection_tools_require_the_additional_permission() {
    let service = McpTerminalService::new(test_runtime_with_app_config(AppConfig::default()));

    let error = service
        .connect_ssh(ConnectSshArgs {
            host: "router.example.test".into(),
            port: None,
            username: "admin".into(),
            auth_method: None,
            private_key_path: None,
            jump_profile_id: None,
            encoding: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("direct_connect_enabled"));

    let error = service
        .connect_telnet(ConnectTelnetArgs {
            host: "router.example.test".into(),
            port: None,
            encoding: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();
    assert!(error.message().contains("direct_connect_enabled"));
}

#[tokio::test]
async fn permission_rejection_stops_before_credential_and_connection_io() {
    let (runtime, _config, protocol, ui, _logger, trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    let service = ExternalControlService::new(runtime);

    let error = service
        .connect_ssh(ConnectSshArgs {
            host: "router.example.test".into(),
            port: None,
            username: "admin".into(),
            auth_method: Some(ExternalControlSshAuthMethod::PublicKey),
            private_key_path: Some("key.pem".into()),
            jump_profile_id: None,
            encoding: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();

    assert!(matches!(error, ExternalControlError::PermissionDenied(_)));
    assert!(protocol.calls.lock().unwrap().is_empty());
    assert!(ui.calls.lock().unwrap().is_empty());
    assert_eq!(*trace.lock().unwrap(), vec!["load_config"]);
}

#[tokio::test]
async fn direct_ssh_uses_common_credential_policy_and_prompt_unknown_host_key_flow() {
    let mut config = AppConfig::default();
    config.external_control.connect_enabled = true;
    config.external_control.direct_connect_enabled = true;
    let (runtime, _config, protocol, ui, _logger, trace) =
        test_runtime_parts(config, Vec::new(), Some(test_logger()));
    *ui.private_key_requires_passphrase.lock().unwrap() = Ok(true);
    *ui.credential_result.lock().unwrap() = Ok(Some("passphrase".into()));
    let service = ExternalControlService::new(runtime.clone());

    let result = service
        .connect_ssh(ConnectSshArgs {
            host: "router.example.test".into(),
            port: Some(2222),
            username: "admin".into(),
            auth_method: Some(ExternalControlSshAuthMethod::PublicKey),
            private_key_path: Some("key.pem".into()),
            jump_profile_id: None,
            encoding: Some(ExternalControlEncoding::ShiftJis),
            terminal_mode: Some(ExternalControlTerminalMode::JuniperJunos),
            cols: Some(132),
            rows: Some(43),
        })
        .await
        .unwrap();

    assert_eq!(result.session_id, "ssh-session");
    let calls = protocol.calls.lock().unwrap();
    let TestProtocolCall::ConnectSsh(request) = &calls[0] else {
        panic!("expected SSH connect call");
    };
    assert_eq!(
        request.host_key_handling,
        ExternalControlHostKeyHandling::PromptUnknown
    );
    assert_eq!(request.prompt_window_id, "main");
    assert_eq!(request.options.host, "router.example.test");
    assert_eq!(
        request.options.key_passphrase.as_deref(),
        Some("passphrase")
    );
    assert_eq!(request.options.cols, 132);
    assert_eq!(request.options.rows, 43);
    drop(calls);
    assert!(matches!(
        ui.calls.lock().unwrap()[0],
        TestUiCall::Credential(_)
    ));
    assert_eq!(
        *trace.lock().unwrap(),
        vec![
            "load_config",
            "load_config",
            "load_config",
            "request_credential",
            "connect_ssh",
            "emit_workspace_updated"
        ]
    );
    assert_eq!(
        runtime
            .workspace
            .snapshot_for_window("main".into())
            .await
            .tabs[0]
            .session_id,
        "ssh-session"
    );
}

#[tokio::test]
async fn saved_ssh_requires_trusted_host_key_without_bypassing_finish() {
    let mut config = AppConfig::default();
    config.external_control.connect_enabled = true;
    config.saved_connections = vec![SavedConnection {
        id: "saved".into(),
        connection_type: "ssh".into(),
        host: Some("router.example.test".into()),
        username: Some("admin".into()),
        auth_method: Some("password".into()),
        ..SavedConnection::default()
    }];
    let (runtime, _config, protocol, _ui, _logger, trace) =
        test_runtime_parts(config, Vec::new(), Some(test_logger()));
    let service = ExternalControlService::new(runtime);

    service
        .connect_saved_profile(ConnectSavedProfileArgs {
            profile_id: "saved".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        })
        .await
        .unwrap();

    let calls = protocol.calls.lock().unwrap();
    let TestProtocolCall::ConnectSsh(request) = &calls[0] else {
        panic!("expected SSH connect call");
    };
    assert_eq!(
        request.host_key_handling,
        ExternalControlHostKeyHandling::RequireTrusted
    );
    assert_eq!(
        *trace.lock().unwrap(),
        vec![
            "load_config",
            "load_config",
            "connect_ssh",
            "emit_workspace_updated"
        ]
    );
}

#[tokio::test]
async fn connection_failures_do_not_register_or_notify_workspace() {
    let mut config = AppConfig::default();
    config.external_control.connect_enabled = true;
    config.external_control.direct_connect_enabled = true;
    let (runtime, _config, protocol, ui, _logger, _trace) =
        test_runtime_parts(config, Vec::new(), Some(test_logger()));
    *protocol.telnet_result.lock().unwrap() = Err("refused".into());
    let service = ExternalControlService::new(runtime.clone());

    let error = service
        .connect_telnet(ConnectTelnetArgs {
            host: "router.example.test".into(),
            port: None,
            encoding: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();

    assert!(matches!(error, ExternalControlError::InvalidArguments(_)));
    assert!(runtime
        .workspace
        .snapshot_for_window("main".into())
        .await
        .tabs
        .is_empty());
    assert!(!ui
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| matches!(call, TestUiCall::WorkspaceUpdated(_))));
}

#[tokio::test]
async fn ssh_and_serial_connection_errors_follow_the_same_common_failure_path() {
    let mut ssh_config = AppConfig::default();
    ssh_config.external_control.connect_enabled = true;
    ssh_config.external_control.direct_connect_enabled = true;
    let (ssh_runtime, _config, ssh_protocol, _ui, _logger, _trace) =
        test_runtime_parts(ssh_config, Vec::new(), Some(test_logger()));
    *ssh_protocol.ssh_result.lock().unwrap() = Err("ssh failed".into());
    let ssh_error = ExternalControlService::new(ssh_runtime.clone())
        .connect_ssh(ConnectSshArgs {
            host: "router.example.test".into(),
            port: None,
            username: "admin".into(),
            auth_method: None,
            private_key_path: None,
            jump_profile_id: None,
            encoding: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        ssh_error,
        ExternalControlError::InvalidArguments(_)
    ));
    assert!(ssh_runtime
        .workspace
        .snapshot_for_window("main".into())
        .await
        .tabs
        .is_empty());

    let mut serial_config = AppConfig::default();
    serial_config.external_control.connect_enabled = true;
    let (serial_runtime, _config, serial_protocol, _ui, _logger, _trace) = test_runtime_parts(
        serial_config,
        vec![serial::PortInfo {
            name: "COM3".into(),
            port_type: "USB".into(),
        }],
        Some(test_logger()),
    );
    *serial_protocol.serial_result.lock().unwrap() = Err("serial failed".into());
    let serial_error = ExternalControlService::new(serial_runtime.clone())
        .connect_serial_console(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        serial_error,
        ExternalControlError::InvalidArguments(_)
    ));
    assert!(serial_runtime
        .workspace
        .snapshot_for_window("main".into())
        .await
        .tabs
        .is_empty());
}

#[tokio::test]
async fn terminal_input_validates_before_calling_protocol_io() {
    let (runtime, _config, protocol, _ui, _logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Serial, "COM3".into())
        .await;
    let service = ExternalControlService::new(runtime);

    let sent = service
        .send_terminal_input(SendTerminalInputArgs {
            session_id: "s1".into(),
            data: "show version\n".into(),
        })
        .await
        .unwrap();
    assert!(sent.sent);
    assert_eq!(
        protocol.calls.lock().unwrap()[0],
        TestProtocolCall::Write(
            TerminalProtocol::Serial,
            "s1".into(),
            "show version\n".into()
        )
    );

    protocol.calls.lock().unwrap().clear();
    let error = service
        .send_terminal_input(SendTerminalInputArgs {
            session_id: "s1".into(),
            data: "x".repeat(20_001),
        })
        .await
        .unwrap_err();
    assert!(matches!(error, ExternalControlError::InvalidArguments(_)));
    assert!(protocol.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn automatic_log_failure_keeps_the_connected_session_result() {
    let mut config = AppConfig::default();
    config.external_control.connect_enabled = true;
    config.external_control.direct_connect_enabled = true;
    config.terminal.auto_session_log = true;
    let (runtime, _config, _protocol, _ui, logger, trace) =
        test_runtime_parts(config, Vec::new(), Some(test_logger()));
    *logger.start_result.lock().unwrap() = Some(Err("log unavailable".into()));
    let service = ExternalControlService::new(runtime.clone());

    let result = service
        .connect_telnet(ConnectTelnetArgs {
            host: "router.example.test".into(),
            port: None,
            encoding: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
        .await
        .unwrap();

    assert_eq!(result.session_id, "telnet-session");
    assert!(!result.auto_logging);
    let snapshot = runtime.workspace.snapshot_for_window("main".into()).await;
    assert_eq!(snapshot.tabs[0].session_id, "telnet-session");
    assert!(!snapshot.tabs[0].is_manual_logging);
    assert_eq!(
        *trace.lock().unwrap(),
        vec![
            "load_config",
            "load_config",
            "load_config",
            "connect_telnet",
            "start_auto_log",
            "emit_workspace_updated"
        ]
    );
}

#[test]
fn direct_connections_validate_and_normalize_targets() {
    let ssh = prepare_direct_ssh_connection(
        &AppConfig::default(),
        ConnectSshArgs {
            host: "2001:db8::1".into(),
            port: None,
            username: " admin ".into(),
            auth_method: None,
            private_key_path: None,
            jump_profile_id: None,
            encoding: Some(ExternalControlEncoding::ShiftJis),
            terminal_mode: Some(ExternalControlTerminalMode::JuniperJunos),
            cols: None,
            rows: None,
        },
    )
    .unwrap();
    assert_eq!(ssh.target, "admin@[2001:db8::1]:22");
    assert_eq!(ssh.encoding, "shift-jis");
    assert_eq!(ssh.terminal_mode, "juniper_junos");
    assert_eq!(ssh.cols, 120);
    assert_eq!(ssh.rows, 30);

    let telnet = prepare_direct_telnet_connection(ConnectTelnetArgs {
        host: "router.example.test".into(),
        port: None,
        encoding: None,
        terminal_mode: None,
        cols: Some(132),
        rows: Some(43),
    })
    .unwrap();
    assert_eq!(telnet.target, "router.example.test:23");
    assert_eq!(telnet.cols, 132);
    assert_eq!(telnet.rows, 43);

    let error = prepare_direct_telnet_connection(ConnectTelnetArgs {
        host: "router.example.test".into(),
        port: None,
        encoding: None,
        terminal_mode: None,
        cols: Some(0),
        rows: None,
    })
    .unwrap_err();
    assert!(error.contains("cols"));

    for invalid in [
        "ssh://router.example.test",
        "admin@router.example.test",
        "router.example.test:2222",
        "router.example.test/path",
        "[2001:db8::1]",
        " router.example.test",
        "router.example.test ",
        "router_example.test",
        "router..example.test",
        "-router.example.test",
    ] {
        assert!(
            normalize_direct_host(invalid).is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn direct_connection_requests_round_trip_through_the_control_protocol_shape() {
    let request = ExternalControlRequest::ConnectSsh(ConnectSshArgs {
        host: "router.example.test".into(),
        port: Some(2222),
        username: "admin".into(),
        auth_method: Some(ExternalControlSshAuthMethod::PublicKey),
        private_key_path: Some("id_ed25519".into()),
        jump_profile_id: Some("jump".into()),
        encoding: Some(ExternalControlEncoding::Utf8),
        terminal_mode: Some(ExternalControlTerminalMode::General),
        cols: Some(132),
        rows: Some(43),
    });

    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["operation"], "connect_ssh");
    assert_eq!(value["arguments"]["auth_method"], "public_key");
    assert_eq!(value["arguments"]["encoding"], "utf8");
    assert_eq!(
        serde_json::from_value::<ExternalControlRequest>(value).unwrap(),
        request
    );
}

#[test]
fn direct_ssh_requires_an_externally_enabled_single_jump_profile() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![SavedConnection {
        id: "jump".into(),
        connection_type: "ssh".into(),
        host: Some("jump.example.test".into()),
        username: Some("operator".into()),
        auth_method: Some("password".into()),
        external_control_enabled: false,
        ..SavedConnection::default()
    }];
    let args = ConnectSshArgs {
        host: "target.example.test".into(),
        port: None,
        username: "admin".into(),
        auth_method: None,
        private_key_path: None,
        jump_profile_id: Some("jump".into()),
        encoding: None,
        terminal_mode: None,
        cols: None,
        rows: None,
    };

    let error = prepare_direct_ssh_connection(&config, args.clone()).unwrap_err();
    assert!(error.contains("disabled for external control"));

    config.saved_connections[0].external_control_enabled = true;
    config.saved_connections[0].jump_profile_id = Some("nested".into());
    let error = prepare_direct_ssh_connection(&config, args.clone()).unwrap_err();
    assert!(error.contains("Nested"));

    config.saved_connections[0].jump_profile_id = None;
    let prepared = prepare_direct_ssh_connection(&config, args).unwrap();
    match prepared.kind {
        PreparedConnectionKind::Ssh { jump_profile, .. } => {
            assert_eq!(jump_profile.unwrap().id, "jump");
        }
        PreparedConnectionKind::Telnet { .. } => panic!("expected SSH"),
    }
}

#[tokio::test]
async fn service_lists_serial_ports_from_runtime_injected_ports() {
    let runtime = test_runtime_with_serial_ports(vec![
        serial::PortInfo {
            name: "COM3".into(),
            port_type: "USB".into(),
        },
        serial::PortInfo {
            name: "COM9".into(),
            port_type: "Bluetooth".into(),
        },
    ]);
    let service = McpTerminalService::new(runtime);

    let result = service.list_serial_ports().await.unwrap();

    assert_eq!(result.ports.len(), 2);
    assert_eq!(result.ports[0].name, "COM3");
    assert_eq!(result.ports[1].name, "COM9");
}

#[tokio::test]
async fn control_service_dispatches_to_in_process_backend() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
        .await;
    let server = ExaTermMcpServer::with_service(ExternalControlService::new(runtime));

    let result = server
        .call_tool_json("list_terminal_sessions", json!({}))
        .await
        .unwrap();

    assert_eq!(result["sessions"][0]["session_id"], "s1");
    assert_eq!(result["sessions"][0]["protocol"], "ssh");
}

#[tokio::test]
async fn control_service_rejects_unknown_tools() {
    let server = ExaTermMcpServer::with_service(ExternalControlService::new(test_runtime()));

    let error = server
        .call_tool_json("missing_tool", json!({}))
        .await
        .unwrap_err();

    assert!(error.message.contains("Unknown MCP tool"));
}

#[tokio::test]
async fn mcp_start_terminal_log_rejects_cli_only_options() {
    let server = ExaTermMcpServer::with_service(ExternalControlService::new(test_runtime()));

    let error = server
        .call_tool_json(
            "start_terminal_log",
            json!({
                "session_id": "s1",
                "file_path": "C:\\logs\\session.log",
                "write_mode": "overwrite"
            }),
        )
        .await
        .unwrap_err();

    assert!(error.message.contains("unknown field"));
}

#[tokio::test]
async fn control_plane_rejects_unknown_protocol_version() {
    let (server_stream, client_stream) = tokio::io::duplex(4096);
    let service = ExternalControlService::new(test_runtime());
    tokio::spawn(async move {
        handle_control_connection(service, server_stream)
            .await
            .expect("control connection should close cleanly");
    });
    let (read_half, mut write_half) = tokio::io::split(client_stream);
    let mut reader = BufReader::new(read_half);

    write_json_line_for_test(
        &mut write_half,
        json!({
            "protocol_version": CONTROL_PROTOCOL_VERSION + 1
        }),
    )
    .await;
    let response = read_json_line_for_test(&mut reader).await;

    assert_eq!(response["protocol_version"], CONTROL_PROTOCOL_VERSION);
    assert_eq!(CONTROL_PROTOCOL_VERSION, 4);
    assert!(response["session_nonce"].is_null());
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Unsupported"));
}

#[tokio::test]
async fn control_plane_rejects_missing_and_wrong_nonce() {
    let (server_stream, client_stream) = tokio::io::duplex(4096);
    let service = ExternalControlService::new(test_runtime());
    tokio::spawn(async move {
        handle_control_connection(service, server_stream)
            .await
            .expect("control connection should close cleanly");
    });
    let (read_half, mut write_half) = tokio::io::split(client_stream);
    let mut reader = BufReader::new(read_half);

    write_json_line_for_test(
        &mut write_half,
        json!({
            "protocol_version": CONTROL_PROTOCOL_VERSION
        }),
    )
    .await;
    let handshake = read_json_line_for_test(&mut reader).await;
    assert!(handshake["session_nonce"].as_str().is_some());

    write_json_line_for_test(
        &mut write_half,
        json!({
            "protocol_version": CONTROL_PROTOCOL_VERSION,
            "request_id": "missing",
            "request": {
                "operation": "list_terminal_sessions"
            }
        }),
    )
    .await;
    let response = read_json_line_for_test(&mut reader).await;
    assert_eq!(response["request_id"], "missing");
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("nonce"));

    write_json_line_for_test(
        &mut write_half,
        json!({
            "protocol_version": CONTROL_PROTOCOL_VERSION,
            "request_id": "wrong",
            "session_nonce": "wrong-nonce",
            "request": {
                "operation": "list_terminal_sessions"
            }
        }),
    )
    .await;
    let response = read_json_line_for_test(&mut reader).await;
    assert_eq!(response["request_id"], "wrong");
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("nonce"));
}

#[tokio::test]
async fn proxy_control_call_preserves_structured_result() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
        .await;
    let (server_stream, client_stream) = tokio::io::duplex(4096);
    let service = ExternalControlService::new(runtime);
    tokio::spawn(async move {
        handle_control_connection(service, server_stream)
            .await
            .expect("control connection should close cleanly");
    });

    let result = external_control_call_over_stream(
        client_stream,
        ExternalControlRequest::ListTerminalSessions,
    )
    .await
    .unwrap()
    .into_value()
    .unwrap();

    assert_eq!(result["sessions"][0]["session_id"], "s1");
    assert_eq!(result["sessions"][0]["protocol"], "ssh");
}

#[tokio::test]
async fn stdio_server_smoke_initialize_and_tools_list() {
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);
    let service = ExternalControlService::new(test_runtime());
    tokio::spawn(async move {
        let server = ExaTermMcpServer::with_service(service)
            .serve(server_transport)
            .await
            .expect("stdio server should initialize");
        let _ = server.waiting().await;
    });
    let (read_half, mut write_half) = tokio::io::split(client_transport);
    let mut reader = BufReader::new(read_half);

    write_json_line_for_test(
        &mut write_half,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {
                    "name": "exaterm-test",
                    "version": "1.0"
                }
            }
        }),
    )
    .await;
    let initialize = read_json_line_for_test(&mut reader).await;
    assert_eq!(initialize["id"], 1);
    assert_eq!(initialize["result"]["serverInfo"]["name"], "exaterm");

    write_json_line_for_test(
        &mut write_half,
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )
    .await;
    write_json_line_for_test(
        &mut write_half,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }),
    )
    .await;
    let tools = read_json_line_for_test(&mut reader).await;
    assert_eq!(tools["id"], 2);
    let tool_names = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"list_terminal_sessions"));
    assert!(tool_names.contains(&"connect_ssh"));
    assert!(tool_names.contains(&"connect_telnet"));
    assert!(tool_names.contains(&"read_terminal_output"));
    assert!(tool_names.contains(&"get_terminal_log_status"));
    assert!(tool_names.contains(&"pause_terminal_log"));
    assert!(tool_names.contains(&"resume_terminal_log"));
    assert!(!tool_names.contains(&"read_terminal_output_delta"));
    assert!(!tool_names.contains(&"wait_terminal_output"));

    let read_tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "read_terminal_output")
        .unwrap();
    assert_eq!(read_tool["inputSchema"]["type"], "object");
    assert_eq!(
        read_tool["inputSchema"]["oneOf"].as_array().unwrap().len(),
        3
    );
    let read_schema = read_tool["inputSchema"].to_string();
    assert!(read_schema.contains("\"mode\""));
    assert!(read_schema.contains("\"recent\""));
    assert!(read_schema.contains("\"delta\""));
    assert!(read_schema.contains("\"wait\""));

    let ssh_connect_tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "connect_ssh")
        .unwrap();
    let ssh_schema = ssh_connect_tool["inputSchema"].to_string();
    assert!(ssh_schema.contains("\"host\""));
    assert!(ssh_schema.contains("\"username\""));
    assert!(ssh_schema.contains("\"jump_profile_id\""));

    let serial_connect_tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "connect_serial_console")
        .unwrap();
    assert!(serial_connect_tool["inputSchema"]
        .to_string()
        .contains("\"juniper_junos\""));
    assert!(serial_connect_tool["inputSchema"]
        .to_string()
        .contains("\"vyos\""));
    assert!(serial_connect_tool["inputSchema"]
        .to_string()
        .contains("\"fujitsu_sir\""));
    assert!(serial_connect_tool["inputSchema"]
        .to_string()
        .contains("\"allied_telesis_awplus\""));
    assert!(serial_connect_tool["inputSchema"]
        .to_string()
        .contains("\"furukawa_fitelnet\""));

    let start_log_tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "start_terminal_log")
        .unwrap();
    let start_log_schema = start_log_tool["inputSchema"].to_string();
    assert!(start_log_schema.contains("\"session_id\""));
    assert!(!start_log_schema.contains("file_path"));
    assert!(!start_log_schema.contains("write_mode"));
}

async fn write_json_line_for_test<W>(writer: &mut W, value: Value)
where
    W: tokio::io::AsyncWrite + Unpin,
{
    writer
        .write_all(value.to_string().as_bytes())
        .await
        .unwrap();
    writer.write_all(b"\n").await.unwrap();
    writer.flush().await.unwrap();
}

async fn read_json_line_for_test<R>(reader: &mut R) -> Value
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

#[test]
fn list_connection_profiles_returns_only_ssh_telnet_without_secret_paths() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "dev".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            port: Some(2222),
            username: Some("admin".into()),
            auth_method: Some("public_key".into()),
            private_key_path: Some("C:\\Users\\me\\.ssh\\id_ed25519".into()),
            encoding: Some("shift-jis".into()),
            terminal_mode: Some("cisco_ios".into()),
            jump_profile_id: Some("bastion".into()),
            memo: Some("Cisco ISR branch edge".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "legacy".into(),
            connection_type: "telnet".into(),
            host: Some("192.0.2.20".into()),
            port: None,
            encoding: Some("euc-jp".into()),
            memo: Some("  ".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "console".into(),
            connection_type: "serial".into(),
            ..SavedConnection::default()
        },
    ];

    let profiles = list_connection_profiles_from_config(&config, None);

    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].id, "dev");
    assert_eq!(profiles[0].private_key_configured, Some(true));
    assert_eq!(profiles[0].jump_profile_id.as_deref(), Some("bastion"));
    assert_eq!(profiles[0].memo.as_deref(), Some("Cisco ISR branch edge"));
    assert_eq!(profiles[1].id, "legacy");
    assert_eq!(profiles[1].port, 23);
    assert_eq!(profiles[1].memo, None);
    let serialized = serde_json::to_string(&profiles).unwrap();
    assert!(serialized.contains("Cisco ISR branch edge"));
    assert!(!serialized.contains("private_key_path"));
    assert!(!serialized.contains("id_ed25519"));
}

#[test]
fn list_connection_profiles_filters_by_connection_type() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "ssh-profile".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "telnet-profile".into(),
            connection_type: "telnet".into(),
            host: Some("192.0.2.20".into()),
            ..SavedConnection::default()
        },
    ];

    let ssh_profiles =
        list_connection_profiles_from_config(&config, Some(SavedProfileConnectionType::Ssh));
    let telnet_profiles =
        list_connection_profiles_from_config(&config, Some(SavedProfileConnectionType::Telnet));

    assert_eq!(ssh_profiles.len(), 1);
    assert_eq!(ssh_profiles[0].id, "ssh-profile");
    assert_eq!(ssh_profiles[0].auth_method.as_deref(), Some("auto"));
    assert_eq!(telnet_profiles.len(), 1);
    assert_eq!(telnet_profiles[0].id, "telnet-profile");
}

#[test]
fn list_connection_profiles_skips_mcp_disabled_profiles() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "ssh-enabled".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "telnet-disabled".into(),
            connection_type: "telnet".into(),
            host: Some("192.0.2.20".into()),
            external_control_enabled: false,
            ..SavedConnection::default()
        },
    ];

    let profiles = list_connection_profiles_from_config(&config, None);

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, "ssh-enabled");
}

#[tokio::test]
async fn service_lists_connection_profiles_from_runtime_app_config() {
    let mut app_config = AppConfig::default();
    app_config.saved_connections = vec![
        SavedConnection {
            id: "ssh-enabled".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            username: Some("admin".into()),
            auth_method: Some("password".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "telnet-disabled".into(),
            connection_type: "telnet".into(),
            host: Some("192.0.2.20".into()),
            external_control_enabled: false,
            ..SavedConnection::default()
        },
    ];
    let service = McpTerminalService::new(test_runtime_with_app_config(app_config));

    let result = service
        .list_connection_profiles(ListConnectionProfilesArgs::default())
        .await
        .unwrap();

    assert_eq!(result.profiles.len(), 1);
    assert_eq!(result.profiles[0].id, "ssh-enabled");
    assert_eq!(result.profiles[0].connection_type, "ssh");
}

#[test]
fn prepare_saved_profile_rejects_missing_and_unsupported_profiles() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![SavedConnection {
        id: "console".into(),
        connection_type: "serial".into(),
        host: Some("COM1".into()),
        ..SavedConnection::default()
    }];

    let missing = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "missing".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(missing.contains("not found"));

    let unsupported = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "console".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(unsupported.contains("not found"));
}

#[test]
fn prepare_saved_profile_rejects_mcp_disabled_profiles() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![SavedConnection {
        id: "disabled".into(),
        connection_type: "ssh".into(),
        host: Some("192.0.2.10".into()),
        username: Some("admin".into()),
        auth_method: Some("password".into()),
        external_control_enabled: false,
        ..SavedConnection::default()
    }];

    let error = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "disabled".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();

    assert!(error.contains("external control"));
}

#[test]
fn prepare_saved_profile_rejects_incomplete_ssh_profiles() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "missing-user".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            auth_method: Some("password".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "key".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            username: Some("admin".into()),
            auth_method: Some("public_key".into()),
            private_key_path: Some("  ".into()),
            ..SavedConnection::default()
        },
    ];

    let user_error = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "missing-user".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(user_error.contains("username"));

    let key_error = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "key".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(key_error.contains("private key"));
}

#[test]
fn prepare_saved_profile_allows_profiles_without_explicit_mcp_flag() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![SavedConnection {
        id: "default-enabled".into(),
        connection_type: "ssh".into(),
        host: Some("192.0.2.10".into()),
        username: Some("admin".into()),
        auth_method: Some("password".into()),
        ..SavedConnection::default()
    }];

    let prepared = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "default-enabled".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap();

    assert_eq!(prepared.profile_id, "default-enabled");
}

#[test]
fn prepare_saved_profile_defaults_missing_auth_method_to_auto() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![SavedConnection {
        id: "legacy-auto".into(),
        connection_type: "ssh".into(),
        host: Some("192.0.2.10".into()),
        username: Some("admin".into()),
        auth_method: None,
        ..SavedConnection::default()
    }];

    let prepared = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "legacy-auto".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap();

    assert!(matches!(
        prepared.kind,
        PreparedConnectionKind::Ssh { auth_method, .. } if auth_method == "auto"
    ));
}

#[test]
fn prepare_saved_profile_builds_connection_metadata() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![SavedConnection {
        id: "dev".into(),
        connection_type: "ssh".into(),
        host: Some("192.0.2.10".into()),
        port: Some(2222),
        username: Some("admin".into()),
        auth_method: Some("password".into()),
        encoding: Some("shift-jis".into()),
        terminal_mode: Some("arista_eos".into()),
        ..SavedConnection::default()
    }];

    let prepared = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "dev".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: Some(80),
            rows: Some(24),
        },
    )
    .unwrap();

    assert_eq!(prepared.connection_type, "ssh");
    assert_eq!(prepared.profile_id, "dev");
    assert_eq!(prepared.target, "admin@192.0.2.10:2222");
    assert_eq!(prepared.title, "admin@192.0.2.10");
    assert_eq!(prepared.encoding, "shift-jis");
    assert_eq!(prepared.terminal_mode, "arista_eos");
    assert_eq!(prepared.cols, 80);
    assert_eq!(prepared.rows, 24);
}

#[test]
fn prepare_saved_profile_uses_id_and_connection_type() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "router".into(),
            connection_type: "telnet".into(),
            host: Some("192.0.2.23".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "router".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.22".into()),
            username: Some("admin".into()),
            auth_method: Some("password".into()),
            ..SavedConnection::default()
        },
    ];

    let ssh = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "router".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap();
    let telnet = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "router".into(),
            connection_type: SavedProfileConnectionType::Telnet,
            cols: None,
            rows: None,
        },
    )
    .unwrap();

    assert_eq!(ssh.connection_type, "ssh");
    assert_eq!(ssh.target, "admin@192.0.2.22:22");
    assert_eq!(telnet.connection_type, "telnet");
    assert_eq!(telnet.target, "192.0.2.23:23");
}

#[tokio::test]
async fn backend_requires_saved_profile_connection_type() {
    let server = ExaTermMcpServer::with_service(ExternalControlService::new(test_runtime()));
    let error = server
        .call_tool_json(
            "connect_saved_profile",
            json!({
                "profile_id": "router",
            }),
        )
        .await
        .unwrap_err();

    assert!(error.message.contains("connection_type"));
}

#[tokio::test]
async fn mcp_routes_typed_direct_connection_tools() {
    let server = ExaTermMcpServer::with_service(ExternalControlService::new(
        test_runtime_with_app_config(AppConfig::default()),
    ));
    let ssh_error = server
        .call_tool_json(
            "connect_ssh",
            json!({
                "host": "router.example.test",
                "username": "admin"
            }),
        )
        .await
        .unwrap_err();
    assert!(ssh_error.message.contains("direct_connect_enabled"));

    let telnet_error = server
        .call_tool_json(
            "connect_telnet",
            json!({
                "host": "router.example.test"
            }),
        )
        .await
        .unwrap_err();
    assert!(telnet_error.message.contains("direct_connect_enabled"));
}

#[tokio::test]
async fn service_connects_saved_ssh_profile_and_registers_workspace_metadata() {
    let mut app_config = AppConfig::default();
    app_config.terminal.auto_session_log = true;
    app_config.saved_connections = vec![
        SavedConnection {
            id: "bastion".into(),
            connection_type: "ssh".into(),
            host: Some("198.51.100.10".into()),
            port: Some(2222),
            username: Some("jump".into()),
            auth_method: Some("password".into()),
            external_control_enabled: false,
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "inside".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            port: Some(2200),
            username: Some("admin".into()),
            auth_method: Some("password".into()),
            encoding: Some("shift-jis".into()),
            terminal_mode: Some("cisco_ios".into()),
            jump_profile_id: Some("bastion".into()),
            ..SavedConnection::default()
        },
    ];
    let runtime = test_runtime_with_app_config(app_config);
    let service = McpTerminalService::new(runtime.clone());

    let result = service
        .connect_saved_profile(ConnectSavedProfileArgs {
            profile_id: "inside".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: Some(100),
            rows: Some(40),
        })
        .await
        .unwrap();

    let session_id = result.session_id.as_str();
    assert_eq!(result.connection_type, "ssh");
    assert_eq!(result.target, "admin@192.0.2.10:2200");
    assert_eq!(result.title, "admin@192.0.2.10");
    assert_eq!(result.encoding, "shift-jis");
    assert_eq!(result.terminal_mode, "cisco_ios");
    assert!(result.auto_logging);

    let snapshot = runtime.workspace.snapshot_for_window("main".into()).await;
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].session_id, session_id);
    assert_eq!(snapshot.tabs[0].title, "admin@192.0.2.10");
    assert_eq!(snapshot.tabs[0].encoding, "shift-jis");
    assert_eq!(snapshot.tabs[0].terminal_mode, "cisco_ios");
    assert!(snapshot.tabs[0].is_manual_logging);
    assert!(!snapshot.tabs[0].is_manual_logging_paused);
    assert!(snapshot.tabs[0].manual_log_file_path.is_some());
    assert_eq!(
        snapshot.tabs[0].connection_info,
        Some(WorkspaceConnectionInfo::Ssh {
            host: "192.0.2.10".into(),
            port: 2200,
            username: "admin".into(),
            auth_method: "password".into(),
            private_key_path: None,
            jump_profile_id: Some("bastion".into()),
        })
    );
    assert!(snapshot.tabs[0].manual_log_file_path.is_some());
}

#[tokio::test]
async fn service_connects_direct_telnet_and_registers_workspace_metadata() {
    let mut config = AppConfig::default();
    config.external_control.direct_connect_enabled = true;
    let runtime = test_runtime_with_app_config(config);
    let service = McpTerminalService::new(runtime.clone());

    let result = service
        .connect_telnet(ConnectTelnetArgs {
            host: "router.example.test".into(),
            port: Some(2323),
            encoding: Some(ExternalControlEncoding::EucJp),
            terminal_mode: Some(ExternalControlTerminalMode::JuniperJunos),
            cols: Some(100),
            rows: Some(40),
        })
        .await
        .unwrap();

    let session_id = result.session_id.as_str();
    assert_eq!(result.connection_type, "telnet");
    assert_eq!(result.target, "router.example.test:2323");
    assert_eq!(result.encoding, "euc-jp");
    assert_eq!(result.terminal_mode, "juniper_junos");

    let snapshot = runtime.workspace.snapshot_for_window("main".into()).await;
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].session_id, session_id);
    assert_eq!(
        snapshot.tabs[0].connection_info,
        Some(WorkspaceConnectionInfo::Telnet {
            host: "router.example.test".into(),
            port: 2323,
        })
    );
}

#[tokio::test]
async fn service_connects_saved_telnet_profile_with_default_port() {
    let mut app_config = AppConfig::default();
    app_config.saved_connections = vec![SavedConnection {
        id: "legacy".into(),
        connection_type: "telnet".into(),
        host: Some("192.0.2.20".into()),
        encoding: Some("euc-jp".into()),
        terminal_mode: Some("general".into()),
        ..SavedConnection::default()
    }];
    let runtime = test_runtime_with_app_config(app_config);
    let service = McpTerminalService::new(runtime.clone());

    let result = service
        .connect_saved_profile(ConnectSavedProfileArgs {
            profile_id: "legacy".into(),
            connection_type: SavedProfileConnectionType::Telnet,
            cols: None,
            rows: None,
        })
        .await
        .unwrap();

    assert_eq!(result.connection_type, "telnet");
    assert_eq!(result.target, "192.0.2.20:23");
    assert_eq!(result.title, "192.0.2.20:23");
    assert_eq!(result.encoding, "euc-jp");
    assert_eq!(result.terminal_mode, "general");
    let snapshot = runtime.workspace.snapshot_for_window("main".into()).await;
    assert_eq!(
        snapshot.tabs[0].connection_info,
        Some(WorkspaceConnectionInfo::Telnet {
            host: "192.0.2.20".into(),
            port: 23,
        })
    );
}

#[test]
fn prepare_saved_profile_allows_disabled_jump_profile_when_target_is_enabled() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "bastion".into(),
            connection_type: "ssh".into(),
            host: Some("198.51.100.10".into()),
            port: Some(2222),
            username: Some("jump".into()),
            auth_method: Some("password".into()),
            external_control_enabled: false,
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "inside".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            username: Some("admin".into()),
            auth_method: Some("password".into()),
            jump_profile_id: Some("bastion".into()),
            ..SavedConnection::default()
        },
    ];

    let prepared = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "inside".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap();

    match prepared.kind {
        PreparedConnectionKind::Ssh {
            jump_profile: Some(jump_profile),
            ..
        } => assert_eq!(jump_profile.id, "bastion"),
        other => panic!("expected SSH jump profile, got {other:?}"),
    }
}

#[test]
fn prepare_saved_profile_resolves_jump_profile() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "bastion".into(),
            connection_type: "ssh".into(),
            host: Some("198.51.100.10".into()),
            port: Some(2222),
            username: Some("jump".into()),
            auth_method: Some("password".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "inside".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            username: Some("admin".into()),
            auth_method: Some("password".into()),
            jump_profile_id: Some("bastion".into()),
            ..SavedConnection::default()
        },
    ];

    let prepared = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "inside".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap();

    match prepared.kind {
        PreparedConnectionKind::Ssh {
            jump_profile: Some(jump_profile),
            ..
        } => {
            assert_eq!(jump_profile.id, "bastion");
            assert_eq!(jump_profile.host, "198.51.100.10");
            assert_eq!(jump_profile.port, 2222);
            assert_eq!(jump_profile.username, "jump");
        }
        other => panic!("expected SSH jump profile, got {other:?}"),
    }
}

#[test]
fn prepare_saved_profile_rejects_invalid_jump_profiles() {
    let mut config = AppConfig::default();
    config.saved_connections = vec![
        SavedConnection {
            id: "inside".into(),
            connection_type: "ssh".into(),
            host: Some("192.0.2.10".into()),
            username: Some("admin".into()),
            auth_method: Some("password".into()),
            jump_profile_id: Some("inside".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "telnet-hop".into(),
            connection_type: "telnet".into(),
            host: Some("198.51.100.20".into()),
            ..SavedConnection::default()
        },
        SavedConnection {
            id: "nested-hop".into(),
            connection_type: "ssh".into(),
            host: Some("198.51.100.30".into()),
            username: Some("jump".into()),
            auth_method: Some("password".into()),
            jump_profile_id: Some("other-hop".into()),
            ..SavedConnection::default()
        },
    ];

    let self_ref = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "inside".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(self_ref.contains("reference itself"));

    config.saved_connections[0].jump_profile_id = Some("missing".into());
    let missing = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "inside".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(missing.contains("not found"));

    config.saved_connections[0].jump_profile_id = Some("telnet-hop".into());
    let telnet = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "inside".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(telnet.contains("SSH profiles"));

    config.saved_connections[0].jump_profile_id = Some("nested-hop".into());
    let nested = prepare_saved_profile_connection(
        &config,
        ConnectSavedProfileArgs {
            profile_id: "inside".into(),
            connection_type: SavedProfileConnectionType::Ssh,
            cols: None,
            rows: None,
        },
    )
    .unwrap_err();
    assert!(nested.contains("Nested"));
}

#[test]
fn ssh_credential_required_defers_password_prompt_and_rejects_missing_key() {
    assert_eq!(normalize_profile_auth_method(None).unwrap(), "auto");
    assert_eq!(
        normalize_profile_auth_method(Some("keyboard_interactive")).unwrap(),
        "keyboard_interactive"
    );
    assert!(!ssh_credential_required("password", None, None).unwrap());
    assert!(!ssh_credential_required("keyboard_interactive", None, None).unwrap());
    assert!(!ssh_credential_required("auto", None, None).unwrap());

    let error = ssh_credential_required("public_key", None, None).unwrap_err();
    assert!(error.contains("private key"));
}

#[test]
fn prepare_serial_console_uses_defaults_and_line_settings() {
    let ports = vec![serial::PortInfo {
        name: "COM3".into(),
        port_type: "USB".into(),
    }];

    let prepared = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: " COM3 ".into(),
            baud_rate: Some(115_200),
            data_bits: Some(7),
            parity: Some(McpSerialParity::Even),
            stop_bits: Some(2),
            flow_control: Some(McpSerialFlowControl::Hardware),
            terminal_mode: Some(McpTerminalMode::FujitsuSir),
            cols: Some(80),
            rows: Some(24),
        },
        &ports,
    )
    .unwrap();

    assert_eq!(prepared.port, "COM3");
    assert_eq!(prepared.config.baud_rate, 115_200);
    assert_eq!(prepared.config.data_bits, 7);
    assert_eq!(prepared.config.parity, "even");
    assert_eq!(prepared.config.stop_bits, 2);
    assert_eq!(prepared.config.flow_control, "hardware");
    assert_eq!(prepared.target, "COM3");
    assert_eq!(prepared.title, "COM3");
    assert_eq!(prepared.encoding, "utf-8");
    assert_eq!(prepared.terminal_mode, "fujitsu_sir");

    let junos = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(McpTerminalMode::JuniperJunos),
            cols: None,
            rows: None,
        },
        &ports,
    )
    .unwrap();

    assert_eq!(junos.terminal_mode, "juniper_junos");

    let awplus = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(McpTerminalMode::AlliedTelesisAwplus),
            cols: None,
            rows: None,
        },
        &ports,
    )
    .unwrap();

    assert_eq!(awplus.terminal_mode, "allied_telesis_awplus");

    let fitelnet = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(McpTerminalMode::FurukawaFitelnet),
            cols: None,
            rows: None,
        },
        &ports,
    )
    .unwrap();

    assert_eq!(fitelnet.terminal_mode, "furukawa_fitelnet");

    let defaulted = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        },
        &ports,
    )
    .unwrap();
    assert_eq!(defaulted.config, serial::SerialConfig::default());
    assert_eq!(defaulted.terminal_mode, "general");
}

#[test]
fn prepare_serial_console_rejects_missing_port_and_invalid_settings() {
    let ports = vec![serial::PortInfo {
        name: "COM5".into(),
        port_type: "USB".into(),
    }];

    let missing = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: "COM4".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        },
        &ports,
    )
    .unwrap_err();
    assert!(missing.contains("COM4"));
    assert!(missing.contains("COM5"));

    let invalid_data_bits = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: "COM5".into(),
            baud_rate: None,
            data_bits: Some(9),
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        },
        &ports,
    )
    .unwrap_err();
    assert!(invalid_data_bits.contains("data_bits"));

    let invalid_stop_bits = prepare_serial_console_connection(
        ConnectSerialConsoleArgs {
            port: "COM5".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: Some(3),
            flow_control: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        },
        &ports,
    )
    .unwrap_err();
    assert!(invalid_stop_bits.contains("stop_bits"));
}

#[tokio::test]
async fn service_connects_serial_console_and_registers_workspace_metadata() {
    let runtime = test_runtime_with_serial_ports(vec![serial::PortInfo {
        name: "COM3".into(),
        port_type: "USB".into(),
    }]);
    let service = McpTerminalService::new(runtime.clone());

    let result = service
        .connect_serial_console(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: Some(115_200),
            data_bits: Some(7),
            parity: Some(McpSerialParity::Even),
            stop_bits: Some(2),
            flow_control: Some(McpSerialFlowControl::Hardware),
            terminal_mode: Some(McpTerminalMode::AristaEos),
            cols: Some(90),
            rows: Some(30),
        })
        .await
        .unwrap();

    let session_id = result.session_id.as_str();
    assert_eq!(result.connection_type, "serial");
    assert_eq!(result.target, "COM3");
    assert_eq!(result.title, "COM3");
    assert_eq!(result.encoding, "utf-8");
    assert_eq!(result.terminal_mode, "arista_eos");

    let snapshot = runtime.workspace.snapshot_for_window("main".into()).await;
    assert_eq!(snapshot.tabs.len(), 1);
    assert_eq!(snapshot.tabs[0].session_id, session_id);
    assert_eq!(snapshot.tabs[0].title, "COM3");
    assert_eq!(snapshot.tabs[0].terminal_mode, "arista_eos");
    assert_eq!(snapshot.tabs[0].connection_info, None);
}

#[tokio::test]
async fn service_lists_terminal_sessions() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
        .await;
    let service = McpTerminalService::new(runtime);

    let result = service.list_terminal_sessions().await.unwrap();
    assert_eq!(result.sessions[0].session_id, "s1");
    assert_eq!(result.sessions[0].protocol, TerminalProtocol::Ssh);
    assert_eq!(result.sessions[0].encoding, "utf-8");
}

#[tokio::test]
async fn service_reads_terminal_output_with_multibyte_tail() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
        .await;
    runtime
        .terminals
        .append_output("s1", "example".as_bytes())
        .await;
    let service = McpTerminalService::new(runtime);

    let result = service
        .read_terminal_output(ReadTerminalOutputArgs::Recent {
            session_id: "s1".into(),
            max_chars: Some(2),
        })
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();
    assert_eq!(result["session_id"], "s1");
    assert_eq!(result["mode"], "recent");
    assert_eq!(result["output"], "le");
    assert_eq!(result["truncated"], true);
    assert_eq!(result["start_cursor"], 5);
    assert_eq!(result["cursor"], 7);
}

#[tokio::test]
async fn service_reads_terminal_output_in_delta_mode() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
        .await;
    runtime
        .terminals
        .append_output("s1", "abcalpha".as_bytes())
        .await;
    let service = McpTerminalService::new(runtime);

    let result = service
        .read_terminal_output(ReadTerminalOutputArgs::Delta {
            session_id: "s1".into(),
            cursor: 3,
            max_chars: Some(100),
        })
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();

    assert_eq!(result["mode"], "delta");
    assert_eq!(result["output"], "alpha");
    assert_eq!(result["start_cursor"], 3);
    assert_eq!(result["cursor"], 8);
}

#[tokio::test]
async fn service_reads_non_utf8_terminal_output() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session_with_encoding(
            "s1".into(),
            TerminalProtocol::Ssh,
            "host:22".into(),
            Some("shift-jis".into()),
        )
        .await;
    runtime
        .terminals
        .append_output("s1", &encoding_rs::SHIFT_JIS.encode("αβγδε").0.into_owned())
        .await;
    let service = McpTerminalService::new(runtime);

    let result = service
        .read_terminal_output(ReadTerminalOutputArgs::Recent {
            session_id: "s1".into(),
            max_chars: Some(100),
        })
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();
    assert_eq!(result["output"], "αβγδε");
    assert_eq!(result["cursor"], 5);
}

#[tokio::test]
async fn service_waits_for_matching_terminal_output() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
        .await;
    let terminals = runtime.terminals.clone();
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(10)).await;
        terminals.append_output("s1", b"router#").await;
    });
    let service = McpTerminalService::new(runtime);

    let result = service
        .read_terminal_output(ReadTerminalOutputArgs::Wait {
            session_id: "s1".into(),
            cursor: Some(0),
            contains: Some("router#".into()),
            timeout_ms: Some(500),
            max_chars: Some(100),
        })
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();

    assert_eq!(result["mode"], "wait");
    assert_eq!(result["matched"], true);
    assert_eq!(result["timed_out"], false);
    assert_eq!(result["output"], "router#");
}

#[tokio::test]
async fn service_wait_timeout_returns_latest_delta() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
        .await;
    runtime.terminals.append_output("s1", b"partial").await;
    let service = McpTerminalService::new(runtime);

    let result = service
        .read_terminal_output(ReadTerminalOutputArgs::Wait {
            session_id: "s1".into(),
            cursor: Some(0),
            contains: Some("missing".into()),
            timeout_ms: Some(1),
            max_chars: Some(100),
        })
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();

    assert_eq!(result["matched"], false);
    assert_eq!(result["timed_out"], true);
    assert_eq!(result["output"], "partial");
}

#[tokio::test]
async fn service_wait_without_cursor_starts_from_current_output() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
        .await;
    runtime.terminals.append_output("s1", b"old").await;
    let terminals = runtime.terminals.clone();
    tokio::spawn(async move {
        time::sleep(Duration::from_millis(10)).await;
        terminals.append_output("s1", b"new").await;
    });
    let service = McpTerminalService::new(runtime);

    let result = service
        .read_terminal_output(ReadTerminalOutputArgs::Wait {
            session_id: "s1".into(),
            cursor: None,
            contains: None,
            timeout_ms: Some(500),
            max_chars: Some(100),
        })
        .await
        .unwrap();
    let result = serde_json::to_value(result).unwrap();

    assert_eq!(result["matched"], true);
    assert_eq!(result["output"], "new");
    assert_eq!(result["start_cursor"], 3);
}

#[tokio::test]
async fn backend_rejects_invalid_read_terminal_output_mode_arguments() {
    let server = ExaTermMcpServer::with_service(ExternalControlService::new(test_runtime()));

    let missing_cursor = server
        .call_tool_json(
            "read_terminal_output",
            json!({
                "mode": "delta",
                "session_id": "s1",
            }),
        )
        .await
        .unwrap_err();
    assert!(missing_cursor.message.contains("cursor"));

    let unexpected_cursor = server
        .call_tool_json(
            "read_terminal_output",
            json!({
                "mode": "recent",
                "session_id": "s1",
                "cursor": 0,
            }),
        )
        .await
        .unwrap_err();
    assert!(unexpected_cursor.message.contains("unknown field"));
}

#[tokio::test]
async fn service_rejects_send_to_disconnected_session() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Telnet, "host:23".into())
        .await;
    runtime.terminals.mark_disconnected("s1").await;
    let service = McpTerminalService::new(runtime);

    let error = service
        .send_terminal_input(SendTerminalInputArgs {
            session_id: "s1".into(),
            data: "show version\n".into(),
        })
        .await
        .unwrap_err();
    assert!(matches!(&error, ExternalControlError::Unavailable(_)));
    assert!(error.message().contains("disconnected"));
}

#[tokio::test]
async fn service_starts_terminal_log_for_connected_session() {
    let (runtime, _config, _protocol, ui, _logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    register_test_terminal(&runtime, "s1", TerminalProtocol::Ssh, "host:22").await;
    let service = McpTerminalService::new(runtime);

    let result = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap();

    assert_eq!(result.session_id, "s1");
    assert!(result.started);
    assert!(!result.already_active);
    assert_eq!(result.log_mode, "manual");
    assert!(result.file_path.ends_with(".log"));
    let calls = ui.calls.lock().unwrap();
    let TestUiCall::LogControl(window_id, event, payload) = &calls[0] else {
        panic!("expected log control request");
    };
    assert_eq!(window_id, "main");
    assert_eq!(event, "external-control://log-start-request");
    assert_eq!(payload.session_id, "s1");
    assert_eq!(payload.connection_type, "ssh");
    assert_eq!(payload.target, "host:22");
}

#[tokio::test]
async fn service_log_start_preserves_negative_and_missing_path_ack_errors() {
    let (missing_runtime, _config, _protocol, missing_ui, _logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    register_test_terminal(
        &missing_runtime,
        "missing-path",
        TerminalProtocol::Telnet,
        "host:23",
    )
    .await;
    *missing_ui.log_result.lock().unwrap() =
        Some(Ok(ExternalControlLogControlAck { file_path: None }));
    let missing_error = ExternalControlService::new(missing_runtime)
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "missing-path".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(missing_error, ExternalControlError::Internal(_)));
    assert!(missing_error
        .message()
        .contains("did not include a log file path"));

    let (negative_runtime, _config, _protocol, negative_ui, _logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    register_test_terminal(
        &negative_runtime,
        "negative",
        TerminalProtocol::Serial,
        "COM3",
    )
    .await;
    *negative_ui.log_result.lock().unwrap() = Some(Err("GUI rejected log start".into()));
    let negative_error = ExternalControlService::new(negative_runtime)
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "negative".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap_err();
    assert!(negative_error.message().contains("GUI rejected log start"));
}

#[tokio::test]
async fn service_start_terminal_log_rejects_missing_and_disconnected_sessions() {
    let runtime = test_runtime();
    register_test_terminal(&runtime, "s1", TerminalProtocol::Serial, "COM1").await;
    runtime.terminals.mark_disconnected("s1").await;
    let service = McpTerminalService::new(runtime);

    let missing = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "missing".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(&missing, ExternalControlError::NotFound(_)));
    assert!(missing.message().contains("not found"));

    let disconnected = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(
        &disconnected,
        ExternalControlError::Unavailable(_)
    ));
    assert!(disconnected.message().contains("disconnected"));
}

#[tokio::test]
async fn service_reports_missing_logger_as_internal_error() {
    let runtime = test_runtime_parts(AppConfig::default(), Vec::new(), None).0;
    register_test_terminal(&runtime, "s1", TerminalProtocol::Ssh, "host:22").await;
    let service = McpTerminalService::new(runtime);

    let error = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap_err();

    assert!(matches!(&error, ExternalControlError::Internal(_)));
}

#[tokio::test]
async fn service_start_terminal_log_reports_already_active() {
    let runtime = test_runtime();
    register_test_terminal(&runtime, "s1", TerminalProtocol::Telnet, "host:23").await;
    let service = McpTerminalService::new(runtime);

    let first = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap();
    let second = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap();

    assert!(!second.started);
    assert!(second.already_active);
    assert_eq!(second.file_path, first.file_path);
}

#[tokio::test]
async fn service_reports_inactive_active_paused_and_auto_log_status() {
    let (runtime, _config, _protocol, _ui, logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    register_test_terminal(&runtime, "s1", TerminalProtocol::Ssh, "host:22").await;
    let service = McpTerminalService::new(runtime.clone());

    let inactive = service
        .get_terminal_log_status(TerminalLogSessionArgs {
            session_id: "s1".into(),
        })
        .await
        .unwrap();
    assert_eq!(inactive.state, TerminalLogState::Inactive);
    assert_eq!(inactive.file_path, None);
    assert_eq!(inactive.log_mode, None);

    let auto_path = logger
        .start_auto_log("s1".into(), "ssh".into(), "host:22".into())
        .await
        .unwrap();
    let active = service
        .get_terminal_log_status(TerminalLogSessionArgs {
            session_id: "s1".into(),
        })
        .await
        .unwrap();
    assert_eq!(active.state, TerminalLogState::Active);
    assert_eq!(active.file_path.as_deref(), Some(auto_path.as_str()));
    assert_eq!(active.log_mode.as_deref(), Some("auto"));

    let tab = runtime.workspace.tab_for_session("s1").await.unwrap();
    runtime
        .workspace
        .update_tab_metadata(
            tab.tab_id,
            WorkspaceTabMetadataPatch {
                title: None,
                encoding: None,
                terminal_mode: None,
                is_connected: None,
                is_manual_logging: Some(true),
                is_manual_logging_paused: Some(true),
                manual_log_file_path: Some(auto_path.clone()),
            },
        )
        .await
        .unwrap();
    let paused = service
        .get_terminal_log_status(TerminalLogSessionArgs {
            session_id: "s1".into(),
        })
        .await
        .unwrap();
    assert_eq!(paused.state, TerminalLogState::Paused);
    assert_eq!(paused.log_mode.as_deref(), Some("auto"));
}

#[tokio::test]
async fn service_ignores_stale_paused_metadata_without_an_active_logger() {
    let runtime = test_runtime();
    register_test_terminal(&runtime, "s1", TerminalProtocol::Ssh, "host:22").await;
    let tab = runtime.workspace.tab_for_session("s1").await.unwrap();
    runtime
        .workspace
        .update_tab_metadata(
            tab.tab_id,
            WorkspaceTabMetadataPatch {
                title: None,
                encoding: None,
                terminal_mode: None,
                is_connected: None,
                is_manual_logging: Some(true),
                is_manual_logging_paused: Some(true),
                manual_log_file_path: Some("C:\\stale.log".into()),
            },
        )
        .await
        .unwrap();

    let status = ExternalControlService::new(runtime)
        .get_terminal_log_status(TerminalLogSessionArgs {
            session_id: "s1".into(),
        })
        .await
        .unwrap();

    assert_eq!(status.state, TerminalLogState::Inactive);
    assert_eq!(status.file_path, None);
    assert_eq!(status.log_mode, None);
}

#[tokio::test]
async fn service_pause_and_resume_are_idempotent_and_owner_window_scoped() {
    let (runtime, _config, _protocol, ui, _logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    register_test_terminal(&runtime, "s1", TerminalProtocol::Ssh, "host:22").await;
    let service = McpTerminalService::new(runtime.clone());
    service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap();

    let paused = service
        .set_terminal_log_paused(
            TerminalLogSessionArgs {
                session_id: "s1".into(),
            },
            true,
        )
        .await
        .unwrap();
    assert!(paused.changed);
    assert_eq!(paused.state, TerminalLogState::Paused);
    assert!(ui.calls.lock().unwrap().iter().any(|call| matches!(
        call,
        TestUiCall::LogControl(window_id, event, payload)
            if window_id == "main"
                && event == "external-control://log-pause-request"
                && payload.session_id == "s1"
    )));

    let tab = runtime.workspace.tab_for_session("s1").await.unwrap();
    runtime
        .workspace
        .update_tab_metadata(
            tab.tab_id,
            WorkspaceTabMetadataPatch {
                title: None,
                encoding: None,
                terminal_mode: None,
                is_connected: None,
                is_manual_logging: Some(true),
                is_manual_logging_paused: Some(true),
                manual_log_file_path: paused.file_path.clone(),
            },
        )
        .await
        .unwrap();
    let pause_again = service
        .set_terminal_log_paused(
            TerminalLogSessionArgs {
                session_id: "s1".into(),
            },
            true,
        )
        .await
        .unwrap();
    assert!(!pause_again.changed);

    let resumed = service
        .set_terminal_log_paused(
            TerminalLogSessionArgs {
                session_id: "s1".into(),
            },
            false,
        )
        .await
        .unwrap();
    assert!(resumed.changed);
    assert_eq!(resumed.state, TerminalLogState::Active);
}

#[tokio::test]
async fn service_pause_rejects_inactive_and_disconnected_sessions() {
    let runtime = test_runtime();
    register_test_terminal(&runtime, "s1", TerminalProtocol::Telnet, "host:23").await;
    let service = McpTerminalService::new(runtime.clone());

    let inactive = service
        .set_terminal_log_paused(
            TerminalLogSessionArgs {
                session_id: "s1".into(),
            },
            true,
        )
        .await
        .unwrap_err();
    assert!(matches!(inactive, ExternalControlError::Unavailable(_)));
    assert!(inactive.message().contains("not active"));

    runtime.terminals.mark_disconnected("s1").await;
    let disconnected = service
        .set_terminal_log_paused(
            TerminalLogSessionArgs {
                session_id: "s1".into(),
            },
            false,
        )
        .await
        .unwrap_err();
    assert!(matches!(disconnected, ExternalControlError::Unavailable(_)));
    assert!(disconnected.message().contains("disconnected"));
}

#[tokio::test]
async fn service_custom_log_path_is_propagated_and_conflicts_are_rejected() {
    let (runtime, _config, _protocol, ui, _logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    register_test_terminal(&runtime, "s1", TerminalProtocol::Serial, "COM3").await;
    let service = McpTerminalService::new(runtime);
    let file_path = std::env::temp_dir()
        .join(format!("exaterm_cli_log_{}.log", Uuid::new_v4()))
        .to_string_lossy()
        .to_string();

    let first = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: Some(file_path.clone()),
            write_mode: Some(ExternalControlLogWriteMode::Append),
        })
        .await
        .unwrap();
    assert_eq!(first.file_path, file_path);
    let calls = ui.calls.lock().unwrap();
    let payload = calls
        .iter()
        .find_map(|call| match call {
            TestUiCall::LogControl(_, event, payload)
                if event == "external-control://log-start-request" =>
            {
                Some(payload)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(payload.file_path.as_deref(), Some(first.file_path.as_str()));
    assert_eq!(
        payload.write_mode,
        Some(ExternalControlLogWriteMode::Append)
    );
    drop(calls);

    let same = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: Some(first.file_path.clone()),
            write_mode: Some(ExternalControlLogWriteMode::Overwrite),
        })
        .await
        .unwrap();
    assert!(same.already_active);

    let conflict = service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: Some(format!("{}.different", first.file_path)),
            write_mode: Some(ExternalControlLogWriteMode::Overwrite),
        })
        .await
        .unwrap_err();
    assert!(matches!(conflict, ExternalControlError::Unavailable(_)));
    assert!(conflict.message().contains("different file path"));
}

#[tokio::test]
async fn service_revalidates_custom_log_path_options() {
    let runtime = test_runtime();
    register_test_terminal(&runtime, "s1", TerminalProtocol::Ssh, "host:22").await;
    let service = McpTerminalService::new(runtime);

    for args in [
        StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: Some("relative.log".into()),
            write_mode: Some(ExternalControlLogWriteMode::Overwrite),
        },
        StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: Some(r"C:\logs\session.log".into()),
            write_mode: None,
        },
        StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: Some(ExternalControlLogWriteMode::Append),
        },
    ] {
        let error = service.start_terminal_log(args).await.unwrap_err();
        assert!(matches!(error, ExternalControlError::InvalidArguments(_)));
    }
}

#[tokio::test]
async fn service_stop_terminal_log_stops_active_and_reports_inactive() {
    let (runtime, _config, _protocol, ui, _logger, _trace) =
        test_runtime_parts(AppConfig::default(), Vec::new(), Some(test_logger()));
    register_test_terminal(&runtime, "s1", TerminalProtocol::Ssh, "host:22").await;
    let service = McpTerminalService::new(runtime);

    let inactive = service
        .stop_terminal_log(StopTerminalLogArgs {
            session_id: "s1".into(),
        })
        .await
        .unwrap();
    assert!(!inactive.stopped);
    assert!(inactive.already_inactive);

    service
        .start_terminal_log(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
        .await
        .unwrap();
    service.runtime.terminals.mark_disconnected("s1").await;

    let stopped = service
        .stop_terminal_log(StopTerminalLogArgs {
            session_id: "s1".into(),
        })
        .await
        .unwrap();
    assert!(stopped.stopped);
    assert!(!stopped.already_inactive);
    assert!(ui.calls.lock().unwrap().iter().any(|call| matches!(
        call,
        TestUiCall::LogControl(window_id, event, payload)
            if window_id == "main"
                && event == "external-control://log-stop-request"
                && payload.session_id == "s1"
    )));

    let inactive_again = service
        .stop_terminal_log(StopTerminalLogArgs {
            session_id: "s1".into(),
        })
        .await
        .unwrap();
    assert!(inactive_again.already_inactive);
}

#[tokio::test]
async fn service_rejects_empty_run_terminal_command() {
    let runtime = test_runtime();
    runtime
        .terminals
        .register_session("s1".into(), TerminalProtocol::Ssh, "host:22".into())
        .await;
    let service = McpTerminalService::new(runtime);

    let error = service
        .run_terminal_command(RunTerminalCommandArgs {
            session_id: "s1".into(),
            command: "   ".into(),
            append_newline: None,
            wait_contains: None,
            timeout_ms: None,
            settle_ms: None,
            max_chars: None,
        })
        .await
        .unwrap_err();

    assert!(matches!(&error, ExternalControlError::InvalidArguments(_)));
    assert!(error.message().contains("must not be empty"));
}
