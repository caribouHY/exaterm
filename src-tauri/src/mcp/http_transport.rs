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
use tokio::net::TcpListener;

use crate::mcp::backend::McpRuntime;
use crate::mcp::service::ExaTermMcpServer;

pub(super) type ExaTermMcpHttpService =
    StreamableHttpService<ExaTermMcpServer, LocalSessionManager>;

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

pub(super) fn mcp_server_config(configured_host: &str) -> StreamableHttpServerConfig {
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

pub(super) async fn mcp_http_handler(
    State(service): State<ExaTermMcpHttpService>,
    request: Request<Body>,
) -> Response {
    if !origin_is_allowed(request.headers()) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    service.handle(request).await.map(Body::new)
}

pub(super) fn origin_is_allowed(headers: &HeaderMap) -> bool {
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
