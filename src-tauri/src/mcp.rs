use std::sync::Arc;

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

use crate::config::McpConfig;
use crate::serial::{self, SerialState};
use crate::ssh::{self, SshState};
use crate::telnet::{self, TelnetState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};

const DEFAULT_READ_CHARS: usize = 2_000;
const MAX_READ_CHARS: usize = 20_000;

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
        let max_chars = args
            .max_chars
            .unwrap_or(DEFAULT_READ_CHARS)
            .clamp(1, MAX_READ_CHARS);
        let snapshot = self
            .runtime
            .terminals
            .read_output(&args.session_id, max_chars)
            .await
            .map_err(invalid_params)?;

        Ok(json!(snapshot))
    }

    async fn send_terminal_input(&self, args: SendTerminalInputArgs) -> Result<Value, McpError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| invalid_params("セッションが見つかりません"))?;

        if info.status != TerminalStatus::Connected {
            return Err(invalid_params("セッションは切断済みです"));
        }

        match info.protocol {
            TerminalProtocol::Ssh => {
                ssh::write_data(&self.runtime.ssh, &args.session_id, args.data).await
            }
            TerminalProtocol::Serial => {
                serial::write_data(&self.runtime.serial, &args.session_id, args.data).await
            }
            TerminalProtocol::Telnet => {
                telnet::write_data(&self.runtime.telnet, &args.session_id, args.data).await
            }
        }
        .map_err(internal_error)?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
        }))
    }
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
struct SendTerminalInputArgs {
    /// Session ID returned by list_terminal_sessions.
    session_id: String,
    /// Text to send to the terminal. Include newline characters when needed.
    data: String,
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
                "send_terminal_input"
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
    }
}
