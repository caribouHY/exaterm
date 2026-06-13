use rmcp::{transport, ServiceExt};

use crate::{
    config,
    external_control::client::ExternalControlClient,
    mcp::{backend::ProxyMcpBackend, service::ExaTermMcpServer},
};

pub async fn run_stdio_proxy() -> Result<(), String> {
    let config = config::config_read()?;
    if !config.mcp.enabled || !config.mcp.stdio_enabled {
        return Err(
            "MCP stdio transport is disabled. Set mcp.enabled=true and mcp.stdio_enabled=true."
                .into(),
        );
    }

    let client = ExternalControlClient::new();
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
