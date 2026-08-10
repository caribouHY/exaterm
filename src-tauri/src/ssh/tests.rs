use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use russh::keys::PublicKey;
use uuid::Uuid;

use super::auth::{
    build_auth_request, is_supported_private_key_header, normalize_auth_path,
    private_key_format_hint, SUPPORTED_PRIVATE_KEY_LABELS,
};
use super::client_config::{algorithm_catalog, build_client_config, validate_algorithm_config};
use super::diagnostics::ssh_diagnostic_event_name;
use super::host_key::{HostKeyHandling, HostKeyVerifier};
use super::io::{
    record_ssh_read_drop, run_ssh_channel_operation_with_timeout, run_ssh_operation_with_timeout,
    ssh_read_overflow_event_name, SshReadDropNotice, SshReadDropState, SSH_CONNECT_TIMEOUT_ERROR,
    SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS, SSH_WRITE_ERROR, SSH_WRITE_TIMEOUT_ERROR,
};
use super::private_key_requires_passphrase;
use super::types::SshAuthRequest;
use crate::config::{SshAlgorithmSelection, SshConfig};
use crate::ssh_known_hosts::HostKeyCheckStatus;

fn preferred_names<T: AsRef<str>>(items: &[T]) -> Vec<&str> {
    items.iter().map(|item| item.as_ref()).collect()
}

fn temp_known_hosts_path() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exaterm-ssh-verifier-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir.join("known_hosts")
}

fn read_test_key(base64: &str) -> PublicKey {
    russh::keys::parse_public_key_base64(base64).unwrap()
}

