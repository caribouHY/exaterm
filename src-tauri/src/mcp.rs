use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::config::McpConfig;
use crate::serial::{self, SerialState};
use crate::ssh::{self, SshState};
use crate::telnet::{self, TelnetState};
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const MAX_HTTP_HEADER_BYTES: usize = 64 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
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

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

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
    log::info!("MCP server listening on http://{}/mcp", address);

    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|error| format!("MCP accept error: {}", error))?;
        let runtime = runtime.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = handle_connection(stream, runtime).await {
                log::warn!("MCP request failed: {}", error);
            }
        });
    }
}

async fn handle_connection(mut stream: TcpStream, runtime: McpRuntime) -> Result<(), String> {
    let request = read_http_request(&mut stream).await?;
    let response = handle_http_request(request, runtime).await;
    stream
        .write_all(&response)
        .await
        .map_err(|error| format!("MCP response write error: {}", error))
}

async fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];
    let header_end = loop {
        let n = stream
            .read(&mut temp)
            .await
            .map_err(|error| format!("MCP request read error: {}", error))?;
        if n == 0 {
            return Err("MCP request closed before headers".into());
        }
        buffer.extend_from_slice(&temp[..n]);
        if buffer.len() > MAX_HTTP_HEADER_BYTES {
            return Err("MCP request headers are too large".into());
        }
        if let Some(index) = find_header_end(&buffer) {
            break index;
        }
    };

    let headers_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "MCP request line is missing".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "MCP request method is missing".to_string())?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| "MCP request path is missing".to_string())?
        .to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > MAX_HTTP_BODY_BYTES {
        return Err("MCP request body is too large".into());
    }

    let body_start = header_end + 4;
    let mut body = buffer[body_start..].to_vec();
    while body.len() < content_length {
        let n = stream
            .read(&mut temp)
            .await
            .map_err(|error| format!("MCP body read error: {}", error))?;
        if n == 0 {
            return Err("MCP request closed before body completed".into());
        }
        body.extend_from_slice(&temp[..n]);
    }
    body.truncate(content_length);

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

async fn handle_http_request(request: HttpRequest, runtime: McpRuntime) -> Vec<u8> {
    if request.path != "/mcp" {
        return http_response(404, "Not Found", &[], b"not found".to_vec());
    }

    if !origin_is_allowed(&request.headers) {
        return http_response(403, "Forbidden", &[], b"forbidden".to_vec());
    }

    match request.method.as_str() {
        "POST" => handle_mcp_post(request, runtime).await,
        "GET" | "DELETE" => http_response(
            405,
            "Method Not Allowed",
            &[("Allow", "POST, OPTIONS")],
            b"method not allowed".to_vec(),
        ),
        "OPTIONS" => http_response(
            204,
            "No Content",
            &[
                ("Allow", "POST, OPTIONS"),
                ("Access-Control-Allow-Origin", "null"),
                (
                    "Access-Control-Allow-Headers",
                    "content-type, mcp-method, mcp-name, mcp-protocol-version",
                ),
            ],
            Vec::new(),
        ),
        _ => http_response(
            405,
            "Method Not Allowed",
            &[("Allow", "POST, OPTIONS")],
            Vec::new(),
        ),
    }
}

async fn handle_mcp_post(request: HttpRequest, runtime: McpRuntime) -> Vec<u8> {
    let parsed = serde_json::from_slice::<JsonRpcRequest>(&request.body);
    let rpc_request = match parsed {
        Ok(request) => request,
        Err(error) => {
            let body = rpc_error(Value::Null, -32700, &format!("Parse error: {}", error));
            return json_response(400, body);
        }
    };

    if rpc_request.id.is_none() {
        let _ = dispatch_notification(&rpc_request.method);
        return http_response(202, "Accepted", &[], Vec::new());
    }

    let id = rpc_request.id.clone().unwrap_or(Value::Null);
    let result = match rpc_request.method.as_str() {
        "initialize" => Ok(initialize_result()),
        "tools/list" => Ok(tools_list_result()),
        "tools/call" => call_tool(rpc_request.params.unwrap_or_else(|| json!({})), runtime).await,
        _ => Err((-32601, format!("Method not found: {}", rpc_request.method))),
    };

    match result {
        Ok(result) => json_response(200, rpc_success(id, result)),
        Err((code, message)) => json_response(200, rpc_error(id, code, &message)),
    }
}

fn dispatch_notification(method: &str) -> Result<(), String> {
    match method {
        "notifications/initialized" => Ok(()),
        _ => Ok(()),
    }
}

async fn call_tool(params: Value, runtime: McpRuntime) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "tools/call requires params.name".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match name {
        "list_terminal_sessions" => list_terminal_sessions(&runtime).await,
        "read_terminal_output" => read_terminal_output(&runtime, arguments).await,
        "send_terminal_input" => send_terminal_input(&runtime, arguments).await,
        _ => Err((-32602, format!("Unknown tool: {}", name))),
    }
}

