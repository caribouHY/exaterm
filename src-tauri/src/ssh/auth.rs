use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use russh::client::{AuthResult, KeyboardInteractiveAuthResponse};
use russh::keys::decode_secret_key;
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::PrivateKey;
use russh::{MethodKind, MethodSet};

use crate::ssh::authentication_prompt::SshAuthenticationContext;
use crate::ssh::diagnostics::SshDiagnostic;
use crate::ssh::io::{run_ssh_operation_with_timeout, SSH_AUTH_TIMEOUT, SSH_AUTH_TIMEOUT_ERROR};
use crate::ssh::types::SshAuthRequest;

const MAX_AUTHENTICATION_STAGES: usize = 8;

pub(super) fn build_auth_request(
    auth_method: Option<String>,
    password: String,
    private_key_path: Option<String>,
    key_passphrase: Option<String>,
    default_private_key_path: Option<String>,
) -> Result<SshAuthRequest, String> {
    let method = auth_method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("auto");

    match method {
        "auto" => Ok(SshAuthRequest::Auto {
            private_key_path: normalize_optional_string(private_key_path)
                .or_else(|| normalize_optional_string(default_private_key_path)),
            key_passphrase: normalize_optional_string(key_passphrase),
        }),
        "password" => Ok(SshAuthRequest::Password { password }),
        "keyboard_interactive" => Ok(SshAuthRequest::KeyboardInteractive),
        "public_key" => {
            let private_key_path = private_key_path.unwrap_or_default().trim().to_string();
            if private_key_path.is_empty() {
                return Err(
                    "SSH public key authentication error: specify a private key file".to_string(),
                );
            }

            let key_passphrase = normalize_optional_string(key_passphrase);

            Ok(SshAuthRequest::PublicKey {
                private_key_path,
                key_passphrase,
            })
        }
        _ => Err("The SSH authentication method is invalid".to_string()),
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn expand_percent_env_vars(path: &str) -> String {
    let mut expanded = String::new();
    let mut rest = path;

    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        if let Some(end) = after_start.find('%') {
            let name = &after_start[..end];
            if let Ok(value) = std::env::var(name) {
                expanded.push_str(&value);
            } else {
                expanded.push('%');
                expanded.push_str(name);
                expanded.push('%');
            }
            rest = &after_start[end + 1..];
        } else {
            expanded.push_str(&rest[start..]);
            return expanded;
        }
    }

    expanded.push_str(rest);
    expanded
}

pub(super) fn normalize_auth_path(path: &str) -> PathBuf {
    let trimmed = path.trim().trim_matches('"').trim_matches('\'');
    let expanded = expand_percent_env_vars(trimmed);

    if let Some(rest) = expanded
        .strip_prefix("~/")
        .or_else(|| expanded.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(expanded)
}

const PEM_BEGIN_PREFIX: &str = "-----BEGIN ";
const PEM_END_SUFFIX: &str = "-----";
pub(super) const SUPPORTED_PRIVATE_KEY_LABELS: &[&str] = &[
    "OPENSSH PRIVATE KEY",
    "RSA PRIVATE KEY",
    "DSA PRIVATE KEY",
    "EC PRIVATE KEY",
    "ENCRYPTED PRIVATE KEY",
    "PRIVATE KEY",
];

fn pem_header_label(line: &str) -> Option<&str> {
    line.strip_prefix(PEM_BEGIN_PREFIX)?
        .strip_suffix(PEM_END_SUFFIX)
}

pub(super) fn is_supported_private_key_header(line: &str) -> bool {
    pem_header_label(line).is_some_and(|label| SUPPORTED_PRIVATE_KEY_LABELS.contains(&label))
}

pub(super) fn private_key_format_hint(secret: &str) -> Result<(), String> {
    let first_line = secret.lines().find(|line| !line.trim().is_empty());

    match first_line.map(str::trim) {
        Some(line) if line.starts_with("ssh-") => Err(
            "A public key file was specified instead of a private key file. Specify the private key itself."
                .to_string(),
        ),
        Some(line) if line.starts_with("PuTTY-User-Key-File-") => Err(
            "PuTTY-format (.ppk) private keys cannot be loaded directly. Convert the key to OpenSSH format."
                .to_string(),
        ),
        Some(line)
            if line.starts_with("-----BEGIN ")
                && line.contains("PUBLIC KEY")
                && !line.contains("PRIVATE KEY") =>
        {
            Err(
                "A public key file was specified instead of a private key file. Specify the private key itself."
                    .to_string(),
            )
        }
        Some(line) if is_supported_private_key_header(line) => Ok(()),
        _ => Err(
            "Specify an OpenSSH or PEM private key file. Public key files cannot be used as private keys."
                .to_string(),
        ),
    }
}

fn read_private_key_secret(path: &str) -> Result<(PathBuf, String), String> {
    let path = normalize_auth_path(path);
    let secret = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Failed to open the private key file: {} ({})",
            path.to_string_lossy(),
            error
        )
    })?;

    Ok((path, secret))
}