fn write_temp_private_key(contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exaterm-ssh-key-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("id_ed25519");
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn diagnostic_event_name_scopes_to_request_id() {
    assert_eq!(
        ssh_diagnostic_event_name("request-1"),
        "ssh://connect-diagnostic/request-1"
    );
}

#[test]
fn gui_and_external_control_use_distinct_host_key_handling() {
    assert_ne!(HostKeyHandling::Prompt, HostKeyHandling::RequireTrusted);
}

#[test]
fn read_overflow_event_name_scopes_to_session_id() {
    assert_eq!(
        ssh_read_overflow_event_name("session-1"),
        "ssh://read-overflow/session-1"
    );
}

#[test]
fn read_drop_state_reports_first_drop_only_until_interval() {
    let state = Arc::new(StdMutex::new(SshReadDropState::default()));

    assert_eq!(
        record_ssh_read_drop(&state, 32),
        Some(SshReadDropNotice {
            dropped_chunks: 1,
            dropped_bytes: 32,
        })
    );
    assert_eq!(record_ssh_read_drop(&state, 64), None);

    let state = state.lock().unwrap();
    assert_eq!(state.dropped_chunks, 2);
    assert_eq!(state.dropped_bytes, 96);
}

#[test]
fn read_drop_state_reports_every_notice_interval() {
    let state = Arc::new(StdMutex::new(SshReadDropState::default()));
    let mut last_notice = None;

    for _ in 0..SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS {
        last_notice = record_ssh_read_drop(&state, 1);
    }

    assert_eq!(
        last_notice,
        Some(SshReadDropNotice {
            dropped_chunks: SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS,
            dropped_bytes: SSH_READ_DROP_NOTICE_INTERVAL_CHUNKS,
        })
    );
}

#[tokio::test]
async fn channel_operation_timeout_returns_fixed_error() {
    let error = run_ssh_channel_operation_with_timeout(
        Duration::from_millis(1),
        SSH_WRITE_TIMEOUT_ERROR,
        std::future::pending::<Result<(), String>>(),
    )
    .await
    .unwrap_err();

    assert_eq!(error, SSH_WRITE_TIMEOUT_ERROR);
    assert!(!error.contains("secret"));
    assert!(!error.contains("payload"));
}

#[tokio::test]
async fn channel_operation_timeout_preserves_operation_result() {
    let success = run_ssh_channel_operation_with_timeout(
        Duration::from_secs(1),
        SSH_WRITE_TIMEOUT_ERROR,
        async { Ok(()) },
    )
    .await;
    assert_eq!(success, Ok(()));

    let error = run_ssh_channel_operation_with_timeout(
        Duration::from_secs(1),
        SSH_WRITE_TIMEOUT_ERROR,
        async { Err(SSH_WRITE_ERROR.to_string()) },
    )
    .await
    .unwrap_err();
    assert_eq!(error, SSH_WRITE_ERROR);
}

#[tokio::test]
async fn ssh_operation_timeout_returns_fixed_error_without_sensitive_context() {
    let error = run_ssh_operation_with_timeout(
        Duration::from_millis(1),
        SSH_CONNECT_TIMEOUT_ERROR,
        std::future::pending::<Result<&'static str, String>>(),
    )
    .await
    .unwrap_err();

    assert_eq!(error, SSH_CONNECT_TIMEOUT_ERROR);
    assert!(!error.contains("secret"));
    assert!(!error.contains("payload"));
    assert!(!error.contains("host.example.com"));
    assert!(!error.contains("admin"));
}

#[tokio::test]
async fn ssh_operation_timeout_preserves_success_value() {
    let value =
        run_ssh_operation_with_timeout(Duration::from_secs(1), SSH_CONNECT_TIMEOUT_ERROR, async {
            Ok("connected")
        })
        .await
        .unwrap();

    assert_eq!(value, "connected");
}

fn generate_temp_private_key(passphrase: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exaterm-ssh-keygen-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("id_ed25519");
    let status = Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", passphrase, "-f"])
        .arg(&path)
        .arg("-q")
        .status()
        .unwrap_or_else(|error| panic!("ssh-keygen is required for this test: {error}"));
    assert!(status.success(), "ssh-keygen failed with status {status}");
    path
}

#[test]
fn verifier_rejects_unknown_keys() {
    let verifier =
        HostKeyVerifier::with_path("example.com".to_string(), 22, temp_known_hosts_path());
    let key = read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ");

    let allowed = verifier.check_key(&key).unwrap();
    assert!(!allowed);
    assert_eq!(
        verifier.last_result().unwrap().status,
        HostKeyCheckStatus::Unknown
    );
}

#[test]
fn verifier_rejects_mismatched_keys() {
    let known_hosts_path = temp_known_hosts_path();
    let stored_key =
        read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAILIG2T/B0l0gaqj3puu510tu9N1OkQ4znY3LYuEm5zCF");
    crate::ssh_known_hosts::write_trusted_host_with_path(
        "example.com",
        22,
        &stored_key,
        false,
        &known_hosts_path,
    )
    .unwrap();

    let verifier =
        HostKeyVerifier::with_path("example.com".to_string(), 22, known_hosts_path.clone());
    let new_key =
        read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ");

    let allowed = verifier.check_key(&new_key).unwrap();
    assert!(!allowed);
    assert_eq!(
        verifier.last_result().unwrap().status,
        HostKeyCheckStatus::Mismatch
    );

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn verifier_accepts_trusted_key_without_prompt() {
    let known_hosts_path = temp_known_hosts_path();
    let key = read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ");
    crate::ssh_known_hosts::write_trusted_host_with_path(
        "example.com",
        22,
        &key,
        false,
        &known_hosts_path,
    )
    .unwrap();
    let verifier =
        HostKeyVerifier::with_path("example.com".to_string(), 22, known_hosts_path.clone());

    assert!(verifier.check_key(&key).unwrap());
    assert_eq!(
        verifier.last_result().unwrap().status,
        HostKeyCheckStatus::Trusted
    );
    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn default_client_config_keeps_russh_defaults() {
    let config = build_client_config(&SshConfig::default()).unwrap();
    let default = russh::client::Config::default();

    assert_eq!(
        preferred_names(config.preferred.kex.as_ref()),
        preferred_names(default.preferred.kex.as_ref())
    );
    assert_eq!(
        preferred_names(config.preferred.cipher.as_ref()),
        preferred_names(default.preferred.cipher.as_ref())
    );
    assert_eq!(
        preferred_names(config.preferred.mac.as_ref()),
        preferred_names(default.preferred.mac.as_ref())
    );
    assert_eq!(
        preferred_names(config.preferred.key.as_ref()),
        preferred_names(default.preferred.key.as_ref())
    );
}

#[test]
fn legacy_client_config_appends_legacy_algorithms() {
    let config = build_client_config(&SshConfig {
        algorithm_mode: "custom".into(),
        algorithms: crate::ssh::legacy_algorithm_selection(),
    })
    .unwrap();

    let kex = preferred_names(config.preferred.kex.as_ref());
    let cipher = preferred_names(config.preferred.cipher.as_ref());
    let mac = preferred_names(config.preferred.mac.as_ref());
    let key = preferred_names(config.preferred.key.as_ref());

    assert!(kex.contains(&"diffie-hellman-group1-sha1"));
    assert!(kex.contains(&"diffie-hellman-group14-sha1"));
    assert!(cipher.contains(&"aes128-cbc"));
    assert!(cipher.contains(&"aes192-cbc"));
    assert!(cipher.contains(&"aes256-cbc"));
    assert!(cipher.contains(&"3des-cbc"));
    assert!(mac.contains(&"hmac-sha1"));
    assert!(mac.contains(&"hmac-sha1-etm@openssh.com"));
    assert!(key.contains(&"ssh-rsa"));
}

#[test]
fn algorithm_catalog_excludes_internal_algorithms_and_marks_defaults() {
    let catalog = algorithm_catalog();

    assert!(!catalog.kex.iter().any(|item| item.name == "none"));
    assert!(!catalog.cipher.iter().any(|item| item.name == "none"));
    assert!(!catalog.cipher.iter().any(|item| item.name == "clear"));
    assert!(!catalog
        .kex
        .iter()
        .any(|item| item.name.starts_with("ext-info-") || item.name.starts_with("kex-strict-")));
    assert!(catalog.kex.iter().any(|item| item.recommended));
    assert!(catalog.host_key.iter().any(|item| item.recommended));
    assert!(catalog.cipher.iter().any(|item| item.recommended));
    assert!(catalog.mac.iter().any(|item| item.recommended));
    assert!(catalog.compression.iter().any(|item| item.recommended));
}

fn minimal_custom_config() -> SshConfig {
    SshConfig {
        algorithm_mode: "custom".into(),
        algorithms: SshAlgorithmSelection {
            kex: vec!["curve25519-sha256".into()],
            host_key: vec!["ssh-ed25519".into()],
            cipher: vec!["aes128-ctr".into()],
            mac: vec!["hmac-sha2-256".into()],
            compression: vec!["none".into()],
        },
    }
}

#[test]
fn custom_client_config_uses_catalog_order_and_keeps_kex_extensions() {
    let mut ssh_config = minimal_custom_config();
    ssh_config.algorithms.cipher = vec!["aes128-ctr".into(), "aes256-ctr".into()];

    let config = build_client_config(&ssh_config).unwrap();
    let ciphers = preferred_names(config.preferred.cipher.as_ref());
    let kex = preferred_names(config.preferred.kex.as_ref());

    assert_eq!(ciphers, vec!["aes256-ctr", "aes128-ctr"]);
    assert!(kex.contains(&"ext-info-c"));
    assert!(kex.contains(&"kex-strict-c-v00@openssh.com"));
}

#[test]
fn custom_algorithm_validation_rejects_empty_unknown_and_duplicate_values() {
    let mut ssh_config = minimal_custom_config();
    ssh_config.algorithms.mac.clear();
    assert!(validate_algorithm_config(&ssh_config)
        .unwrap_err()
        .contains("MAC must not be empty"));

    ssh_config = minimal_custom_config();
    ssh_config.algorithms.cipher = vec!["unknown-cipher".into()];
    assert!(validate_algorithm_config(&ssh_config)
        .unwrap_err()
        .contains("Unsupported SSH cipher algorithm"));

    ssh_config = minimal_custom_config();
    ssh_config.algorithms.kex.push("curve25519-sha256".into());
    assert!(validate_algorithm_config(&ssh_config)
        .unwrap_err()
        .contains("Duplicate SSH key exchange algorithm"));
}

#[test]
fn auth_request_defaults_to_password() {
    let request = build_auth_request(None, "secret".to_string(), None, None).unwrap();

    assert_eq!(
        request,
        SshAuthRequest::Password {
            password: "secret".to_string()
        }
    );
}

#[test]
fn public_key_auth_requires_private_key_path() {
    let error = build_auth_request(
        Some("public_key".to_string()),
        String::new(),
        Some("  ".to_string()),
        None,
    )
    .unwrap_err();

    assert!(error.contains("private key file"));
}

#[test]
fn public_key_auth_trims_path_and_empty_passphrase() {
    let request = build_auth_request(
        Some("public_key".to_string()),
        String::new(),
        Some(" C:\\Users\\me\\.ssh\\id_ed25519 ".to_string()),
        Some("  ".to_string()),
    )
    .unwrap();

    assert_eq!(
        request,
        SshAuthRequest::PublicKey {
            private_key_path: "C:\\Users\\me\\.ssh\\id_ed25519".to_string(),
            key_passphrase: None,
        }
    );
}

#[test]
fn auth_path_strips_quotes() {
    assert_eq!(
        normalize_auth_path("\"C:\\Users\\me\\.ssh\\id_ed25519\""),
        PathBuf::from("C:\\Users\\me\\.ssh\\id_ed25519")
    );
}

#[test]
fn private_key_hint_rejects_public_key_files() {
    let error =
        private_key_format_hint("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEexample user@example\n")
            .unwrap_err();

    assert!(error.contains("public key file"));
}

#[test]
fn private_key_hint_rejects_putty_keys() {
    let error = private_key_format_hint("PuTTY-User-Key-File-3: ssh-ed25519\n").unwrap_err();

    assert!(error.contains("PuTTY-format"));
}

#[test]
fn private_key_hint_accepts_supported_pem_headers() {
    for label in SUPPORTED_PRIVATE_KEY_LABELS {
        let secret = format!(
            "-----BEGIN {}-----\nexample\n-----END {}-----\n",
            label, label
        );

        assert!(
            private_key_format_hint(&secret).is_ok(),
            "expected supported key header for {label}"
        );
    }
}

#[test]
fn private_key_header_requires_exact_pem_boundaries() {
    assert!(!is_supported_private_key_header(
        "----BEGIN OPENSSH PRIVATE KEY-----"
    ));
    assert!(!is_supported_private_key_header(
        "-----BEGIN OPENSSH PRIVATE KEY----"
    ));
    assert!(!is_supported_private_key_header(
        "-----BEGIN OPENSSH PRIVATE KEY----- extra"
    ));
    assert!(!is_supported_private_key_header(
        "-----BEGIN CERTIFICATE-----"
    ));
}

#[test]
fn private_key_requires_passphrase_detects_unencrypted_keys() {
    let path = generate_temp_private_key("");

    let requires_passphrase = private_key_requires_passphrase(path.to_str().unwrap()).unwrap();

    assert!(!requires_passphrase);
}

#[test]
fn private_key_requires_passphrase_detects_encrypted_keys() {
    let path = generate_temp_private_key("secret");

    let requires_passphrase = private_key_requires_passphrase(path.to_str().unwrap()).unwrap();

    assert!(requires_passphrase);
}

#[test]
fn private_key_requires_passphrase_rejects_public_key_files() {
    let path =
        write_temp_private_key("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEexample user@example\n");

    let error = private_key_requires_passphrase(path.to_str().unwrap()).unwrap_err();

    assert!(error.contains("public key file"));
}

#[test]
fn private_key_requires_passphrase_rejects_empty_path() {
    let error = private_key_requires_passphrase("  ").unwrap_err();

    assert!(error.contains("private key file"));
}