async fn list_terminal_sessions(runtime: &McpRuntime) -> Result<Value, (i64, String)> {
    let sessions = runtime.terminals.list_sessions().await;
    let text = serde_json::to_string_pretty(&sessions)
        .map_err(|error| (-32603, format!("Serialize sessions failed: {}", error)))?;

    Ok(tool_result(
        text,
        json!({
            "sessions": sessions,
        }),
        false,
    ))
}

async fn read_terminal_output(
    runtime: &McpRuntime,
    arguments: Value,
) -> Result<Value, (i64, String)> {
    let session_id = string_arg(&arguments, "session_id")?;
    let max_chars = arguments
        .get("max_chars")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_READ_CHARS)
        .clamp(1, MAX_READ_CHARS);

    let snapshot = runtime
        .terminals
        .read_output(session_id, max_chars)
        .await
        .map_err(|error| (-32602, error))?;
    let text = serde_json::to_string_pretty(&snapshot)
        .map_err(|error| (-32603, format!("Serialize output failed: {}", error)))?;

    Ok(tool_result(text, json!(snapshot), false))
}

async fn send_terminal_input(
    runtime: &McpRuntime,
    arguments: Value,
) -> Result<Value, (i64, String)> {
    let session_id = string_arg(&arguments, "session_id")?;
    let data = string_arg(&arguments, "data")?;
    let info = runtime
        .terminals
        .session_info(session_id)
        .await
        .ok_or_else(|| (-32602, "セッションが見つかりません".to_string()))?;

    if info.status != TerminalStatus::Connected {
        return Err((-32602, "セッションは切断済みです".to_string()));
    }

    match info.protocol {
        TerminalProtocol::Ssh => ssh::write_data(&runtime.ssh, session_id, data.to_string()).await,
        TerminalProtocol::Serial => {
            serial::write_data(&runtime.serial, session_id, data.to_string()).await
        }
        TerminalProtocol::Telnet => {
            telnet::write_data(&runtime.telnet, session_id, data.to_string()).await
        }
    }
    .map_err(|error| (-32603, error))?;

    Ok(tool_result(
        "input sent".to_string(),
        json!({
            "session_id": session_id,
            "sent": true,
        }),
        false,
    ))
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "tools": {
                "listChanged": true
            }
        },
        "serverInfo": {
            "name": "exaterm",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "list_terminal_sessions",
                "title": "List Terminal Sessions",
                "description": "List ExaTerm terminal sessions that were opened by the user.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            },
            {
                "name": "read_terminal_output",
                "title": "Read Terminal Output",
                "description": "Read recent output from an ExaTerm terminal session.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "Session ID returned by list_terminal_sessions."
                        },
                        "max_chars": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": MAX_READ_CHARS,
                            "description": "Maximum number of recent characters to return."
                        }
                    },
                    "required": ["session_id"],
                    "additionalProperties": false
                }
            },
            {
                "name": "send_terminal_input",
                "title": "Send Terminal Input",
                "description": "Send text to an existing connected ExaTerm terminal session.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "Session ID returned by list_terminal_sessions."
                        },
                        "data": {
                            "type": "string",
                            "description": "Text to send to the terminal. Include newline characters when needed."
                        }
                    },
                    "required": ["session_id", "data"],
                    "additionalProperties": false
                }
            }
        ]
    })
}

fn string_arg<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, (i64, String)> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, format!("Missing string argument: {}", name)))
}

fn tool_result(text: String, structured_content: Value, is_error: bool) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured_content,
        "isError": is_error
    })
}

fn rpc_success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn json_response(status: u16, body: Value) -> Vec<u8> {
    let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    http_response(
        status,
        status_reason(status),
        &[("Content-Type", "application/json")],
        body,
    )
}

fn http_response(status: u16, reason: &str, headers: &[(&str, &str)], body: Vec<u8>) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        status,
        reason,
        body.len()
    );
    for (name, value) in headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");

    let mut bytes = response.into_bytes();
    bytes.extend(body);
    bytes
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    }
}

fn origin_is_allowed(headers: &HashMap<String, String>) -> bool {
    let Some(origin) = headers.get("origin") else {
        return true;
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
    let host = authority.split(':').next().unwrap_or(authority);

    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_local_origins() {
        let mut headers = HashMap::new();
        headers.insert("origin".into(), "https://example.com".into());
        assert!(!origin_is_allowed(&headers));

        headers.insert("origin".into(), "http://127.0.0.1:8765".into());
        assert!(origin_is_allowed(&headers));

        headers.insert("origin".into(), "http://localhost.evil.test".into());
        assert!(!origin_is_allowed(&headers));
    }

    #[test]
    fn tools_list_exposes_terminal_tools() {
        let tools = tools_list_result();
        let names = tools["tools"]
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
    }
}
