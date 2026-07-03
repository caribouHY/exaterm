use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use russh::keys::PublicKey;
use russh::Disconnect;
use tauri::AppHandle;

use crate::config::config_load;
use crate::ssh::client_config::load_client_config;
#[cfg(not(test))]
use crate::ssh::diagnostics::host_key_error_message;
use crate::ssh::diagnostics::{
    emit_host_key_diagnostic_for_role, normalize_diagnostic_role, PendingHostKey, SshDiagnostic,
};
use crate::ssh::io::{
    run_ssh_operation_with_timeout, SshState, SSH_CONNECT_TIMEOUT, SSH_CONNECT_TIMEOUT_ERROR,
};
use crate::ssh::jump::connect_jump_profile;
use crate::ssh::profiles::resolve_jump_profile;
use crate::ssh::types::{SshJumpProfile, SshProbeHostKeyOptions};
use crate::ssh_known_hosts::{
    endpoint_cache_key, inspect_host_key_with_path, known_hosts_path, write_trusted_host,
    HostKeyCheckResult, HostKeyCheckStatus,
};

#[derive(Clone, Copy)]
pub(super) enum HostKeyVerificationMode {
    Probe,
    Enforce,
}

#[derive(Clone)]
pub(super) struct HostKeyVerifier {
    host: String,
    port: u16,
    known_hosts_path: PathBuf,
    mode: HostKeyVerificationMode,
    observed_key: Arc<StdMutex<Option<PendingHostKey>>>,
    last_result: Arc<StdMutex<Option<HostKeyCheckResult>>>,
}

impl HostKeyVerifier {
    pub(super) fn probe(host: String, port: u16) -> Self {
        Self::with_path(
            host,
            port,
            HostKeyVerificationMode::Probe,
            known_hosts_path(),
        )
    }

    pub(super) fn enforce(host: String, port: u16) -> Self {
        Self::with_path(
            host,
            port,
            HostKeyVerificationMode::Enforce,
            known_hosts_path(),
        )
    }

    pub(super) fn with_path(
        host: String,
        port: u16,
        mode: HostKeyVerificationMode,
        known_hosts_path: PathBuf,
    ) -> Self {
        Self {
            host,
            port,
            known_hosts_path,
            mode,
            observed_key: Arc::new(StdMutex::new(None)),
            last_result: Arc::new(StdMutex::new(None)),
        }
    }

    pub(super) fn check_key(&self, server_public_key: &PublicKey) -> Result<bool, russh::Error> {
        {
            let mut observed = self
                .observed_key
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *observed = Some(PendingHostKey {
                key: server_public_key.clone(),
            });
        }

        let result = inspect_host_key_with_path(
            &self.host,
            self.port,
            server_public_key,
            &self.known_hosts_path,
        )
        .map_err(|error| russh::Error::IO(std::io::Error::new(std::io::ErrorKind::Other, error)))?;

        {
            let mut decision = self
                .last_result
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *decision = Some(result.clone());
        }

        Ok(match self.mode {
            HostKeyVerificationMode::Probe => true,
            HostKeyVerificationMode::Enforce => result.status == HostKeyCheckStatus::Trusted,
        })
    }

    pub(super) fn last_result(&self) -> Option<HostKeyCheckResult> {
        self.last_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn observed_key(&self) -> Option<PendingHostKey> {
        self.observed_key
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

pub(super) struct ProbeClientHandler {
    pub(super) host_verifier: HostKeyVerifier,
}

struct ProbeConnectContext<'a> {
    config: Arc<russh::client::Config>,
    handler: ProbeClientHandler,
    host: &'a str,
    port: u16,
    diagnostic: Option<&'a SshDiagnostic>,
    role: &'static str,
}

impl russh::client::Handler for ProbeClientHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        let result = self.host_verifier.check_key(server_public_key);
        async move { result }
    }
}

