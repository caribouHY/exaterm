use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::mcp::backend::{
    internal_error, structured_tool_result, ConnectSavedProfileArgs, ConnectSerialConsoleArgs,
    McpRuntime, ReadTerminalOutputArgs, ReadTerminalOutputDeltaArgs, RunTerminalCommandArgs,
    SendTerminalInputArgs, StartTerminalLogArgs, StopTerminalLogArgs, WaitTerminalOutputArgs,
};
use crate::mcp::control::McpControlService;

#[derive(Clone)]
pub(super) struct ExaTermMcpServer {
    control: McpControlService,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl ExaTermMcpServer {
    pub(super) fn new(runtime: McpRuntime) -> Self {
        Self::with_control(McpControlService::in_process(runtime))
    }

    pub(super) fn with_control(control: McpControlService) -> Self {
        Self {
            control,
            tool_router: Self::tool_router(),
        }
    }

    async fn call_tool(&self, name: &str, args: Value) -> Result<CallToolResult, McpError> {
        self.control
            .call_tool(name, args)
            .await
            .and_then(structured_tool_result)
    }

    async fn call_tool_with_args<T>(&self, name: &str, args: T) -> Result<CallToolResult, McpError>
    where
        T: Serialize,
    {
        let args = serde_json::to_value(args)
            .map_err(|error| internal_error(format!("Serialize MCP tool args failed: {error}")))?;
        self.call_tool(name, args).await
    }

    #[tool(
        name = "list_terminal_sessions",
        description = "List ExaTerm terminal sessions that were opened by the user."
    )]
    async fn list_terminal_sessions(&self) -> Result<CallToolResult, McpError> {
        self.call_tool("list_terminal_sessions", json!({})).await
    }

    #[tool(
        name = "list_connection_profiles",
        description = "List saved SSH and Telnet connection profiles when MCP profile connections are enabled. Secrets and private key paths are not returned."
    )]
    async fn list_connection_profiles(&self) -> Result<CallToolResult, McpError> {
        self.call_tool("list_connection_profiles", json!({})).await
    }

    #[tool(
        name = "connect_saved_profile",
        description = "Open a new ExaTerm SSH or Telnet session from a saved profile when MCP profile connections are enabled. SSH passwords and encrypted key passphrases are requested in the ExaTerm UI."
    )]
    async fn connect_saved_profile(
        &self,
        Parameters(args): Parameters<ConnectSavedProfileArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("connect_saved_profile", args)
            .await
    }

    #[tool(
        name = "list_serial_ports",
        description = "List available Serial console ports when MCP profile connections are enabled."
    )]
    async fn list_serial_ports(&self) -> Result<CallToolResult, McpError> {
        self.call_tool("list_serial_ports", json!({})).await
    }

    #[tool(
        name = "connect_serial_console",
        description = "Open a new ExaTerm Serial console session from explicit port and line settings when MCP profile connections are enabled."
    )]
    async fn connect_serial_console(
        &self,
        Parameters(args): Parameters<ConnectSerialConsoleArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("connect_serial_console", args)
            .await
    }

    #[tool(
        name = "read_terminal_output",
        description = "Read recent output from an ExaTerm terminal session."
    )]
    async fn read_terminal_output(
        &self,
        Parameters(args): Parameters<ReadTerminalOutputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("read_terminal_output", args).await
    }

    #[tool(
        name = "read_terminal_output_delta",
        description = "Read output written after a previously returned ExaTerm terminal cursor."
    )]
    async fn read_terminal_output_delta(
        &self,
        Parameters(args): Parameters<ReadTerminalOutputDeltaArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("read_terminal_output_delta", args)
            .await
    }

    #[tool(
        name = "wait_terminal_output",
        description = "Wait until an ExaTerm terminal session produces new output or a target string appears."
    )]
    async fn wait_terminal_output(
        &self,
        Parameters(args): Parameters<WaitTerminalOutputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("wait_terminal_output", args).await
    }

    #[tool(
        name = "send_terminal_input",
        description = "Send text to an existing connected ExaTerm terminal session."
    )]
    async fn send_terminal_input(
        &self,
        Parameters(args): Parameters<SendTerminalInputArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("send_terminal_input", args).await
    }

    #[tool(
        name = "start_terminal_log",
        description = "Start a manual plaintext log for an existing connected ExaTerm terminal session. The log is saved under ExaTerm's log directory and the file path is returned."
    )]
    async fn start_terminal_log(
        &self,
        Parameters(args): Parameters<StartTerminalLogArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("start_terminal_log", args).await
    }

    #[tool(
        name = "stop_terminal_log",
        description = "Stop a manual plaintext log for an existing ExaTerm terminal session after flushing pending displayed output."
    )]
    async fn stop_terminal_log(
        &self,
        Parameters(args): Parameters<StopTerminalLogArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("stop_terminal_log", args).await
    }

    #[tool(
        name = "run_terminal_command",
        description = "Send a command to an existing connected ExaTerm terminal session, wait for output, and return the output delta."
    )]
    async fn run_terminal_command(
        &self,
        Parameters(args): Parameters<RunTerminalCommandArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_with_args("run_terminal_command", args).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ExaTermMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("exaterm", env!("CARGO_PKG_VERSION")))
    }
}
