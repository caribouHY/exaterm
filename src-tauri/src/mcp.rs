use std::{sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::time;

use crate::config::McpConfig;
use crate::serial::{self, SerialState};
use crate::ssh::{self, SshState};
use crate::telnet::{self, TelnetState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};

const DEFAULT_READ_CHARS: usize = 2_000;
const MAX_READ_CHARS: usize = 20_000;
const MAX_INPUT_CHARS: usize = 20_000;
const DEFAULT_WAIT_TIMEOUT_MS: u64 = 10_000;
const MAX_WAIT_TIMEOUT_MS: u64 = 60_000;
const DEFAULT_SETTLE_MS: u64 = 250;
const MAX_SETTLE_MS: u64 = 5_000;

#[derive(Clone)]
pub struct McpRuntime {
    pub config: McpConfig,
    pub terminals: TerminalControlState,
    pub ssh: SshState,
    pub serial: SerialState,
    pub telnet: TelnetState,
}

#[derive(Clone)]
struct McpTerminalService {
    runtime: McpRuntime,
}

impl McpTerminalService {
    fn new(runtime: McpRuntime) -> Self {
        Self { runtime }
    }

    async fn list_terminal_sessions(&self) -> Result<Value, McpError> {
        let sessions = self.runtime.terminals.list_sessions().await;
        Ok(json!({
            "sessions": sessions,
        }))
    }

    async fn read_terminal_output(&self, args: ReadTerminalOutputArgs) -> Result<Value, McpError> {
        let snapshot = self
            .runtime
            .terminals
            .read_output(&args.session_id, normalize_max_chars(args.max_chars))
            .await
            .map_err(invalid_params)?;

        Ok(json!(snapshot))
    }

    async fn read_terminal_output_delta(
        &self,
        args: ReadTerminalOutputDeltaArgs,
    ) -> Result<Value, McpError> {
        let snapshot = self
            .runtime
            .terminals
            .read_output_delta(
                &args.session_id,
                args.cursor,
                normalize_max_chars(args.max_chars),
            )
            .await
            .map_err(invalid_params)?;

        Ok(json!(snapshot))
    }

    async fn wait_terminal_output(&self, args: WaitTerminalOutputArgs) -> Result<Value, McpError> {
        let start_cursor = match args.cursor {
            Some(cursor) => cursor,
            None => self
                .runtime
                .terminals
                .cursor(&args.session_id)
                .await
                .map_err(invalid_params)?,
        };
        let contains = args.contains.filter(|value| !value.is_empty());

        wait_for_terminal_output(
            &self.runtime.terminals,
            &args.session_id,
            start_cursor,
            contains.as_deref(),
            normalize_max_chars(args.max_chars),
            normalize_timeout_ms(args.timeout_ms),
        )
        .await
    }

    async fn send_terminal_input(&self, args: SendTerminalInputArgs) -> Result<Value, McpError> {
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, args.data).await?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
        }))
    }

    async fn run_terminal_command(&self, args: RunTerminalCommandArgs) -> Result<Value, McpError> {
        if args.command.trim().is_empty() {
            return Err(invalid_params("送信するコマンドが空です"));
        }
        if args.command.chars().count() > MAX_INPUT_CHARS {
            return Err(invalid_params(format!(
                "コマンドは{}文字以内で指定してください",
                MAX_INPUT_CHARS
            )));
        }

        let start_cursor = self
            .runtime
            .terminals
            .cursor(&args.session_id)
            .await
            .map_err(invalid_params)?;
        let data = if args.append_newline.unwrap_or(true) {
            format!("{}\n", args.command)
        } else {
            args.command.clone()
        };
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, data).await?;

        let max_chars = normalize_max_chars(args.max_chars);
        let wait_result = wait_for_terminal_output(
            &self.runtime.terminals,
            &args.session_id,
            start_cursor,
            args.wait_contains
                .as_deref()
                .filter(|value| !value.is_empty()),
            max_chars,
            normalize_timeout_ms(args.timeout_ms),
        )
        .await?;

        let settle_ms = args
            .settle_ms
            .unwrap_or(DEFAULT_SETTLE_MS)
            .clamp(0, MAX_SETTLE_MS);
        if wait_result["timed_out"] == false && settle_ms > 0 {
            time::sleep(Duration::from_millis(settle_ms)).await;
        }

        let snapshot = self
            .runtime
            .terminals
            .read_output_delta(&args.session_id, start_cursor, max_chars)
            .await
            .map_err(invalid_params)?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
            "matched": wait_result["matched"],
            "timed_out": wait_result["timed_out"],
            "output": snapshot.output,
            "truncated": snapshot.truncated,
            "available_chars": snapshot.available_chars,
            "start_cursor": snapshot.start_cursor,
            "cursor": snapshot.cursor,
        }))
    }
}

