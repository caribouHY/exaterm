use std::time::Duration;

use crate::connect_attempt::{run_with_attempt, ConnectAttempt};
use russh::{client, Channel, ChannelMsg, Disconnect};

use super::diagnostics::SshDiagnostic;
use super::io::{
    run_ssh_operation_with_timeout, SSH_CHANNEL_OPEN_TIMEOUT, SSH_CHANNEL_OPEN_TIMEOUT_ERROR,
    SSH_PTY_TIMEOUT, SSH_PTY_TIMEOUT_ERROR, SSH_SHELL_TIMEOUT, SSH_SHELL_TIMEOUT_ERROR,
};

pub(super) const SSH_SETUP_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn start_session<H, J>(
    handle: &client::Handle<H>,
    jump_handle: Option<&client::Handle<J>>,
    cols: u32,
    rows: u32,
    diagnostic: Option<&SshDiagnostic>,
    mut attempt: Option<&mut ConnectAttempt>,
) -> Result<Channel<client::Msg>, String>
where
    H: client::Handler + 'static,
    J: client::Handler + 'static,
{
    let mut channel = None;
    let result = run_with_attempt(
        attempt.as_deref(),
        Box::pin(open_and_start_shell(
            handle,
            &mut channel,
            cols,
            rows,
            diagnostic,
        )),
    )
    .await;
    let result = result.and_then(|()| {
        if attempt
            .as_mut()
            .is_some_and(|attempt| !attempt.begin_completion())
        {
            Err("The SSH connection attempt was cancelled".to_string())
        } else {
            Ok(())
        }
    });
    if let Err(error) = result {
        cleanup_failed_connection(channel.as_ref(), handle, jump_handle).await;
        return Err(error);
    }
    Ok(channel.expect("successful shell setup owns a channel"))
}

pub(super) async fn open_and_start_shell<H: client::Handler + 'static>(
    handle: &client::Handle<H>,
    channel_slot: &mut Option<Channel<client::Msg>>,
    cols: u32,
    rows: u32,
    diagnostic: Option<&SshDiagnostic>,
) -> Result<(), String> {
    if let Some(diagnostic) = diagnostic {
        diagnostic.progress("target", "opening_session");
        diagnostic.info("target: opening session channel");
    }
    *channel_slot = Some(
        run_ssh_operation_with_timeout(
            SSH_CHANNEL_OPEN_TIMEOUT,
            SSH_CHANNEL_OPEN_TIMEOUT_ERROR,
            async {
                handle
                    .channel_open_session()
                    .await
                    .map_err(|error| format!("Failed to open the SSH channel: {error}"))
            },
        )
        .await?,
    );
    let channel = channel_slot.as_mut().expect("session channel was opened");
    if let Some(diagnostic) = diagnostic {
        diagnostic.info("target: requesting pty");
    }
    ShellStartRequest::Pty { cols, rows }
        .send_and_confirm(channel)
        .await?;
    if let Some(diagnostic) = diagnostic {
        diagnostic.info("target: requesting shell");
    }
    ShellStartRequest::Shell.send_and_confirm(channel).await
}

pub(super) enum ShellStartRequest {
    Pty { cols: u32, rows: u32 },
    Shell,
}

impl ShellStartRequest {
    pub(super) async fn send_and_confirm(
        self,
        channel: &mut Channel<client::Msg>,
    ) -> Result<(), String> {
        let (timeout, timeout_error, failure) = match self {
            Self::Pty { .. } => (SSH_PTY_TIMEOUT, SSH_PTY_TIMEOUT_ERROR, "PTY request failed"),
            Self::Shell => (
                SSH_SHELL_TIMEOUT,
                SSH_SHELL_TIMEOUT_ERROR,
                "Shell request failed",
            ),
        };
        run_ssh_operation_with_timeout(timeout, timeout_error, async {
            match self {
                Self::Pty { cols, rows } => {
                    channel
                        .request_pty(true, "xterm-256color", cols, rows, 0, 0, &[])
                        .await
                }
                Self::Shell => channel.request_shell(true).await,
            }
            .map_err(|_| failure.to_string())?;
            loop {
                match channel.wait().await {
                    Some(ChannelMsg::Success) => return Ok(()),
                    Some(ChannelMsg::Failure | ChannelMsg::Eof | ChannelMsg::Close) | None => {
                        return Err(failure.to_string());
                    }
                    // Data is already queued by SshClientHandler; only consume the channel copy.
                    Some(_) => {}
                }
            }
        })
        .await
    }
}

pub(super) async fn cleanup_failed_connection<H, J>(
    channel: Option<&Channel<client::Msg>>,
    handle: &client::Handle<H>,
    jump_handle: Option<&client::Handle<J>>,
) where
    H: client::Handler + 'static,
    J: client::Handler + 'static,
{
    // A blocked close must not prevent either transport from receiving disconnect.
    let _ = tokio::time::timeout(SSH_SETUP_CLEANUP_TIMEOUT, async {
        tokio::join!(
            async {
                if let Some(channel) = channel {
                    let _ = channel.close().await;
                }
            },
            async {
                let _ = handle
                    .disconnect(Disconnect::ByApplication, "Connection setup failed", "en")
                    .await;
            },
            async {
                if let Some(jump_handle) = jump_handle {
                    let _ = jump_handle
                        .disconnect(Disconnect::ByApplication, "Connection setup failed", "en")
                        .await;
                }
            },
        );
    })
    .await;
}

#[cfg(test)]
mod tests;