pub fn private_key_requires_passphrase(path: &str) -> Result<bool, String> {
    if path.trim().is_empty() {
        return Err("Specify a private key file".to_string());
    }

    let (_path, secret) = read_private_key_secret(path)?;
    private_key_format_hint(&secret)?;
    match decode_secret_key(&secret, None) {
        Ok(_) => Ok(false),
        Err(russh::keys::Error::KeyIsEncrypted) => Ok(true),
        Err(russh::keys::Error::CouldNotReadKey) => Err(
            "Failed to load the private key. Check the key format, passphrase, or file contents."
                .to_string(),
        ),
        Err(other) => Err(format!("Failed to load the private key: {}", other)),
    }
}

pub fn ssh_private_key_requires_passphrase(private_key_path: String) -> Result<bool, String> {
    private_key_requires_passphrase(&private_key_path)
        .map_err(|error| format!("SSH public key authentication error: {}", error))
}

fn load_private_key_for_auth(path: &str, passphrase: Option<&str>) -> Result<PrivateKey, String> {
    let (_path, secret) = read_private_key_secret(path)?;
    private_key_format_hint(&secret)?;
    decode_secret_key(&secret, passphrase).map_err(|error| match error {
        russh::keys::Error::KeyIsEncrypted => {
            "The private key is encrypted with a passphrase. Enter the key passphrase.".to_string()
        }
        russh::keys::Error::CouldNotReadKey => {
            "Failed to load the private key. Check the key format, passphrase, or file contents."
                .to_string()
        }
        other => format!("Failed to load the private key: {}", other),
    })
}

pub(super) async fn authenticate_ssh(
    handle: &mut russh::client::Handle<impl russh::client::Handler + Send + 'static>,
    username: &str,
    auth: SshAuthRequest,
    context: &SshAuthenticationContext<'_>,
    diagnostic: Option<&SshDiagnostic>,
) -> Result<(), String> {
    match auth {
        SshAuthRequest::Auto {
            private_key_path,
            key_passphrase,
        } => {
            authenticate_auto(
                handle,
                username,
                private_key_path,
                key_passphrase,
                context,
                diagnostic,
            )
            .await
        }
        SshAuthRequest::Password { password } => {
            let password = if password.is_empty() {
                context.request_password().await?
            } else {
                password
            };
            match authenticate_password(handle, username, password).await? {
                AuthResult::Success => Ok(()),
                AuthResult::Failure { .. } => Err(password_failure()),
            }
        }
        SshAuthRequest::KeyboardInteractive => {
            match authenticate_keyboard_interactive(handle, username, context).await? {
                AuthResult::Success => Ok(()),
                AuthResult::Failure { .. } => Err(keyboard_interactive_failure()),
            }
        }
        SshAuthRequest::PublicKey {
            private_key_path,
            key_passphrase,
        } => {
            let key = load_private_key_for_auth(&private_key_path, key_passphrase.as_deref())
                .map_err(|e| format!("SSH public key authentication error: {}", e))?;
            let result = authenticate_public_key(handle, username, key).await?;
            match result {
                AuthResult::Success => Ok(()),
                AuthResult::Failure {
                    remaining_methods,
                    partial_success,
                } if should_continue_public_key(&remaining_methods, partial_success) => {
                    match authenticate_keyboard_interactive(handle, username, context).await? {
                        AuthResult::Success => Ok(()),
                        AuthResult::Failure { .. } => Err(public_key_failure()),
                    }
                }
                AuthResult::Failure { .. } => Err(public_key_failure()),
            }
        }
    }
}