async fn run_host_key_probe(
    host: &str,
    port: u16,
    jump_profile: Option<SshJumpProfile>,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
    diagnostic: Option<SshDiagnostic>,
    role: &'static str,
) -> Result<(HostKeyCheckResult, PendingHostKey), String> {
    let config = Arc::new(load_client_config()?);
    let verifier = HostKeyVerifier::probe(host.to_string(), port);
    let handler = ProbeClientHandler {
        host_verifier: verifier.clone(),
    };
    let context = ProbeConnectContext {
        config,
        handler,
        host,
        port,
        diagnostic: diagnostic.as_ref(),
        role,
    };
    let (handle, jump_handle) =
        connect_probe_handle(context, jump_profile, jump_password, jump_key_passphrase).await?;
    disconnect_probe_handles(handle, jump_handle).await;
    let (result, observed_key) = collected_probe_result(&verifier)?;
    if let Some(diagnostic) = &diagnostic {
        emit_host_key_diagnostic_for_role(diagnostic, role, &result);
    }
    Ok((result, observed_key))
}

async fn connect_probe_handle(
    context: ProbeConnectContext<'_>,
    jump_profile: Option<SshJumpProfile>,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
) -> Result<
    (
        russh::client::Handle<ProbeClientHandler>,
        Option<russh::client::Handle<ProbeClientHandler>>,
    ),
    String,
> {
    match jump_profile {
        Some(jump_profile) => {
            connect_probe_via_jump(context, jump_profile, jump_password, jump_key_passphrase).await
        }
        None => connect_probe_direct(context).await,
    }
}

async fn connect_probe_via_jump(
    context: ProbeConnectContext<'_>,
    jump_profile: SshJumpProfile,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
) -> Result<
    (
        russh::client::Handle<ProbeClientHandler>,
        Option<russh::client::Handle<ProbeClientHandler>>,
    ),
    String,
> {
    let (jump_handle, jump_channel) = connect_jump_profile(
        context.config.clone(),
        jump_profile,
        context.host,
        context.port,
        jump_password,
        jump_key_passphrase,
        context.diagnostic,
    )
    .await?;
    let stream = jump_channel.into_stream();
    emit_probe_start(context.diagnostic, context.role);
    let handle =
        run_ssh_operation_with_timeout(SSH_CONNECT_TIMEOUT, SSH_CONNECT_TIMEOUT_ERROR, async {
            russh::client::connect_stream(context.config, stream, context.handler)
                .await
                .map_err(|error| format!("SSH接続エラー: {}", error))
        })
        .await
        .map_err(|error| {
            emit_probe_error(context.diagnostic, context.role, &error);
            error
        });
    match handle {
        Ok(handle) => Ok((handle, Some(jump_handle))),
        Err(error) => {
            let _ = jump_handle
                .disconnect(Disconnect::ByApplication, "Target handshake failed", "en")
                .await;
            Err(error)
        }
    }
}

async fn connect_probe_direct(
    context: ProbeConnectContext<'_>,
) -> Result<
    (
        russh::client::Handle<ProbeClientHandler>,
        Option<russh::client::Handle<ProbeClientHandler>>,
    ),
    String,
> {
    emit_probe_start(context.diagnostic, context.role);
    let handle =
        run_ssh_operation_with_timeout(SSH_CONNECT_TIMEOUT, SSH_CONNECT_TIMEOUT_ERROR, async {
            russh::client::connect(
                context.config,
                (context.host, context.port),
                context.handler,
            )
            .await
            .map_err(|error| format!("SSH接続エラー: {}", error))
        })
        .await
        .map_err(|error| {
            emit_probe_error(context.diagnostic, context.role, &error);
            error
        })?;
    Ok((handle, None))
}

fn emit_probe_start(diagnostic: Option<&SshDiagnostic>, role: &str) {
    if let Some(diagnostic) = diagnostic {
        diagnostic.info(format!("{role}: starting SSH handshake"));
    }
}

