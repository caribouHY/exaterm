use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConnectResult {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectOptions {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub auth_method: Option<String>,
    pub private_key_path: Option<String>,
    pub key_passphrase: Option<String>,
    pub jump_profile_id: Option<String>,
    pub jump_password: Option<String>,
    pub jump_key_passphrase: Option<String>,
    pub cols: u32,
    pub rows: u32,
    pub encoding: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshJumpProfile {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub private_key_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SshAuthRequest {
    Auto {
        private_key_path: Option<String>,
        key_passphrase: Option<String>,
    },
    Password {
        password: String,
    },
    KeyboardInteractive,
    PublicKey {
        private_key_path: String,
        key_passphrase: Option<String>,
    },
}
