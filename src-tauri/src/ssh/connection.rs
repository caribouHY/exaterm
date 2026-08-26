use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use russh::Disconnect;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::config::config_load;
use crate::connect_attempt::{run_with_attempt, ConnectAttempt};
use crate::logger::LoggerState;
use crate::ssh::auth::{authenticate_ssh, build_auth_request};
use crate::ssh::authentication_prompt::SshAuthenticationPrompter;
use crate::ssh::client_config::build_client_config;
use crate::ssh::diagnostics::{map_connect_error, SshDiagnostic};
use crate::ssh::host_key::{HostKeyHandling, HostKeyVerifier, SshHostKeyHandler};
use crate::ssh::host_key_prompt::{SshHostKeyPrompter, HOST_KEY_PROMPT_TIMEOUT};
use crate::ssh::io::{
    run_ssh_operation_with_timeout, spawn_ssh_read_processor, SshClientHandler, SshReadDropState,
    SshReadRequest, SshSession, SshState, SSH_AUTH_TIMEOUT_ERROR, SSH_CHANNEL_OPEN_TIMEOUT,
    SSH_CHANNEL_OPEN_TIMEOUT_ERROR, SSH_CONNECT_TIMEOUT, SSH_CONNECT_TIMEOUT_ERROR,
    SSH_PTY_TIMEOUT, SSH_PTY_TIMEOUT_ERROR, SSH_READ_QUEUE_CAPACITY, SSH_SHELL_TIMEOUT,
    SSH_SHELL_TIMEOUT_ERROR,
};
use crate::ssh::jump::connect_jump_profile;
use crate::ssh::profiles::resolve_jump_profile;
use crate::ssh::types::{SshAuthRequest, SshConnectOptions, SshConnectResult, SshJumpProfile};
use crate::terminal_control::{TerminalControlState, TerminalProtocol};
use crate::workspace::WorkspaceState;

const SSH_CONNECT_CANCELLED: &str = "The SSH connection attempt was cancelled";

type TargetHandle = russh::client::Handle<SshClientHandler>;
type JumpHandle = russh::client::Handle<SshHostKeyHandler>;
type TargetSessionChannel = russh::Channel<russh::client::Msg>;

struct ConnectPreparation {
    session_id: String,
    diagnostic: SshDiagnostic,
    auth: SshAuthRequest,
    jump_profile: Option<SshJumpProfile>,
    config: Arc<russh::client::Config>,
    host_verifier: HostKeyVerifier,
    handler: SshClientHandler,
    read_rx: mpsc::Receiver<SshReadRequest>,
}

struct ConnectCompletion {
    session_id: String,
    diagnostic: SshDiagnostic,
    read_rx: mpsc::Receiver<SshReadRequest>,
}

pub async fn connect(
    app: &AppHandle,
    state: &SshState,
    terminals: &TerminalControlState,
    workspace: &WorkspaceState,
    logger_state: Option<&LoggerState>,
    prompt_window_id: String,
    host_key_handling: HostKeyHandling,
    options: SshConnectOptions,
    mut attempt: Option<ConnectAttempt>,
) -> Result<SshConnectResult, String> {
    let authentication_prompter = SshAuthenticationPrompter::new(
        app,
        state.authentication_prompts.clone(),
        prompt_window_id.clone(),
        options.request_id.clone(),
    );
    let host_key_prompter = (host_key_handling == HostKeyHandling::Prompt).then(|| {
        SshHostKeyPrompter::new(
            app,
            state.host_key_prompts.clone(),
            prompt_window_id.clone(),
            options.request_id.clone(),
        )
    });
    let connect_timeout = if host_key_prompter.is_some() {
        SSH_CONNECT_TIMEOUT + HOST_KEY_PROMPT_TIMEOUT
    } else {
        SSH_CONNECT_TIMEOUT
    };
    let prepared = prepare_connect(
        app,
        state,
        terminals,
        workspace,
        logger_state,
        &prompt_window_id,
        &options,
        host_key_prompter.clone(),
    )?;
    let ConnectPreparation {
        session_id,
        diagnostic,
        auth,
        jump_profile,
        config,
        host_verifier,
        handler,
        read_rx,
    } = prepared;
    let (mut handle, jump_handle) = connect_target_handle(
        config,
        handler,
        jump_profile,
        &options,
        &host_verifier,
        &diagnostic,
        &authentication_prompter,
        host_key_prompter.as_ref(),
        connect_timeout,
        attempt.as_ref(),
    )
    .await?;
    let channel = match run_with_attempt(
        attempt.as_ref(),
        Box::pin(establish_target_shell(
            &mut handle,
            &jump_handle,
            auth,
            &options,
            &diagnostic,
            &authentication_prompter,
        )),
    )
    .await
    {
        Ok(channel) => channel,
        Err(error) => {
            if error == SSH_CONNECT_CANCELLED {
                disconnect_target_handles(&handle, &jump_handle, "Connection cancelled").await;
            }
            return Err(error);
        }
    };
    if attempt
        .as_mut()
        .is_some_and(|attempt| !attempt.begin_completion())
    {
        disconnect_target_handles(&handle, &jump_handle, "Connection cancelled").await;
        return Err(SSH_CONNECT_CANCELLED.to_string());
    }
    let completion = ConnectCompletion {
        session_id,
        diagnostic,
        read_rx,
    };

    finish_connected_session(
        app,
        state,
        terminals,
        completion,
        handle,
        jump_handle,
        &options,
        channel,
    )
    .await
}