async fn authenticate_auto(
    handle: &mut russh::client::Handle<impl russh::client::Handler + Send + 'static>,
    username: &str,
    private_key_path: Option<String>,
    key_passphrase: Option<String>,
    context: &SshAuthenticationContext<'_>,
    diagnostic: Option<&SshDiagnostic>,
) -> Result<(), String> {
    let initial = authenticate_none(handle, username).await?;
    let mut remaining_methods = match initial {
        AuthResult::Success => return Ok(()),
        AuthResult::Failure {
            remaining_methods, ..
        } => remaining_methods,
    };

    if let Some(private_key_path) = private_key_path {
        if supports(&remaining_methods, MethodKind::PublicKey) {
            match load_private_key_for_auth(&private_key_path, key_passphrase.as_deref()) {
                Ok(key) => match authenticate_public_key(handle, username, key).await? {
                    AuthResult::Success => return Ok(()),
                    AuthResult::Failure {
                        remaining_methods: next_methods,
                        partial_success,
                    } => {
                        automatic_auth_diagnostic(
                            diagnostic,
                            context.phase,
                            if partial_success {
                                "public-key authentication succeeded; additional authentication is required"
                            } else {
                                "public-key authentication was not accepted; trying the next method"
                            },
                        );
                        remaining_methods = next_methods;
                    }
                },
                Err(_) => automatic_auth_diagnostic(
                    diagnostic,
                    context.phase,
                    "the configured private key could not be loaded; trying the next method",
                ),
            }
        }
    } else {
        automatic_auth_diagnostic(
            diagnostic,
            context.phase,
            "public-key authentication skipped because no private key is configured",
        );
    }

    if supports(&remaining_methods, MethodKind::KeyboardInteractive) {
        match authenticate_keyboard_interactive(handle, username, context).await? {
            AuthResult::Success => return Ok(()),
            AuthResult::Failure {
                remaining_methods: next_methods,
                ..
            } => {
                automatic_auth_diagnostic(
                    diagnostic,
                    context.phase,
                    "keyboard-interactive authentication was not accepted; trying the next method",
                );
                remaining_methods = next_methods;
            }
        }
    }

    if supports(&remaining_methods, MethodKind::Password) {
        let password = context.request_password().await?;
        if matches!(
            authenticate_password(handle, username, password).await?,
            AuthResult::Success
        ) {
            return Ok(());
        }
    }

    Err("SSH automatic authentication failed".to_string())
}

fn automatic_auth_diagnostic(diagnostic: Option<&SshDiagnostic>, phase: &str, message: &str) {
    if let Some(diagnostic) = diagnostic {
        diagnostic.info(format!("{phase}: automatic authentication: {message}"));
    }
}

async fn authenticate_none(
    handle: &mut russh::client::Handle<impl russh::client::Handler + Send + 'static>,
    username: &str,
) -> Result<AuthResult, String> {
    run_ssh_operation_with_timeout(SSH_AUTH_TIMEOUT, SSH_AUTH_TIMEOUT_ERROR, async {
        handle
            .authenticate_none(username)
            .await
            .map_err(|error| format!("SSH authentication error: {error}"))
    })
    .await
}

