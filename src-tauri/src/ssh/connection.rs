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
    SshReadRequest, SshSession, SshState, SSH_AUTH_TIMEOUT_ERROR, SSH_CHANNEL_OPEN_TIMEOUT_ERROR,
    SSH_CONNECT_TIMEOUT, SSH_CONNECT_TIMEOUT_ERROR, SSH_PTY_TIMEOUT_ERROR, SSH_READ_QUEUE_CAPACITY,
    SSH_SHELL_TIMEOUT_ERROR,
};
use crate::ssh::jump::{connect_jump_profile, JumpAttemptContext, JumpConnectInputs};
use crate::ssh::profiles::resolve_jump_profile;
use crate::ssh::shell::{cleanup_failed_connection, start_session};
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

pub(crate) struct SshConnectRuntime<'a> {
    pub app: &'a AppHandle,
    pub state: &'a SshState,
    pub terminals: &'a TerminalControlState,
    pub workspace: &'a WorkspaceState,
    pub logger: Option<&'a LoggerState>,
}

pub(crate) struct SshConnectRequest {
    pub prompt_window_id: String,
    pub host_key_handling: HostKeyHandling,
    pub options: SshConnectOptions,
    pub attempt: Option<ConnectAttempt>,
}

struct ConnectedTarget {
    handle: TargetHandle,
    jump_handle: Option<JumpHandle>,
    channel: TargetSessionChannel,
}

struct TargetConnectInputs {
    config: Arc<russh::client::Config>,
    handler: SshClientHandler,
    jump_profile: Option<SshJumpProfile>,
}

struct TargetAttemptContext<'a> {
    options: &'a SshConnectOptions,
    host_verifier: &'a HostKeyVerifier,
    diagnostic: &'a SshDiagnostic,
    authentication_prompter: &'a SshAuthenticationPrompter,
    host_key_prompter: Option<&'a SshHostKeyPrompter>,
    connect_timeout: Duration,
    attempt: Option<&'a ConnectAttempt>,
}

