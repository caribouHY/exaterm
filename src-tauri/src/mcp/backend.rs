use rmcp::ErrorData as McpError;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::external_control::{
    client::ExternalControlClient, ExternalControlError, ExternalControlRequest,
    ExternalControlResponse, ExternalControlService,
};

#[derive(Clone)]
pub(super) enum McpTarget {
    #[cfg_attr(not(test), allow(dead_code))]
    Service {
        service: ExternalControlService,
    },
    Client {
        client: ExternalControlClient,
    },
}

impl McpTarget {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn with_service(service: ExternalControlService) -> Self {
        Self::Service { service }
    }

    pub(super) fn with_client(client: ExternalControlClient) -> Self {
        Self::Client { client }
    }

    pub(super) async fn call_tool(&self, name: &str, args: Value) -> Result<Value, McpError> {
        let request = request_from_tool(name, args)?;
        let response = match self {
            Self::Service { service } => service.execute(request).await,
            Self::Client { client } => client.call(request).await,
        };
        response
            .map(ExternalControlResponse::into_value)
            .map_err(external_error_to_mcp)
    }
}

pub(super) fn request_from_tool(
    name: &str,
    args: Value,
) -> Result<ExternalControlRequest, McpError> {
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
