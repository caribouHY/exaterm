use crate::external_control::{
    protocol::handle_control_connection_with_permit,
    service::{ExternalControlRuntime, ExternalControlService},
};

#[cfg(windows)]
const PIPE_CREATE_RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

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

    let pending = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    let pipe_name = crate::external_control::client::control_pipe_name();
    loop {
        let mut options = ServerOptions::new();
        options.pipe_mode(PipeMode::Byte).max_instances(16);
        let server = match options.create(&pipe_name) {
            Ok(server) => server,
            Err(error) => {
                log::warn!("External control pipe create error; retrying: {error}");
                tokio::time::sleep(PIPE_CREATE_RETRY_INTERVAL).await;
                continue;
            }
        };
        if let Err(error) = server.connect().await {
            log::warn!("External control pipe connect error: {error}");
            continue;
        }
        if super::transport::windows::verify_peer(&server, true).is_err() {
            continue;
        }
        let Ok(permit) = pending.clone().try_acquire_owned() else {
            continue;
        };
        let service = service.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) =
                handle_control_connection_with_permit(service, server, Some(permit)).await
            {
                log::warn!("External control connection closed with error: {error}");
            }
        });
    }
}

#[cfg(unix)]
async fn run_local_control_server(service: ExternalControlService) -> Result<(), String> {
    let (listener, _listener_lock) =
        super::transport::unix::bind_at(&super::transport::unix::control_directory()).await?;
    let pending = std::sync::Arc::new(tokio::sync::Semaphore::new(8));
    loop {
        let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        if super::transport::unix::verify_peer(&stream).is_err() {
            continue;
        }
        let Ok(permit) = pending.clone().try_acquire_owned() else {
            continue;
        };
        let service = service.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) =
                handle_control_connection_with_permit(service, stream, Some(permit)).await
            {
                log::warn!("External control connection closed with error: {error}");
            }
        });
    }
}