async fn send_terminal_input_to_runtime(
    runtime: &McpRuntime,
    session_id: &str,
    data: String,
) -> Result<(), McpError> {
    if data.chars().count() > MAX_INPUT_CHARS {
        return Err(invalid_params(format!(
            "入力は{}文字以内で指定してください",
            MAX_INPUT_CHARS
        )));
    }

    let info = runtime
        .terminals
        .session_info(session_id)
        .await
        .ok_or_else(|| invalid_params("セッションが見つかりません"))?;

    if info.status != TerminalStatus::Connected {
        return Err(invalid_params("セッションは切断済みです"));
    }

    match info.protocol {
        TerminalProtocol::Ssh => ssh::write_data(&runtime.ssh, session_id, data).await,
        TerminalProtocol::Serial => serial::write_data(&runtime.serial, session_id, data).await,
        TerminalProtocol::Telnet => telnet::write_data(&runtime.telnet, session_id, data).await,
    }
    .map_err(internal_error)
}

async fn wait_for_terminal_output(
    terminals: &TerminalControlState,
    session_id: &str,
    start_cursor: usize,
    contains: Option<&str>,
    max_chars: usize,
    timeout_ms: u64,
) -> Result<Value, McpError> {
    let deadline = time::Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let output_changed = terminals.output_change_notified();
        tokio::pin!(output_changed);

        let snapshot = terminals
            .read_output_delta(session_id, start_cursor, max_chars)
            .await
            .map_err(invalid_params)?;
        let matched = match contains {
            Some(needle) => snapshot.output.contains(needle),
            None => !snapshot.output.is_empty(),
        };

        if matched {
            return Ok(json!({
                "session_id": snapshot.session_id,
                "matched": true,
                "timed_out": false,
                "output": snapshot.output,
                "truncated": snapshot.truncated,
                "available_chars": snapshot.available_chars,
                "start_cursor": snapshot.start_cursor,
                "cursor": snapshot.cursor,
            }));
        }

        let now = time::Instant::now();
        if now >= deadline {
            return Ok(json!({
                "session_id": snapshot.session_id,
                "matched": false,
                "timed_out": true,
                "output": snapshot.output,
                "truncated": snapshot.truncated,
                "available_chars": snapshot.available_chars,
                "start_cursor": snapshot.start_cursor,
                "cursor": snapshot.cursor,
            }));
        }

        let remaining = deadline - now;
        let _ = time::timeout(remaining, &mut output_changed).await;
    }
}

fn normalize_max_chars(max_chars: Option<usize>) -> usize {
    max_chars
        .unwrap_or(DEFAULT_READ_CHARS)
        .clamp(1, MAX_READ_CHARS)
}

fn normalize_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(DEFAULT_WAIT_TIMEOUT_MS)
        .clamp(1, MAX_WAIT_TIMEOUT_MS)
}

#[derive(Clone)]
struct ExaTermMcpServer {
    service: McpTerminalService,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ExaTermMcpServer {
    fn new(runtime: McpRuntime) -> Self {
        Self {
            service: McpTerminalService::new(runtime),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "list_terminal_sessions",
        description = "List ExaTerm terminal sessions that were opened by the user."
    )]
    async fn list_terminal_sessions(&self) -> Result<CallToolResult, McpError> {
        self.service
            .list_terminal_sessions()
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "read_terminal_output",
        description = "Read recent output from an ExaTerm terminal session."
    )]
    async fn read_terminal_output(
        &self,
        Parameters(args): Parameters<ReadTerminalOutputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .read_terminal_output(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "read_terminal_output_delta",
        description = "Read output written after a previously returned ExaTerm terminal cursor."
    )]
    async fn read_terminal_output_delta(
        &self,
        Parameters(args): Parameters<ReadTerminalOutputDeltaArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .read_terminal_output_delta(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "wait_terminal_output",
        description = "Wait until an ExaTerm terminal session produces new output or a target string appears."
    )]
    async fn wait_terminal_output(
        &self,
        Parameters(args): Parameters<WaitTerminalOutputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .wait_terminal_output(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "send_terminal_input",
        description = "Send text to an existing connected ExaTerm terminal session."
    )]
    async fn send_terminal_input(
        &self,
        Parameters(args): Parameters<SendTerminalInputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .send_terminal_input(args)
            .await
            .and_then(structured_tool_result)
    }

