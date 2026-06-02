use std::{
    fs::{self, File, OpenOptions},
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

use rmcp::{transport, ServiceExt};
use serde_json::Value;
use tokio::time;

use crate::{
    config,
    mcp::{
        backend::{McpRuntime, ProxyMcpBackend},
        control::{
            control_call_over_stream, control_probe_over_stream, handle_control_connection,
            CONTROL_UNAVAILABLE_MESSAGE,
        },
        service::ExaTermMcpServer,
    },
};

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const POST_LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default)]
pub(super) struct McpControlClient;

impl McpControlClient {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Result<Value, rmcp::ErrorData> {
        call_local_control_tool(tool_name, args).await
    }

    async fn probe(&self) -> Result<(), String> {
        probe_local_control_plane().await
    }
}

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

    let client = McpControlClient::new();
    discover_or_start_gui(&client).await?;
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

async fn discover_or_start_gui(client: &McpControlClient) -> Result<(), String> {
    if wait_for_control_plane(client, DISCOVERY_TIMEOUT)
        .await
        .is_ok()
    {
        return Ok(());
    }

    let launch_lock = LaunchLock::acquire()?;
    if launch_lock.is_some() {
        start_gui_process()?;
    }

    wait_for_control_plane(client, POST_LAUNCH_TIMEOUT)
        .await
        .map_err(|_| CONTROL_UNAVAILABLE_MESSAGE.to_string())
}

async fn wait_for_control_plane(
    client: &McpControlClient,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = time::Instant::now() + timeout;
    loop {
        if client.probe().await.is_ok() {
            return Ok(());
        }
        if time::Instant::now() >= deadline {
            return Err(CONTROL_UNAVAILABLE_MESSAGE.into());
        }
        time::sleep(RETRY_INTERVAL).await;
    }
}

fn start_gui_process() -> Result<(), String> {
    let gui_path = resolve_gui_exe_path()?;
    Command::new(&gui_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            format!(
                "ExaTerm GUI launch failed at {}: {error}",
                gui_path.display()
            )
        })?;
    Ok(())
}

fn resolve_gui_exe_path() -> Result<PathBuf, String> {
    let current_exe =
        std::env::current_exe().map_err(|error| format!("Cannot resolve proxy path: {error}"))?;
    let current_dir = current_exe
        .parent()
        .ok_or_else(|| "Cannot resolve proxy directory".to_string())?;

    let candidate_names = gui_executable_names();
    for dir in current_dir.ancestors().take(4) {
        for name in &candidate_names {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err("Cannot find ExaTerm GUI executable near exaterm-mcp".into())
}

#[cfg(windows)]
fn gui_executable_names() -> Vec<&'static str> {
    vec!["exaterm.exe", "ExaTerm.exe"]
}

#[cfg(not(windows))]
fn gui_executable_names() -> Vec<&'static str> {
    vec!["exaterm", "ExaTerm"]
}

struct LaunchLock {
    path: PathBuf,
    _file: File,
}

impl LaunchLock {
    fn acquire() -> Result<Option<Self>, String> {
        let path = launch_lock_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Cannot create ExaTerm data directory: {error}"))?;
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => Ok(Some(Self { path, _file: file })),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
            Err(error) => Err(format!("Cannot create MCP launch lock: {error}")),
        }
    }
}

impl Drop for LaunchLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn launch_lock_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join("exaterm-mcp-launch.lock")
}

#[cfg(windows)]
async fn run_local_control_server(
    control: crate::mcp::control::McpControlService,
) -> Result<(), String> {
    use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};

    let pipe_name = control_pipe_name();
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

    let listener = TcpListener::bind(control_tcp_address())
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

#[cfg(windows)]
async fn call_local_control_tool(tool_name: &str, args: Value) -> Result<Value, rmcp::ErrorData> {
    let stream = connect_named_pipe().await?;
    control_call_over_stream(stream, tool_name, args).await
}

#[cfg(not(windows))]
async fn call_local_control_tool(tool_name: &str, args: Value) -> Result<Value, rmcp::ErrorData> {
    let stream = tokio::net::TcpStream::connect(control_tcp_address())
        .await
        .map_err(|_| rmcp::ErrorData::internal_error(CONTROL_UNAVAILABLE_MESSAGE, None))?;
    control_call_over_stream(stream, tool_name, args).await
}

#[cfg(windows)]
async fn probe_local_control_plane() -> Result<(), String> {
    let stream = connect_named_pipe()
        .await
        .map_err(|error| error.message.into_owned())?;
    control_probe_over_stream(stream).await
}

#[cfg(not(windows))]
async fn probe_local_control_plane() -> Result<(), String> {
    let stream = tokio::net::TcpStream::connect(control_tcp_address())
        .await
        .map_err(|error| format!("MCP control TCP connect error: {error}"))?;
    control_probe_over_stream(stream).await
}

#[cfg(windows)]
async fn connect_named_pipe(
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient, rmcp::ErrorData> {
    use tokio::net::windows::named_pipe::ClientOptions;

    ClientOptions::new()
        .open(control_pipe_name())
        .map_err(|_| rmcp::ErrorData::internal_error(CONTROL_UNAVAILABLE_MESSAGE, None))
}

#[cfg(windows)]
fn control_pipe_name() -> String {
    format!(
        r"\\.\pipe\exaterm-mcp-control-{}",
        current_user_scope_hash()
    )
}

#[cfg(not(windows))]
fn control_tcp_address() -> String {
    let port = 39_500 + (current_user_scope_hash_value() % 1_000) as u16;
    format!("127.0.0.1:{port}")
}

fn current_user_scope_hash() -> String {
    format!("{:016x}", current_user_scope_hash_value())
}

fn current_user_scope_hash_value() -> u64 {
    let user = std::env::var("USERDOMAIN")
        .ok()
        .zip(std::env::var("USERNAME").ok())
        .map(|(domain, user)| format!("{domain}\\{user}"))
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "unknown-user".into())
        .to_ascii_lowercase();
    fnv1a_64(user.as_bytes())
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