pub(crate) async fn connect(
    runtime: SshConnectRuntime<'_>,
    request: SshConnectRequest,
) -> Result<SshConnectResult, String> {
    let SshConnectRequest {
        prompt_window_id,
        host_key_handling,
        options,
        mut attempt,
    } = request;
    let authentication_prompter = SshAuthenticationPrompter::new(
        runtime.app,
        runtime.state.authentication_prompts.clone(),
        prompt_window_id.clone(),
        options.request_id.clone(),
    );
    let host_key_prompter = (host_key_handling != HostKeyHandling::RequireTrusted).then(|| {
        SshHostKeyPrompter::new(
            runtime.app,
            runtime.state.host_key_prompts.clone(),
            prompt_window_id.clone(),
            options.request_id.clone(),
            host_key_handling == HostKeyHandling::Prompt,
        )
    });
    let connect_timeout = if host_key_prompter.is_some() {
        SSH_CONNECT_TIMEOUT + HOST_KEY_PROMPT_TIMEOUT
    } else {
        SSH_CONNECT_TIMEOUT
    };
    let prepared = prepare_connect(
        &runtime,
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
    let target_inputs = TargetConnectInputs {
        config,
        handler,
        jump_profile,
    };
    let target_context = TargetAttemptContext {
        options: &options,
        host_verifier: &host_verifier,
        diagnostic: &diagnostic,
        authentication_prompter: &authentication_prompter,
        host_key_prompter: host_key_prompter.as_ref(),
        connect_timeout,
        attempt: attempt.as_ref(),
    };
    let (mut handle, jump_handle) = connect_target_handle(target_inputs, &target_context).await?;
    let setup_result = run_with_attempt(
        attempt.as_ref(),
        Box::pin(authenticate_target_session(
            &mut handle,
            auth,
            &options,
            &diagnostic,
            &authentication_prompter,
        )),
    )
    .await;
    if let Err(error) = setup_result {
        cleanup_failed_connection(None, &handle, jump_handle.as_ref()).await;
        return Err(error);
    }
    let channel = start_session(
        &handle,
        jump_handle.as_ref(),
        options.cols,
        options.rows,
        Some(&diagnostic),
        attempt.as_mut(),
    )
    .await
    .map_err(|error| {
        if error != SSH_CONNECT_CANCELLED {
            let label = if error.starts_with("PTY") {
                "pty request"
            } else if error.starts_with("Shell") {
                "shell request"
            } else {
                "session channel"
            };
            emit_target_timeout_or_failure(&diagnostic, &error, label, label);
        }
        error
    })?;
    let completion = ConnectCompletion {
        session_id,
        diagnostic,
        read_rx,
    };
    let connected_target = ConnectedTarget {
        handle,
        jump_handle,
        channel,
    };

    finish_connected_session(&runtime, completion, connected_target, &options).await
}

async fn finish_connected_session(
    runtime: &SshConnectRuntime<'_>,
    completion: ConnectCompletion,
    connected_target: ConnectedTarget,
    options: &SshConnectOptions,
) -> Result<SshConnectResult, String> {
    let ConnectedTarget {
        handle,
        jump_handle,
        channel,
    } = connected_target;
    let (mut channel_read_half, channel_write_half) = channel.split();
    tokio::spawn(async move { while channel_read_half.wait().await.is_some() {} });
    register_connected_session(
        runtime.state,
        runtime.terminals,
        &completion.session_id,
        handle,
        channel_write_half,
        jump_handle,
        options,
    )
    .await;
    spawn_ssh_read_processor(
        runtime.app,
        &completion.session_id,
        runtime.terminals.clone(),
        completion.read_rx,
    );
    let _ = runtime.app.emit("ssh://connected", &completion.session_id);
    completion.diagnostic.info("target: session ready");
    Ok(SshConnectResult {
        session_id: completion.session_id,
    })
}

async fn authenticate_target_session(
    handle: &mut TargetHandle,
    auth: SshAuthRequest,
    options: &SshConnectOptions,
    diagnostic: &SshDiagnostic,
    prompter: &SshAuthenticationPrompter,
) -> Result<(), String> {
    let auth_context = prompter.context("target", &options.host, options.port, &options.username);
    authenticate_target(handle, &options.username, auth, diagnostic, &auth_context).await?;
    Ok(())
}

fn prepare_connect(
    runtime: &SshConnectRuntime<'_>,
    prompt_window_id: &str,
    options: &SshConnectOptions,
    host_key_prompter: Option<SshHostKeyPrompter>,
) -> Result<ConnectPreparation, String> {
    let session_id = Uuid::new_v4().to_string();
    let diagnostic = SshDiagnostic::new(
        runtime.app,
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
        app: runtime.app.clone(),
        session_id: session_id.clone(),
        sessions: runtime.state.sessions.clone(),
        host_verifier: host_verifier.clone(),
        host_key_prompter,
        diagnostic: diagnostic.clone(),
        terminals: runtime.terminals.clone(),
        workspace: runtime.workspace.clone(),
        logger: runtime.logger.cloned(),
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
    inputs: TargetConnectInputs,
    context: &TargetAttemptContext<'_>,
) -> Result<(TargetHandle, Option<JumpHandle>), String> {
    let TargetConnectInputs {
        config,
        handler,
        jump_profile,
    } = inputs;
    match jump_profile {
        Some(jump_profile) => connect_target_via_jump(config, handler, jump_profile, context).await,
        None => connect_target_direct(config, handler, context).await,
    }
}

async fn connect_target_via_jump(
    config: Arc<russh::client::Config>,
    handler: SshClientHandler,
    jump_profile: SshJumpProfile,
    context: &TargetAttemptContext<'_>,
) -> Result<(TargetHandle, Option<JumpHandle>), String> {
    let (jump_handle, jump_channel) = connect_jump_profile(
        JumpConnectInputs {
            config: config.clone(),
            profile: jump_profile,
            target_host: &context.options.host,
            target_port: context.options.port,
            password: context.options.jump_password.clone(),
            key_passphrase: context.options.jump_key_passphrase.clone(),
        },
        JumpAttemptContext {
            diagnostic: Some(context.diagnostic),
            authentication_prompter: context.authentication_prompter,
            host_key_prompter: context.host_key_prompter,
            connect_timeout: context.connect_timeout,
            attempt: context.attempt,
        },
    )
    .await?;
    let stream = jump_channel.into_stream();
    context.diagnostic.progress("target", "connecting");
    context.diagnostic.info("target: starting SSH handshake");
    let handle = run_with_attempt(
        context.attempt,
        Box::pin(run_ssh_operation_with_timeout(
            context.connect_timeout,
            SSH_CONNECT_TIMEOUT_ERROR,
            async {
                russh::client::connect_stream(config, stream, handler)
                    .await
                    .map_err(|error| map_connect_error(error, context.host_verifier))
            },
        )),
    )
    .await;
    match handle {
        Ok(handle) => Ok((handle, Some(jump_handle))),
        Err(error) => {
            if error != SSH_CONNECT_CANCELLED {
                emit_target_timeout_or_failure(
                    context.diagnostic,
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
    context: &TargetAttemptContext<'_>,
) -> Result<(TargetHandle, Option<JumpHandle>), String> {
    context.diagnostic.progress("target", "connecting");
    context.diagnostic.info("target: starting SSH handshake");
    let handle = run_with_attempt(
        context.attempt,
        Box::pin(run_ssh_operation_with_timeout(
            context.connect_timeout,
            SSH_CONNECT_TIMEOUT_ERROR,
            async {
                russh::client::connect(
                    config,
                    (context.options.host.as_str(), context.options.port),
                    handler,
                )
                .await
                .map_err(|error| map_connect_error(error, context.host_verifier))
            },
        )),
    )
    .await
    .map_err(|error| {
        if error != SSH_CONNECT_CANCELLED {
            emit_target_timeout_or_failure(
                context.diagnostic,
                &error,
                "SSH handshake",
                "SSH handshake",
            );
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

async fn authenticate_target(
    handle: &mut TargetHandle,
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
            Err(error)
        }
    }
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
