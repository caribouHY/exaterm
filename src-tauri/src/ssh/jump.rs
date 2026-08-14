use std::sync::Arc;
use std::time::Duration;

use russh::Disconnect;

use crate::connect_attempt::{run_with_attempt, ConnectAttempt};
use crate::ssh::auth::{authenticate_ssh, build_auth_request};
use crate::ssh::authentication_prompt::SshAuthenticationPrompter;
use crate::ssh::diagnostics::{map_connect_error, SshDiagnostic};
use crate::ssh::host_key::{HostKeyVerifier, SshHostKeyHandler};
use crate::ssh::host_key_prompt::SshHostKeyPrompter;
use crate::ssh::io::{
    run_ssh_operation_with_timeout, SSH_AUTH_TIMEOUT_ERROR, SSH_CHANNEL_OPEN_TIMEOUT,
    SSH_CONNECT_TIMEOUT_ERROR, SSH_JUMP_CHANNEL_OPEN_TIMEOUT_ERROR,
};
use crate::ssh::types::SshJumpProfile;

pub(super) async fn connect_jump_profile(
    config: Arc<russh::client::Config>,
    jump_profile: SshJumpProfile,
    target_host: &str,
    target_port: u16,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
    diagnostic: Option<&SshDiagnostic>,
    authentication_prompter: &SshAuthenticationPrompter,
    host_key_prompter: Option<&SshHostKeyPrompter>,
    connect_timeout: Duration,
    attempt: Option<&ConnectAttempt>,
) -> Result<
    (
        russh::client::Handle<SshHostKeyHandler>,
        russh::Channel<russh::client::Msg>,
    ),
    String,
> {
    let auth = build_auth_request(
        Some(jump_profile.auth_method.clone()),
        jump_password.unwrap_or_default(),
        jump_profile.private_key_path.clone(),
        jump_key_passphrase,
        None,
    )?;
    let jump_verifier = HostKeyVerifier::new(jump_profile.host.clone(), jump_profile.port);
    let mut handle = run_with_attempt(
        attempt,
        connect_jump_ssh(
            config,
            &jump_profile,
            &jump_verifier,
            diagnostic,
            host_key_prompter,
            connect_timeout,
        ),
    )
    .await?;
    let auth_context = authentication_prompter.context(
        "jump",
        &jump_profile.host,
        jump_profile.port,
        &jump_profile.username,
    );
    if let Err(error) = run_with_attempt(
        attempt,
        authenticate_jump(
            &mut handle,
            &jump_profile.username,
            auth,
            diagnostic,
            &auth_context,
        ),
    )
    .await
    {
        let _ = handle
            .disconnect(Disconnect::ByApplication, "Jump authentication ended", "en")
            .await;
        return Err(error);
    }
    let channel = match run_with_attempt(
        attempt,
        open_jump_direct_tcpip(&mut handle, target_host, target_port, diagnostic),
    )
    .await
    {
        Ok(channel) => channel,
        Err(error) => {
            let _ = handle
                .disconnect(Disconnect::ByApplication, "Jump channel open ended", "en")
                .await;
            return Err(error);
        }
    };
    Ok((handle, channel))
}

async fn connect_jump_ssh(
    config: Arc<russh::client::Config>,
    jump_profile: &SshJumpProfile,
    jump_verifier: &HostKeyVerifier,
    diagnostic: Option<&SshDiagnostic>,
    host_key_prompter: Option<&SshHostKeyPrompter>,
    connect_timeout: Duration,
) -> Result<russh::client::Handle<SshHostKeyHandler>, String> {
    let handler = SshHostKeyHandler {
        verifier: jump_verifier.clone(),
        prompter: host_key_prompter.cloned(),
        phase: "jump",
        diagnostic: diagnostic.cloned(),
    };
    if let Some(diagnostic) = diagnostic {
        diagnostic.progress("jump", "connecting");
        diagnostic.info("jump: connecting");
    }
    run_ssh_operation_with_timeout(connect_timeout, SSH_CONNECT_TIMEOUT_ERROR, async {
        russh::client::connect(
            config,
            (jump_profile.host.as_str(), jump_profile.port),
            handler,
        )
        .await
        .map_err(|error| map_connect_error(error, jump_verifier))
    })
    .await
    .map_err(|error| {
        emit_jump_error(
            diagnostic,
            &error,
            SSH_CONNECT_TIMEOUT_ERROR,
            "SSH handshake",
        );
        error
    })
}

async fn authenticate_jump(
    handle: &mut russh::client::Handle<SshHostKeyHandler>,
    username: &str,
    auth: crate::ssh::types::SshAuthRequest,
    diagnostic: Option<&SshDiagnostic>,
    context: &crate::ssh::authentication_prompt::SshAuthenticationContext<'_>,
) -> Result<(), String> {
    if let Some(diagnostic) = diagnostic {
        diagnostic.progress("jump", "authenticating");
        diagnostic.info("jump: host key accepted");
        diagnostic.info("jump: authentication started");
    }
    let result = authenticate_ssh(handle, username, auth, context, diagnostic).await;
    match result {
        Ok(()) => {
            if let Some(diagnostic) = diagnostic {
                diagnostic.info("jump: authentication succeeded");
            }
            Ok(())
        }
        Err(error) => {
            emit_jump_error(diagnostic, &error, SSH_AUTH_TIMEOUT_ERROR, "authentication");
            let _ = handle
                .disconnect(
                    Disconnect::ByApplication,
                    "Jump authentication failed",
                    "en",
                )
                .await;
            Err(error)
        }
    }
}

async fn open_jump_direct_tcpip(
    handle: &mut russh::client::Handle<SshHostKeyHandler>,
    target_host: &str,
    target_port: u16,
    diagnostic: Option<&SshDiagnostic>,
) -> Result<russh::Channel<russh::client::Msg>, String> {
    if let Some(diagnostic) = diagnostic {
        diagnostic.info("jump: opening direct-tcpip channel");
    }
    let result = run_ssh_operation_with_timeout(
        SSH_CHANNEL_OPEN_TIMEOUT,
        SSH_JUMP_CHANNEL_OPEN_TIMEOUT_ERROR,
        async {
            handle
                .channel_open_direct_tcpip(target_host, u32::from(target_port), "127.0.0.1", 0)
                .await
                .map_err(|error| format!("Failed to open the SSH jump channel: {}", error))
        },
    )
    .await;
    match result {
        Ok(channel) => {
            if let Some(diagnostic) = diagnostic {
                diagnostic.info("jump: direct-tcpip channel opened");
            }
            Ok(channel)
        }
        Err(error) => {
            emit_jump_error(
                diagnostic,
                &error,
                SSH_JUMP_CHANNEL_OPEN_TIMEOUT_ERROR,
                "direct-tcpip channel",
            );
            let _ = handle
                .disconnect(Disconnect::ByApplication, "Jump channel open failed", "en")
                .await;
            Err(error)
        }
    }
}

fn emit_jump_error(
    diagnostic: Option<&SshDiagnostic>,
    error: &str,
    timeout_error: &str,
    operation: &str,
) {
    if let Some(diagnostic) = diagnostic {
        if error == timeout_error {
            diagnostic.error(format!("error: jump {operation} timed out"));
        } else {
            diagnostic.error(format!("error: jump {operation} failed"));
        }
    }
}