async fn finish_connected_session(
    app: &AppHandle,
    state: &SshState,
    terminals: &TerminalControlState,
    completion: ConnectCompletion,
    handle: TargetHandle,
    jump_handle: Option<JumpHandle>,
    options: &SshConnectOptions,
    channel: TargetSessionChannel,
) -> Result<SshConnectResult, String> {
    let (mut channel_read_half, channel_write_half) = channel.split();
    tokio::spawn(async move { while channel_read_half.wait().await.is_some() {} });
    spawn_ssh_read_processor(
        app,
        &completion.session_id,
        terminals.clone(),
        completion.read_rx,
    );
    register_connected_session(
        state,
        terminals,
        &completion.session_id,
        handle,
        channel_write_half,
        jump_handle,
        options,
    )
    .await;
    let _ = app.emit("ssh://connected", &completion.session_id);
    completion.diagnostic.info("target: session ready");
    Ok(SshConnectResult {
        session_id: completion.session_id,
    })
}

async fn establish_target_shell(
    handle: &mut TargetHandle,
    jump_handle: &Option<JumpHandle>,
    auth: SshAuthRequest,
    options: &SshConnectOptions,
    diagnostic: &SshDiagnostic,
    prompter: &SshAuthenticationPrompter,
) -> Result<TargetSessionChannel, String> {
    let auth_context = prompter.context("target", &options.host, options.port, &options.username);
    authenticate_target(
        handle,
        jump_handle,
        &options.username,
        auth,
        diagnostic,
        &auth_context,
    )
    .await?;
    let channel = open_target_session_channel(handle, jump_handle, diagnostic).await?;
    request_target_pty(
        handle,
        jump_handle,
        &channel,
        options.cols,
        options.rows,
        diagnostic,
    )
    .await?;
    request_target_shell(handle, jump_handle, &channel, diagnostic).await?;
    Ok(channel)
}

fn prepare_connect(
    app: &AppHandle,
    state: &SshState,
    terminals: &TerminalControlState,
    workspace: &WorkspaceState,
    logger_state: Option<&LoggerState>,
    prompt_window_id: &str,
    options: &SshConnectOptions,
    host_key_prompter: Option<SshHostKeyPrompter>,
) -> Result<ConnectPreparation, String> {
    let session_id = Uuid::new_v4().to_string();
    let diagnostic = SshDiagnostic::new(
        app,
        options.request_id.clone(),
        prompt_window_id.to_string(),
    );
    let app_config = config_load().map_err(|error| error.message)?;
    let auth = build_auth_request(
        options.auth_method.clone(),
        options.password.clone(),
        options.private_key_path.clone(),
        options.key_passphrase.clone(),
        Some(app_config.ssh.default_private_key_path.clone()),
    )?;
    let jump_profile = resolve_jump_profile(&app_config, options.jump_profile_id.as_deref(), None)?;
    let config = Arc::new(build_client_config(&app_config.ssh)?);
    let host_verifier = HostKeyVerifier::new(options.host.clone(), options.port);
    let (read_tx, read_rx) = mpsc::channel::<SshReadRequest>(SSH_READ_QUEUE_CAPACITY);
    let handler = SshClientHandler {
        app: app.clone(),
        session_id: session_id.clone(),
        sessions: state.sessions.clone(),
        host_verifier: host_verifier.clone(),
        host_key_prompter,
        diagnostic: diagnostic.clone(),
        terminals: terminals.clone(),
        workspace: workspace.clone(),
        logger: logger_state.cloned(),
        read_tx,
        read_drop_state: Arc::new(StdMutex::new(SshReadDropState::default())),
    };
    Ok(ConnectPreparation {
        session_id,
        diagnostic,
        auth,
        jump_profile,
        config,
        host_verifier,
        handler,
        read_rx,
    })
}

