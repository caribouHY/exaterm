use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use russh::keys::PublicKey;

use crate::ssh::diagnostics::{emit_host_key_diagnostic_for_role, SshDiagnostic};
use crate::ssh::host_key_prompt::{
    SshHostKeyPrompter, HOST_KEY_PROMPT_CANCELLED, HOST_KEY_PROMPT_TIMEOUT_ERROR,
};
use crate::ssh_known_hosts::{
    inspect_host_key_with_path, known_hosts_path, HostKeyCheckResult, HostKeyCheckStatus,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKeyHandling {
    Prompt,
    #[cfg_attr(test, allow(dead_code))]
    PromptUnknown,
    RequireTrusted,
}

#[derive(Clone)]
pub(super) struct HostKeyVerifier {
    host: String,
    port: u16,
    known_hosts_path: PathBuf,
    last_result: Arc<StdMutex<Option<HostKeyCheckResult>>>,
    last_error: Arc<StdMutex<Option<String>>>,
}

pub(super) struct SshHostKeyHandler {
    pub(super) verifier: HostKeyVerifier,
    pub(super) prompter: Option<SshHostKeyPrompter>,
    pub(super) phase: &'static str,
    pub(super) diagnostic: Option<SshDiagnostic>,
}

impl russh::client::Handler for SshHostKeyHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        let verifier = self.verifier.clone();
        let prompter = self.prompter.clone();
        let phase = self.phase;
        let diagnostic = self.diagnostic.clone();
        let server_public_key = server_public_key.clone();
        async move { verify_server_key(verifier, prompter, phase, diagnostic, server_public_key).await }
    }
}

impl HostKeyVerifier {
    pub(super) fn new(host: String, port: u16) -> Self {
        Self::with_path(host, port, known_hosts_path())
    }

    pub(super) fn with_path(host: String, port: u16, known_hosts_path: PathBuf) -> Self {
        Self {
            host,
            port,
            known_hosts_path,
            last_result: Arc::new(StdMutex::new(None)),
            last_error: Arc::new(StdMutex::new(None)),
        }
    }

    pub(super) fn inspect_key(
        &self,
        server_public_key: &PublicKey,
    ) -> Result<HostKeyCheckResult, russh::Error> {
        let result = inspect_host_key_with_path(
            &self.host,
            self.port,
            server_public_key,
            &self.known_hosts_path,
        )
        .map_err(string_to_russh_error)?;
        *self
            .last_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(result.clone());
        Ok(result)
    }

    #[cfg(test)]
    pub(super) fn check_key(&self, server_public_key: &PublicKey) -> Result<bool, russh::Error> {
        Ok(self.inspect_key(server_public_key)?.status == HostKeyCheckStatus::Trusted)
    }

    pub(super) fn last_result(&self) -> Option<HostKeyCheckResult> {
        self.last_result
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(super) fn last_error(&self) -> Option<String> {
        self.last_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn record_error(&self, error: String) {
        *self
            .last_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(error);
    }

    fn known_hosts_path(&self) -> &Path {
        &self.known_hosts_path
    }
}

pub(super) async fn verify_server_key(
    verifier: HostKeyVerifier,
    prompter: Option<SshHostKeyPrompter>,
    phase: &'static str,
    diagnostic: Option<SshDiagnostic>,
    server_public_key: PublicKey,
) -> Result<bool, russh::Error> {
    if let Some(diagnostic) = &diagnostic {
        diagnostic.progress(phase, "verifying_host_key");
    }
    let result = verifier.inspect_key(&server_public_key)?;
    if let Some(diagnostic) = &diagnostic {
        emit_host_key_diagnostic_for_role(diagnostic, phase, &result);
    }
    if result.status == HostKeyCheckStatus::Trusted {
        return Ok(true);
    }

    let Some(prompter) = prompter else {
        return Ok(false);
    };
    if !prompter.allows_status(&result.status) {
        return Ok(false);
    }
    if let Err(error) = prompter
        .confirm(
            phase,
            result,
            &server_public_key,
            verifier.known_hosts_path(),
        )
        .await
    {
        if let Some(diagnostic) = &diagnostic {
            let decision = if error == HOST_KEY_PROMPT_CANCELLED {
                "rejected"
            } else if error == HOST_KEY_PROMPT_TIMEOUT_ERROR {
                "timed out"
            } else {
                "failed"
            };
            diagnostic.info(format!("{phase}: host key confirmation {decision}"));
        }
        verifier.record_error(error.clone());
        return Err(string_to_russh_error(error));
    }

    if let Some(diagnostic) = &diagnostic {
        diagnostic.info(format!("{phase}: host key trusted"));
    }
    Ok(true)
}

fn string_to_russh_error(error: String) -> russh::Error {
    russh::Error::IO(std::io::Error::new(std::io::ErrorKind::Other, error))
}
