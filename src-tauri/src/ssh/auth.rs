use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use russh::keys::decode_secret_key;
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::PrivateKey;

use crate::ssh::types::SshAuthRequest;

pub(super) fn build_auth_request(
    auth_method: Option<String>,
    password: String,
    private_key_path: Option<String>,
    key_passphrase: Option<String>,
) -> Result<SshAuthRequest, String> {
    let method = auth_method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("password");

    match method {
        "password" => Ok(SshAuthRequest::Password { password }),
        "public_key" => {
            let private_key_path = private_key_path.unwrap_or_default().trim().to_string();
            if private_key_path.is_empty() {
                return Err(
                    "SSH public key authentication error: specify a private key file".to_string(),
                );
            }

            let key_passphrase = key_passphrase
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            Ok(SshAuthRequest::PublicKey {
                private_key_path,
                key_passphrase,
            })
        }
        _ => Err("The SSH authentication method is invalid".to_string()),
    }
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
) -> Result<(), String> {
    let (auth_result, failure_message) = match auth {
        SshAuthRequest::Password { password } => (
            handle
                .authenticate_password(username, &password)
                .await
                .map_err(|e| format!("SSH authentication error: {}", e))?,
            "SSH authentication failed: the username or password is incorrect",
        ),
        SshAuthRequest::PublicKey {
            private_key_path,
            key_passphrase,
        } => {
            let key = load_private_key_for_auth(&private_key_path, key_passphrase.as_deref())
                .map_err(|e| format!("SSH public key authentication error: {}", e))?;

            (
                handle
                    .authenticate_publickey(
                        username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), None),
                    )
                    .await
                    .map_err(|e| format!("SSH public key authentication error: {}", e))?,
                "SSH public key authentication failed: check the username, private key, public key registration, or passphrase.",
            )
        }
    };

    if !auth_result.success() {
        return Err(failure_message.to_string());
    }

    Ok(())
}
