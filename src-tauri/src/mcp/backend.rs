use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::external_control::{
    client::ExternalControlClient, ExternalControlError, ExternalControlRequest,
    ExternalControlResponse, ExternalControlRuntime, ExternalControlService,
};

#[async_trait]
pub trait McpBackend: Send + Sync {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError>;
}

#[derive(Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct InProcessMcpBackend {
    service: ExternalControlService,
}

impl InProcessMcpBackend {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(runtime: ExternalControlRuntime) -> Self {
        Self {
            service: ExternalControlService::new(runtime),
        }
    }
}

#[async_trait]
impl McpBackend for InProcessMcpBackend {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        let request = request_from_tool(name, args)?;
        self.service
            .execute(request)
            .await
            .map(ExternalControlResponse::into_value)
            .map_err(external_error_to_mcp)
    }
}

#[derive(Clone)]
pub(super) struct ProxyMcpBackend {
    client: ExternalControlClient,
}

#[derive(Clone, Default)]
pub(crate) struct ControlClient {
    client: ExternalControlClient,
}

impl ControlClient {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn discover_or_start_gui(&self) -> Result<(), String> {
        self.client.discover_or_start_gui().await
    }

    pub(crate) async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        let request = request_from_tool(name, args)?;
        self.client
            .call(request)
            .await
            .map(ExternalControlResponse::into_value)
            .map_err(external_error_to_mcp)
    }
}

impl ProxyMcpBackend {
    pub(super) fn new(client: ExternalControlClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl McpBackend for ProxyMcpBackend {
    async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        let request = request_from_tool(name, args)?;
        self.client
            .call(request)
            .await
            .map(ExternalControlResponse::into_value)
            .map_err(external_error_to_mcp)
    }
}

fn request_from_tool(name: &str, args: Value) -> Result<ExternalControlRequest, McpError> {
    match name {
        "list_terminal_sessions" => Ok(ExternalControlRequest::ListTerminalSessions),
        "list_connection_profiles" => Ok(ExternalControlRequest::ListConnectionProfiles(
            parse_tool_args(args)?,
        )),
        "connect_saved_profile" => Ok(ExternalControlRequest::ConnectSavedProfile(
            parse_tool_args(args)?,
        )),
        "list_serial_ports" => Ok(ExternalControlRequest::ListSerialPorts),
        "connect_serial_console" => Ok(ExternalControlRequest::ConnectSerialConsole(
            parse_tool_args(args)?,
        )),
        "read_terminal_output" => Ok(ExternalControlRequest::ReadTerminalOutput(parse_tool_args(
            args,
        )?)),
        "send_terminal_input" => Ok(ExternalControlRequest::SendTerminalInput(parse_tool_args(
            args,
        )?)),
        "start_terminal_log" => Ok(ExternalControlRequest::StartTerminalLog(parse_tool_args(
            args,
        )?)),
        "stop_terminal_log" => Ok(ExternalControlRequest::StopTerminalLog(parse_tool_args(
            args,
        )?)),
        "run_terminal_command" => Ok(ExternalControlRequest::RunTerminalCommand(parse_tool_args(
            args,
        )?)),
        _ => Err(McpError::invalid_params(
            format!("Unknown MCP tool: {name}"),
            None,
        )),
    }
}

fn parse_tool_args<T>(args: Value) -> Result<T, McpError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(args).map_err(|error| {
        McpError::invalid_params(format!("MCP tool argument parse failed: {error}"), None)
    })
}

pub(super) fn external_error_to_mcp(error: ExternalControlError) -> McpError {
    match error {
        ExternalControlError::InvalidArguments(message)
        | ExternalControlError::NotFound(message) => McpError::invalid_params(message, None),
        ExternalControlError::PermissionDenied(message)
        | ExternalControlError::Unavailable(message)
        | ExternalControlError::Internal(message) => McpError::internal_error(message, None),
    }
}
