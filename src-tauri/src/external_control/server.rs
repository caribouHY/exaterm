use crate::external_control::{
    protocol::handle_control_connection,
    service::{ExternalControlRuntime, ExternalControlService},
};

pub fn spawn_gui_control_plane(runtime: ExternalControlRuntime) {
    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_gui_control_plane(runtime).await {
            log::error!("External control GUI service stopped: {error}");
        }
    });
}

async fn run_gui_control_plane(runtime: ExternalControlRuntime) -> Result<(), String> {
    run_local_control_server(ExternalControlService::new(runtime)).await
}

#[cfg(windows)]
async fn run_local_control_server(service: ExternalControlService) -> Result<(), String> {
    use tokio::net::windows::named_pipe::{PipeMode, ServerOptions};

    let pipe_name = crate::external_control::client::control_pipe_name();
    loop {
        let mut options = ServerOptions::new();
        options.pipe_mode(PipeMode::Byte).max_instances(16);
        let server = options
            .create(&pipe_name)
            .map_err(|error| format!("External control pipe create error: {error}"))?;
        if let Err(error) = server.connect().await {
            log::warn!("External control pipe connect error: {error}");
            continue;
        }
        let service = service.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = handle_control_connection(service, server).await {
                log::warn!("External control connection closed with error: {error}");
            }
        });
    }
}

#[cfg(not(windows))]
async fn run_local_control_server(service: ExternalControlService) -> Result<(), String> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(crate::external_control::client::control_tcp_address())
        .await
        .map_err(|error| format!("External control TCP bind error: {error}"))?;
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|error| format!("External control TCP accept error: {error}"))?;
        let service = service.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = handle_control_connection(service, stream).await {
                log::warn!("External control connection closed with error: {error}");
            }
        });
    }
}