fn emit_probe_error(diagnostic: Option<&SshDiagnostic>, role: &str, error: &str) {
    if let Some(diagnostic) = diagnostic {
        if error == SSH_CONNECT_TIMEOUT_ERROR {
            diagnostic.error(format!("error: {role} SSH handshake timed out"));
        } else {
            diagnostic.error(format!("error: {role} SSH handshake failed"));
        }
    }
}

async fn disconnect_probe_handles(
    handle: russh::client::Handle<ProbeClientHandler>,
    jump_handle: Option<russh::client::Handle<ProbeClientHandler>>,
) {
    let _ = handle
        .disconnect(Disconnect::ByApplication, "Host key probe completed", "en")
        .await;
    if let Some(jump_handle) = jump_handle {
        let _ = jump_handle
            .disconnect(Disconnect::ByApplication, "Host key probe completed", "en")
            .await;
    }
}

fn collected_probe_result(
    verifier: &HostKeyVerifier,
) -> Result<(HostKeyCheckResult, PendingHostKey), String> {
    let result = verifier
        .last_result()
        .ok_or_else(|| "SSHホスト鍵を取得できませんでした".to_string())?;
    let observed_key = verifier
        .observed_key()
        .ok_or_else(|| "SSHホスト鍵を取得できませんでした".to_string())?;
    Ok((result, observed_key))
}

#[cfg(not(test))]
pub async fn verify_trusted_host_key(host: &str, port: u16) -> Result<(), String> {
    let (result, _) = run_host_key_probe(host, port, None, None, None, None, "target").await?;
    if result.status == HostKeyCheckStatus::Trusted {
        Ok(())
    } else {
        Err(host_key_error_message(&result))
    }
}

#[cfg(not(test))]
pub async fn verify_trusted_host_key_via_jump(
    host: &str,
    port: u16,
    jump_profile: SshJumpProfile,
    jump_password: Option<String>,
    jump_key_passphrase: Option<String>,
) -> Result<(), String> {
    let (result, _) = run_host_key_probe(
        host,
        port,
        Some(jump_profile),
        jump_password,
        jump_key_passphrase,
        None,
        "target",
    )
    .await?;
    if result.status == HostKeyCheckStatus::Trusted {
        Ok(())
    } else {
        Err(host_key_error_message(&result))
    }
}

pub async fn ssh_probe_host_key(
    app: AppHandle,
    state: tauri::State<'_, SshState>,
    options: SshProbeHostKeyOptions,
) -> Result<HostKeyCheckResult, String> {
    let config = config_load()?;
    let jump_profile = resolve_jump_profile(&config, options.jump_profile_id.as_deref(), None)?;
    let diagnostic = SshDiagnostic::new(&app, options.request_id.clone());
    let role = normalize_diagnostic_role(options.diagnostic_role);
    let (result, pending_key) = run_host_key_probe(
        &options.host,
        options.port,
        jump_profile,
        options.jump_password,
        options.jump_key_passphrase,
        Some(diagnostic),
        role,
    )
    .await?;
    state
        .pending_host_keys
        .lock()
        .await
        .insert(endpoint_cache_key(&options.host, options.port), pending_key);
    Ok(result)
}

pub async fn ssh_trust_host_key(
    state: tauri::State<'_, SshState>,
    host: String,
    port: u16,
    replace: bool,
) -> Result<(), String> {
    let endpoint = endpoint_cache_key(&host, port);
    let pending_key = {
        let pending = state.pending_host_keys.lock().await;
        pending.get(&endpoint).cloned().ok_or_else(|| {
            "直前に取得したSSHホスト鍵が見つかりません。もう一度接続してください。".to_string()
        })?
    };

    write_trusted_host(&host, port, &pending_key.key, replace)?;
    state.pending_host_keys.lock().await.remove(&endpoint);
    Ok(())
}
