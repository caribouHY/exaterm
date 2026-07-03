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
                return Err("SSH公開鍵認証エラー: 秘密鍵ファイルを指定してください".to_string());
            }

            let key_passphrase = key_passphrase
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            Ok(SshAuthRequest::PublicKey {
                private_key_path,
                key_passphrase,
            })
        }
        _ => Err("SSH認証方式が不正です".to_string()),
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
            "秘密鍵ファイルではなく公開鍵ファイルが指定されています。秘密鍵本体を指定してください"
                .to_string(),
        ),
        Some(line) if line.starts_with("PuTTY-User-Key-File-") => Err(
            "PuTTY形式(.ppk)の秘密鍵は直接読み込めません。OpenSSH形式の秘密鍵に変換してください"
                .to_string(),
        ),
        Some(line)
            if line.starts_with("-----BEGIN ")
                && line.contains("PUBLIC KEY")
                && !line.contains("PRIVATE KEY") =>
        {
            Err(
                "秘密鍵ファイルではなく公開鍵ファイルが指定されています。秘密鍵本体を指定してください"
                    .to_string(),
            )
        }
        Some(line) if is_supported_private_key_header(line) => Ok(()),
        _ => Err(
            "OpenSSH/PEM形式の秘密鍵ファイルを指定してください。公開鍵ファイルは秘密鍵として使用できません"
                .to_string(),
        ),
    }
}

fn read_private_key_secret(path: &str) -> Result<(PathBuf, String), String> {
    let path = normalize_auth_path(path);
    let secret = fs::read_to_string(&path).map_err(|error| {
        format!(
            "秘密鍵ファイルを開けません: {} ({})",
            path.to_string_lossy(),
            error
        )
    })?;

    Ok((path, secret))
}

pub fn private_key_requires_passphrase(path: &str) -> Result<bool, String> {
    if path.trim().is_empty() {
        return Err("秘密鍵ファイルを指定してください".to_string());
    }

    let (_path, secret) = read_private_key_secret(path)?;
    private_key_format_hint(&secret)?;
    match decode_secret_key(&secret, None) {
        Ok(_) => Ok(false),
        Err(russh::keys::Error::KeyIsEncrypted) => Ok(true),
        Err(russh::keys::Error::CouldNotReadKey) => Err(
            "秘密鍵を読み込めません。鍵形式、パスフレーズ、またはファイル内容を確認してください"
                .to_string(),
        ),
        Err(other) => Err(format!("秘密鍵を読み込めません: {}", other)),
    }
}

pub fn ssh_private_key_requires_passphrase(private_key_path: String) -> Result<bool, String> {
    private_key_requires_passphrase(&private_key_path)
        .map_err(|error| format!("SSH公開鍵認証エラー: {}", error))
}

fn load_private_key_for_auth(path: &str, passphrase: Option<&str>) -> Result<PrivateKey, String> {
    let (_path, secret) = read_private_key_secret(path)?;
    private_key_format_hint(&secret)?;
    decode_secret_key(&secret, passphrase).map_err(|error| match error {
        russh::keys::Error::KeyIsEncrypted => {
            "秘密鍵はパスフレーズで暗号化されています。鍵パスフレーズを入力してください".to_string()
        }
        russh::keys::Error::CouldNotReadKey => {
            "秘密鍵を読み込めません。鍵形式、パスフレーズ、またはファイル内容を確認してください"
                .to_string()
        }
        other => format!("秘密鍵を読み込めません: {}", other),
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
                .map_err(|e| format!("SSH認証エラー: {}", e))?,
            "SSH認証失敗: ユーザー名またはパスワードが正しくありません",
        ),
        SshAuthRequest::PublicKey {
            private_key_path,
            key_passphrase,
        } => {
            let key = load_private_key_for_auth(&private_key_path, key_passphrase.as_deref())
                .map_err(|e| format!("SSH公開鍵認証エラー: {}", e))?;

            (
                handle
                    .authenticate_publickey(
                        username,
                        PrivateKeyWithHashAlg::new(Arc::new(key), None),
                    )
                    .await
                    .map_err(|e| format!("SSH公開鍵認証エラー: {}", e))?,
                "SSH公開鍵認証失敗: ユーザー名、秘密鍵、公開鍵の登録状態、またはパスフレーズを確認してください",
            )
        }
    };

    if !auth_result.success() {
        return Err(failure_message.to_string());
    }

    Ok(())
}