async fn connect_target_handle(
    config: Arc<russh::client::Config>,
    handler: SshClientHandler,
    jump_profile: Option<SshJumpProfile>,
    options: &SshConnectOptions,
    host_verifier: &HostKeyVerifier,
    diagnostic: &SshDiagnostic,
    authentication_prompter: &SshAuthenticationPrompter,
    host_key_prompter: Option<&SshHostKeyPrompter>,
    connect_timeout: Duration,
    attempt: Option<&ConnectAttempt>,
) -> Result<(TargetHandle, Option<JumpHandle>), String> {
    match jump_profile {
        Some(jump_profile) => {
            connect_target_via_jump(
                config,
                handler,
                jump_profile,
                options,
                host_verifier,
                diagnostic,
                authentication_prompter,
                host_key_prompter,
                connect_timeout,
                attempt,
            )
            .await
        }
        None => {
            connect_target_direct(
                config,
                handler,
                options,
                host_verifier,
                diagnostic,
                connect_timeout,
                attempt,
            )
            .await
        }
    }
}

async fn connect_target_via_jump(
    config: Arc<russh::client::Config>,
    handler: SshClientHandler,
    jump_profile: SshJumpProfile,
    options: &SshConnectOptions,
    host_verifier: &HostKeyVerifier,
    diagnostic: &SshDiagnostic,
    authentication_prompter: &SshAuthenticationPrompter,
    host_key_prompter: Option<&SshHostKeyPrompter>,
    connect_timeout: Duration,
    attempt: Option<&ConnectAttempt>,
) -> Result<(TargetHandle, Option<JumpHandle>), String> {
    let (jump_handle, jump_channel) = connect_jump_profile(
        config.clone(),
        jump_profile,
        &options.host,
        options.port,
        options.jump_password.clone(),
        options.jump_key_passphrase.clone(),
        Some(diagnostic),
        authentication_prompter,
        host_key_prompter,
        connect_timeout,
        attempt,
    )
    .await?;
    let stream = jump_channel.into_stream();
    diagnostic.progress("target", "connecting");
    diagnostic.info("target: starting SSH handshake");
    let handle = run_with_attempt(
        attempt,
        Box::pin(run_ssh_operation_with_timeout(
            connect_timeout,
            SSH_CONNECT_TIMEOUT_ERROR,
            async {
                russh::client::connect_stream(config, stream, handler)
                    .await
                    .map_err(|error| map_connect_error(error, host_verifier))
            },
        )),
    )
    .await;
    match handle {
        Ok(handle) => Ok((handle, Some(jump_handle))),
        Err(error) => {
            if error != SSH_CONNECT_CANCELLED {
                emit_target_timeout_or_failure(
                    diagnostic,
                    &error,
                    "SSH handshake",
                    "SSH handshake",
                );
            }
            let _ = jump_handle
                .disconnect(Disconnect::ByApplication, "Target handshake failed", "en")
                .await;
            Err(error)
        }
    }
}

async fn connect_target_direct(
    config: Arc<russh::client::Config>,
    handler: SshClientHandler,
    options: &SshConnectOptions,
    host_verifier: &HostKeyVerifier,
    diagnostic: &SshDiagnostic,
    connect_timeout: Duration,
    attempt: Option<&ConnectAttempt>,
) -> Result<(TargetHandle, Option<JumpHandle>), String> {
    diagnostic.progress("target", "connecting");
    diagnostic.info("target: starting SSH handshake");
    let handle = run_with_attempt(
        attempt,
        Box::pin(run_ssh_operation_with_timeout(
            connect_timeout,
            SSH_CONNECT_TIMEOUT_ERROR,
            async {
                russh::client::connect(config, (options.host.as_str(), options.port), handler)
                    .await
                    .map_err(|error| map_connect_error(error, host_verifier))
            },
        )),
    )
    .await
    .map_err(|error| {
        if error != SSH_CONNECT_CANCELLED {
            emit_target_timeout_or_failure(diagnostic, &error, "SSH handshake", "SSH handshake");
        }
        error
    })?;
    Ok((handle, None))
}

fn emit_target_timeout_or_failure(
    diagnostic: &SshDiagnostic,
    error: &str,
    timeout_label: &str,
    failure_label: &str,
) {
    if error == SSH_CONNECT_TIMEOUT_ERROR
        || error == SSH_AUTH_TIMEOUT_ERROR
        || error == SSH_CHANNEL_OPEN_TIMEOUT_ERROR
        || error == SSH_PTY_TIMEOUT_ERROR
        || error == SSH_SHELL_TIMEOUT_ERROR
    {
        diagnostic.error(format!("error: target {timeout_label} timed out"));
    } else {
        diagnostic.error(format!("error: target {failure_label} failed"));
    }
}

