use rmcp::{transport, ServiceExt};

use crate::{
    config,
    mcp::{
        backend::{McpRuntime, ProxyMcpBackend},
        client::ControlClient,
        control::handle_control_connection,
        service::ExaTermMcpServer,
    },
};

pub fn spawn_gui_control_plane(runtime: McpRuntime) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_gui_control_plane(runtime).await {
            log::error!("MCP GUI control plane stopped: {error}");
        }
    });
}

pub async fn run_gui_control_plane(runtime: McpRuntime) -> Result<(), String> {
    run_local_control_server(crate::mcp::control::McpControlService::in_process(runtime)).await
}

pub async fn run_stdio_proxy() -> Result<(), String> {
    let config = config::config_read()?;
    if !config.mcp.enabled || !config.mcp.stdio_enabled {
        return Err(
            "MCP stdio transport is disabled. Set mcp.enabled=true and mcp.stdio_enabled=true."
                .into(),
        );
    }

    let client = ControlClient::new();
    client.discover_or_start_gui().await?;
    let control = crate::mcp::control::McpControlService::new(ProxyMcpBackend::new(client));
    run_stdio_server(control).await
}

pub(super) async fn run_stdio_server(
    control: crate::mcp::control::McpControlService,
) -> Result<(), String> {
    let server = ExaTermMcpServer::with_control(control);
    let running = server
        .serve(transport::stdio())
        .await
        .map_err(|error| format!("MCP stdio initialize error: {error:?}"))?;
    running
        .waiting()
        .await
        .map_err(|error| format!("MCP stdio service join error: {error}"))?;
    Ok(())
}

#[cfg(windows)]
async fn run_local_control_server(
    control: crate::mcp::control::McpControlService,
) -> Result<(), String> {
    use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};

    let pipe_name = crate::mcp::client::control_pipe_name();
    loop {
        let mut options = ServerOptions::new();
        options.pipe_mode(PipeMode::Byte).max_instances(16);
        let server = options
            .create(&pipe_name)
            .map_err(|error| format!("MCP control pipe create error: {error}"))?;
        if let Err(error) = server.connect().await {
            log::warn!("MCP control pipe connect error: {error}");
            continue;
        }
        let control = control.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = handle_control_connection(control, server).await {
                log::warn!("MCP control connection closed with error: {error}");
            }
        });
    }
}

#[cfg(not(windows))]
async fn run_local_control_server(
    control: crate::mcp::control::McpControlService,
) -> Result<(), String> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(crate::mcp::client::control_tcp_address())
        .await
        .map_err(|error| format!("MCP control TCP bind error: {error}"))?;
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|error| format!("MCP control TCP accept error: {error}"))?;
        let control = control.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = handle_control_connection(control, stream).await {
                log::warn!("MCP control connection closed with error: {error}");
            }
        });
    }
}
