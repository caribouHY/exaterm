use rmcp::{transport, ServiceExt};

use crate::{
    config, external_control::client::ExternalControlClient, mcp::service::ExaTermMcpServer,
};

pub async fn run_stdio_proxy() -> Result<(), String> {
    let config = config::config_read()?;
    if !config.external_control.enabled || !config.external_control.mcp_enabled {
        return Err(
            "MCP compatibility adapter is disabled. Set external_control.enabled=true and external_control.mcp_enabled=true."
                .into(),
        );
    }

    let client = ExternalControlClient::new();
    client.discover_or_start_gui().await?;
    run_stdio_server(client).await
}

pub(super) async fn run_stdio_server(client: ExternalControlClient) -> Result<(), String> {
    let server = ExaTermMcpServer::with_client(client);
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