    #[tool(
        name = "run_terminal_command",
        description = "Send a command to an existing connected ExaTerm terminal session, wait for output, and return the output delta."
    )]
    async fn run_terminal_command(
        &self,
        Parameters(args): Parameters<RunTerminalCommandArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.service
            .run_terminal_command(args)
            .await
            .and_then(structured_tool_result)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ExaTermMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("exaterm", env!("CARGO_PKG_VERSION")))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadTerminalOutputArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReadTerminalOutputDeltaArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Cursor returned by read_terminal_output, read_terminal_output_delta, wait_terminal_output, or run_terminal_command.
    cursor: usize,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WaitTerminalOutputArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Cursor to wait from. When omitted, waits from the current output cursor.
    cursor: Option<usize>,
    /// Optional substring to wait for in the output delta.
    contains: Option<String>,
    /// Maximum wait time in milliseconds.
    #[schemars(range(min = 1, max = MAX_WAIT_TIMEOUT_MS))]
    timeout_ms: Option<u64>,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SendTerminalInputArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Text to send to the terminal. Include newline characters when needed.
    data: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RunTerminalCommandArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Command text to send to the terminal.
    command: String,
    /// Append a newline after the command. Defaults to true.
    append_newline: Option<bool>,
    /// Optional substring to wait for in the output delta after sending.
    wait_contains: Option<String>,
    /// Maximum wait time in milliseconds.
    #[schemars(range(min = 1, max = MAX_WAIT_TIMEOUT_MS))]
    timeout_ms: Option<u64>,
    /// Additional quiet period after a match before the final output delta is returned.
    #[schemars(range(min = 0, max = MAX_SETTLE_MS))]
    settle_ms: Option<u64>,
    /// Maximum number of recent characters to return.
    #[schemars(range(min = 1, max = MAX_READ_CHARS))]
    max_chars: Option<usize>,
}

type ExaTermMcpHttpService = StreamableHttpService<ExaTermMcpServer, LocalSessionManager>;

pub fn spawn_mcp_server(runtime: McpRuntime) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_mcp_server(runtime).await {
            log::error!("MCP server stopped: {}", error);
        }
    });
}

async fn run_mcp_server(runtime: McpRuntime) -> Result<(), String> {
    let address = format!("{}:{}", runtime.config.host, runtime.config.port);
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|error| format!("MCP bind error on {}: {}", address, error))?;

    let mcp_runtime = runtime.clone();
    let service = StreamableHttpService::new(
        move || Ok(ExaTermMcpServer::new(mcp_runtime.clone())),
        Arc::new(LocalSessionManager::default()),
        mcp_server_config(&runtime.config.host),
    );
    let app = Router::new()
        .route("/mcp", any(mcp_http_handler))
        .with_state(service);

    log::info!("MCP server listening on http://{}/mcp", address);
    axum::serve(listener, app)
        .await
        .map_err(|error| format!("MCP serve error on {}: {}", address, error))
}

fn mcp_server_config(configured_host: &str) -> StreamableHttpServerConfig {
    let mut allowed_hosts = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let configured_host = configured_host.trim();
    if !configured_host.is_empty()
        && configured_host != "0.0.0.0"
        && configured_host != "::"
        && !allowed_hosts.iter().any(|host| host == configured_host)
    {
        allowed_hosts.push(configured_host.to_string());
    }

    StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(allowed_hosts)
}

async fn mcp_http_handler(
    State(service): State<ExaTermMcpHttpService>,
    request: Request<Body>,
) -> Response {
    if !origin_is_allowed(request.headers()) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    service.handle(request).await.map(Body::new)
}

fn structured_tool_result(value: Value) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| internal_error(format!("Serialize MCP tool result failed: {error}")))?;
    let mut result = CallToolResult::structured(value);
    result.content = vec![Content::text(text)];
    Ok(result)
}

fn invalid_params(message: impl Into<String>) -> McpError {
    McpError::invalid_params(message.into(), None)
}

fn internal_error(message: impl Into<String>) -> McpError {
    McpError::internal_error(message.into(), None)
}