async fn disconnect_target_handles(
    handle: &TargetHandle,
    jump_handle: &Option<JumpHandle>,
    reason: &'static str,
) {
    let _ = handle
        .disconnect(Disconnect::ByApplication, reason, "en")
        .await;
    if let Some(jump_handle) = jump_handle {
        let _ = jump_handle
            .disconnect(Disconnect::ByApplication, reason, "en")
            .await;
    }
}

async fn authenticate_target(
    handle: &mut TargetHandle,
    jump_handle: &Option<JumpHandle>,
    username: &str,
    auth: SshAuthRequest,
    diagnostic: &SshDiagnostic,
    context: &crate::ssh::authentication_prompt::SshAuthenticationContext<'_>,
) -> Result<(), String> {
    diagnostic.progress("target", "authenticating");
    diagnostic.info("target: authentication started");
    let result = authenticate_ssh(handle, username, auth, context, Some(diagnostic)).await;
    match result {
        Ok(()) => {
            diagnostic.info("target: authentication succeeded");
            Ok(())
        }
        Err(error) => {
            emit_target_timeout_or_failure(diagnostic, &error, "authentication", "authentication");
            disconnect_target_handles(handle, jump_handle, "Target authentication failed").await;
            Err(error)
        }
    }
}

async fn open_target_session_channel(
    handle: &mut TargetHandle,
    jump_handle: &Option<JumpHandle>,
    diagnostic: &SshDiagnostic,
) -> Result<TargetSessionChannel, String> {
    diagnostic.progress("target", "opening_session");
    diagnostic.info("target: opening session channel");
    let result = run_ssh_operation_with_timeout(
        SSH_CHANNEL_OPEN_TIMEOUT,
        SSH_CHANNEL_OPEN_TIMEOUT_ERROR,
        async {
            handle
                .channel_open_session()
                .await
                .map_err(|e| format!("Failed to open the SSH channel: {}", e))
        },
    )
    .await;
    match result {
        Ok(channel) => Ok(channel),
        Err(error) => {
            emit_target_timeout_or_failure(
                diagnostic,
                &error,
                "session channel",
                "session channel",
            );
            disconnect_target_handles(handle, jump_handle, "Target channel open failed").await;
            Err(error)
        }
    }
}

async fn request_target_pty(
    handle: &TargetHandle,
    jump_handle: &Option<JumpHandle>,
    channel: &TargetSessionChannel,
    cols: u32,
    rows: u32,
    diagnostic: &SshDiagnostic,
) -> Result<(), String> {
    diagnostic.info("target: requesting pty");
    let result = run_ssh_operation_with_timeout(SSH_PTY_TIMEOUT, SSH_PTY_TIMEOUT_ERROR, async {
        channel
            .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
            .await
            .map_err(|_| "PTY request failed".to_string())
    })
    .await;
    if let Err(error) = result {
        emit_target_timeout_or_failure(diagnostic, &error, "pty request", "pty request");
        disconnect_target_handles(handle, jump_handle, "Target pty request failed").await;
        return Err(error);
    }
    Ok(())
}

async fn request_target_shell(
    handle: &TargetHandle,
    jump_handle: &Option<JumpHandle>,
    channel: &TargetSessionChannel,
    diagnostic: &SshDiagnostic,
) -> Result<(), String> {
    diagnostic.info("target: requesting shell");
    let result =
        run_ssh_operation_with_timeout(SSH_SHELL_TIMEOUT, SSH_SHELL_TIMEOUT_ERROR, async {
            channel
                .request_shell(false)
                .await
                .map_err(|_| "Shell request failed".to_string())
        })
        .await;
    if let Err(error) = result {
        emit_target_timeout_or_failure(diagnostic, &error, "shell request", "shell request");
        disconnect_target_handles(handle, jump_handle, "Target shell request failed").await;
        return Err(error);
    }
    Ok(())
}

async fn register_connected_session(
    state: &SshState,
    terminals: &TerminalControlState,
    session_id: &str,
    handle: TargetHandle,
    channel_write_half: russh::ChannelWriteHalf<russh::client::Msg>,
    jump_handle: Option<JumpHandle>,
    options: &SshConnectOptions,
) {
    let session = SshSession {
        handle,
        channel: Arc::new(Mutex::new(channel_write_half)),
        jump_handle,
    };
    state
        .sessions
        .lock()
        .await
        .insert(session_id.to_string(), Arc::new(Mutex::new(session)));
    terminals
        .register_session_with_encoding(
            session_id.to_string(),
            TerminalProtocol::Ssh,
            format!("{}:{}", options.host, options.port),
            options.encoding.clone(),
        )
        .await;
}