async fn authenticate_public_key(
    handle: &mut russh::client::Handle<impl russh::client::Handler + Send + 'static>,
    username: &str,
    key: PrivateKey,
) -> Result<AuthResult, String> {
    run_ssh_operation_with_timeout(SSH_AUTH_TIMEOUT, SSH_AUTH_TIMEOUT_ERROR, async {
        handle
            .authenticate_publickey(username, PrivateKeyWithHashAlg::new(Arc::new(key), None))
            .await
            .map_err(|error| format!("SSH public key authentication error: {error}"))
    })
    .await
}

fn should_continue_public_key(remaining_methods: &MethodSet, partial_success: bool) -> bool {
    partial_success && supports(remaining_methods, MethodKind::KeyboardInteractive)
}

async fn authenticate_password(
    handle: &mut russh::client::Handle<impl russh::client::Handler + Send + 'static>,
    username: &str,
    password: String,
) -> Result<AuthResult, String> {
    run_ssh_operation_with_timeout(SSH_AUTH_TIMEOUT, SSH_AUTH_TIMEOUT_ERROR, async {
        handle
            .authenticate_password(username, password)
            .await
            .map_err(|error| format!("SSH authentication error: {error}"))
    })
    .await
}

async fn authenticate_keyboard_interactive(
    handle: &mut russh::client::Handle<impl russh::client::Handler + Send + 'static>,
    username: &str,
    context: &SshAuthenticationContext<'_>,
) -> Result<AuthResult, String> {
    let mut response =
        run_ssh_operation_with_timeout(SSH_AUTH_TIMEOUT, SSH_AUTH_TIMEOUT_ERROR, async {
            handle
                .authenticate_keyboard_interactive_start(username, None::<String>)
                .await
                .map_err(|error| format!("SSH keyboard-interactive authentication error: {error}"))
        })
        .await?;

    for _round in 0..MAX_AUTHENTICATION_STAGES {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(AuthResult::Success),
            KeyboardInteractiveAuthResponse::Failure {
                remaining_methods,
                partial_success,
            } => {
                return Ok(AuthResult::Failure {
                    remaining_methods,
                    partial_success,
                });
            }
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => {
                let responses = context
                    .request_keyboard_interactive(name, instructions, prompts)
                    .await?;
                response = run_ssh_operation_with_timeout(
                    SSH_AUTH_TIMEOUT,
                    SSH_AUTH_TIMEOUT_ERROR,
                    async {
                        handle
                            .authenticate_keyboard_interactive_respond(responses)
                            .await
                            .map_err(|error| {
                                format!("SSH keyboard-interactive authentication error: {error}")
                            })
                    },
                )
                .await?;
            }
        }
    }

    Err("SSH authentication failed: too many keyboard-interactive rounds".to_string())
}

fn supports(methods: &MethodSet, method: MethodKind) -> bool {
    methods.contains(&method)
}

fn password_failure() -> String {
    "SSH authentication failed: the username or password is incorrect".to_string()
}

fn keyboard_interactive_failure() -> String {
    "SSH keyboard-interactive authentication failed".to_string()
}

fn public_key_failure() -> String {
    "SSH public key authentication failed: check the username, private key, public key registration, passphrase, or required additional authentication.".to_string()
}

#[cfg(test)]
mod authentication_flow_tests {
    use super::*;

    fn methods(methods: &[MethodKind]) -> MethodSet {
        MethodSet::from(methods)
    }

    #[test]
    fn public_key_only_continues_after_partial_success() {
        let keyboard_interactive = methods(&[MethodKind::KeyboardInteractive]);
        assert!(should_continue_public_key(&keyboard_interactive, true));
        assert!(!should_continue_public_key(&keyboard_interactive, false));
        assert!(!should_continue_public_key(
            &methods(&[MethodKind::Password]),
            true
        ));
    }
}