fn origin_is_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin") else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };

    if origin == "null" {
        return true;
    }

    let Some(origin_without_scheme) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = origin_without_scheme
        .split('/')
        .next()
        .unwrap_or(origin_without_scheme);
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        authority.split(':').next().unwrap_or(authority)
    };

    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_control::TerminalProtocol;
    use axum::body::to_bytes;

    fn test_runtime() -> McpRuntime {
        McpRuntime {
            config: McpConfig::default(),
            terminals: TerminalControlState::new(),
            ssh: SshState::new(),
            serial: SerialState::new(),
            telnet: TelnetState::new(),
        }
    }

    fn test_http_service(runtime: McpRuntime) -> ExaTermMcpHttpService {
        let mcp_runtime = runtime.clone();
        StreamableHttpService::new(
            move || Ok(ExaTermMcpServer::new(mcp_runtime.clone())),
            Arc::new(LocalSessionManager::default()),
            mcp_server_config(&runtime.config.host),
        )
    }

    async fn post_mcp(service: ExaTermMcpHttpService, body: Value) -> (StatusCode, Value) {
        let request = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("host", "127.0.0.1:8765")
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = service.handle(request).await.map(Body::new);
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value = serde_json::from_slice(&body).unwrap_or_else(|error| {
            panic!(
                "MCP response was not JSON: status={status}, error={error}, body={:?}",
                String::from_utf8_lossy(&body)
            )
        });
        (status, value)
    }

    #[test]
    fn rejects_non_local_origins() {
        let mut headers = HeaderMap::new();
        headers.insert("origin", "https://example.com".parse().unwrap());
        assert!(!origin_is_allowed(&headers));

        headers.insert("origin", "http://127.0.0.1:8765".parse().unwrap());
        assert!(origin_is_allowed(&headers));

        headers.insert("origin", "http://localhost.evil.test".parse().unwrap());
        assert!(!origin_is_allowed(&headers));

        headers.insert("origin", "http://[::1]:8765".parse().unwrap());
        assert!(origin_is_allowed(&headers));
    }

    #[test]
    fn mcp_server_config_uses_stateless_json_responses() {
        let config = mcp_server_config("127.0.0.1");
        assert!(!config.stateful_mode);
        assert!(config.json_response);
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
        assert_eq!(result["sessions"][0]["session_id"], "s1");
        assert_eq!(result["sessions"][0]["protocol"], "ssh");
        assert_eq!(result["sessions"][0]["status"], "connected");
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
            .append_output("s1", "こんにちは世界".as_bytes())
            .await;
        let service = McpTerminalService::new(runtime);

        let result = service
            .read_terminal_output(ReadTerminalOutputArgs {
                session_id: "s1".into(),
                max_chars: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(result["session_id"], "s1");
        assert_eq!(result["output"], "世界");
        assert_eq!(result["truncated"], true);
        assert_eq!(result["start_cursor"], 5);
        assert_eq!(result["cursor"], 7);
    }

    #[tokio::test]
    async fn service_reads_terminal_output_delta() {
        let runtime = test_runtime();
        runtime
            .terminals
            .register_session("s1".into(), TerminalProtocol::Serial, "COM1".into())
            .await;
        runtime
            .terminals
            .append_output("s1", "abcこんにちは".as_bytes())
            .await;
        let service = McpTerminalService::new(runtime);

        let result = service
            .read_terminal_output_delta(ReadTerminalOutputDeltaArgs {
                session_id: "s1".into(),
                cursor: 3,
                max_chars: Some(100),
            })
            .await
            .unwrap();

        assert_eq!(result["output"], "こんにちは");
        assert_eq!(result["start_cursor"], 3);
        assert_eq!(result["cursor"], 8);
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
            .wait_terminal_output(WaitTerminalOutputArgs {
                session_id: "s1".into(),
                cursor: Some(0),
                contains: Some("router#".into()),
                timeout_ms: Some(500),
                max_chars: Some(100),
            })
            .await
            .unwrap();

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
            .wait_terminal_output(WaitTerminalOutputArgs {
                session_id: "s1".into(),
                cursor: Some(0),
                contains: Some("missing".into()),
                timeout_ms: Some(1),
                max_chars: Some(100),
            })
            .await
            .unwrap();

        assert_eq!(result["matched"], false);
        assert_eq!(result["timed_out"], true);
        assert_eq!(result["output"], "partial");
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
        assert!(error.message.contains("切断済み"));
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

        assert!(error.message.contains("空"));
    }

    #[tokio::test]
    async fn rmcp_http_service_handles_initialize_and_tools_list() {
        let service = test_http_service(test_runtime());

        let (status, initialize) = post_mcp(
            service.clone(),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "test-client",
                        "version": "0.0.0"
                    }
                }
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(initialize["result"]["serverInfo"]["name"], "exaterm");

        let (status, tools) = post_mcp(
            service,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let names = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "list_terminal_sessions",
                "read_terminal_output",
                "read_terminal_output_delta",
                "run_terminal_command",
                "send_terminal_input",
                "wait_terminal_output",
            ]
        );

        let read_tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "read_terminal_output")
            .unwrap();
        let max_chars_schema = &read_tool["inputSchema"]["properties"]["max_chars"];
        assert_eq!(max_chars_schema["minimum"], 1);
        assert_eq!(max_chars_schema["maximum"], MAX_READ_CHARS);

        let wait_tool = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "wait_terminal_output")
            .unwrap();
        let timeout_schema = &wait_tool["inputSchema"]["properties"]["timeout_ms"];
        assert_eq!(timeout_schema["minimum"], 1);
        assert_eq!(timeout_schema["maximum"], MAX_WAIT_TIMEOUT_MS);
    }
}
