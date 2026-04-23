use std::fs;
use std::path::{Path, PathBuf};

use russh_keys::{key::PublicKey, PublicKeyBase64};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostKeyCheckStatus {
    Trusted,
    Unknown,
    Mismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostKeyCheckResult {
    pub status: HostKeyCheckStatus,
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
    pub known_fingerprint: Option<String>,
}

pub fn known_hosts_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ExaTerm")
        .join("known_hosts")
}

pub fn endpoint_cache_key(host: &str, port: u16) -> String {
    format!("{}:{}", host, port)
}

pub fn inspect_host_key_with_path<P: AsRef<Path>>(
    host: &str,
    port: u16,
    key: &PublicKey,
    path: P,
) -> Result<HostKeyCheckResult, String> {
    let lines = read_known_hosts_lines(path.as_ref())?;
    let mut matched_fingerprint = None;
    let mut trusted = false;

    for line in &lines {
        let Some(parsed) = parse_known_host_line(line) else {
            continue;
        };
        if !host_field_matches_endpoint(parsed.host_field, host, port) {
            continue;
        }

        let parsed_key = match russh_keys::parse_public_key_base64(parsed.key_data) {
            Ok(parsed_key) => parsed_key,
            Err(_) => continue,
        };
        let fingerprint = parsed_key.fingerprint();
        if parsed_key == *key {
            trusted = true;
            matched_fingerprint = Some(fingerprint);
            break;
        }
        if matched_fingerprint.is_none() {
            matched_fingerprint = Some(fingerprint);
        }
    }

    let status = if trusted {
        HostKeyCheckStatus::Trusted
    } else if matched_fingerprint.is_some() {
        HostKeyCheckStatus::Mismatch
    } else {
        HostKeyCheckStatus::Unknown
    };

    Ok(HostKeyCheckResult {
        status,
        host: host.to_string(),
        port,
        algorithm: public_key_algorithm_name(key),
        fingerprint: key.fingerprint(),
        known_fingerprint: if trusted { None } else { matched_fingerprint },
    })
}

pub fn write_trusted_host(host: &str, port: u16, key: &PublicKey, replace: bool) -> Result<(), String> {
    write_trusted_host_with_path(host, port, key, replace, &known_hosts_path())
}

pub fn write_trusted_host_with_path<P: AsRef<Path>>(
    host: &str,
    port: u16,
    key: &PublicKey,
    replace: bool,
    path: P,
) -> Result<(), String> {
    let path = path.as_ref();
    let inspection = inspect_host_key_with_path(host, port, key, path)?;
    match inspection.status {
        HostKeyCheckStatus::Trusted => return Ok(()),
        HostKeyCheckStatus::Mismatch if !replace => {
            return Err("保存済みのSSHホスト鍵と一致しません".to_string())
        }
        _ => {}
    }

    let mut lines = read_known_hosts_lines(path)?;
    if replace {
        lines.retain(|line| !line_matches_endpoint(line, host, port));
    }
    lines.push(render_known_host_line(host, port, key));
    write_known_hosts_lines(path, &lines)
}

pub fn public_key_algorithm_name(key: &PublicKey) -> String {
    match key {
        PublicKey::Ed25519(_) => "ssh-ed25519".to_string(),
        PublicKey::RSA { .. } => "ssh-rsa".to_string(),
        PublicKey::EC { key } => key.algorithm().to_string(),
    }
}

fn read_known_hosts_lines(path: &Path) -> Result<Vec<String>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(content.lines().map(ToString::to_string).collect())
}

fn write_known_hosts_lines(path: &Path, lines: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut content = lines.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("known_hosts");
    let temp_path = path.with_file_name(format!("{}.{}.tmp", file_name, Uuid::new_v4()));

    fs::write(&temp_path, content).map_err(|e| e.to_string())?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::rename(&temp_path, path).map_err(|e| e.to_string())
}

fn render_known_host_line(host: &str, port: u16, key: &PublicKey) -> String {
    format!(
        "{} {} {}",
        endpoint_pattern(host, port),
        public_key_algorithm_name(key),
        key.public_key_base64()
    )
}

fn endpoint_pattern(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{}]:{}", host, port)
    }
}

fn line_matches_endpoint(line: &str, host: &str, port: u16) -> bool {
    parse_known_host_line(line)
        .map(|parsed| host_field_matches_endpoint(parsed.host_field, host, port))
        .unwrap_or(false)
}

fn host_field_matches_endpoint(host_field: &str, host: &str, port: u16) -> bool {
    let expected = endpoint_pattern(host, port);
    host_field.split(',').any(|entry| entry == expected)
}

struct ParsedKnownHostLine<'a> {
    host_field: &'a str,
    key_data: &'a str,
}

fn parse_known_host_line(line: &str) -> Option<ParsedKnownHostLine<'_>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let host_field = parts.next()?;
    let _key_type = parts.next()?;
    let key_data = parts.next()?;

    Some(ParsedKnownHostLine {
        host_field,
        key_data,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    fn temp_known_hosts_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("exaterm-known-hosts-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("known_hosts")
    }

    fn read_test_key(base64: &str) -> PublicKey {
        russh_keys::parse_public_key_base64(base64).unwrap()
    }

    fn ed25519_key() -> PublicKey {
        read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAIJdD7y3aLq454yWBdwLWbieU1ebz9/cu7/QEXn9OIeZJ")
    }

    fn other_ed25519_key() -> PublicKey {
        read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAILIG2T/B0l0gaqj3puu510tu9N1OkQ4znY3LYuEm5zCF")
    }

    #[test]
    fn empty_known_hosts_is_unknown() {
        let path = temp_known_hosts_path();
        let result = inspect_host_key_with_path("example.com", 22, &ed25519_key(), &path).unwrap();
        assert_eq!(result.status, HostKeyCheckStatus::Unknown);
    }

    #[test]
    fn matching_key_is_trusted() {
        let path = temp_known_hosts_path();
        let key = ed25519_key();
        fs::write(&path, format!("{}\n", render_known_host_line("example.com", 22, &key))).unwrap();

        let result = inspect_host_key_with_path("example.com", 22, &key, &path).unwrap();
        assert_eq!(result.status, HostKeyCheckStatus::Trusted);
    }

    #[test]
    fn different_key_is_mismatch() {
        let path = temp_known_hosts_path();
        let key = ed25519_key();
        let other = other_ed25519_key();
        fs::write(&path, format!("{}\n", render_known_host_line("example.com", 22, &other))).unwrap();

        let result = inspect_host_key_with_path("example.com", 22, &key, &path).unwrap();
        assert_eq!(result.status, HostKeyCheckStatus::Mismatch);
        assert_eq!(result.known_fingerprint, Some(other.fingerprint()));
    }

    #[test]
    fn different_port_does_not_match() {
        let path = temp_known_hosts_path();
        let key = ed25519_key();
        fs::write(&path, format!("{}\n", render_known_host_line("example.com", 22, &key))).unwrap();

        let result = inspect_host_key_with_path("example.com", 2222, &key, &path).unwrap();
        assert_eq!(result.status, HostKeyCheckStatus::Unknown);
    }

    #[test]
    fn replace_updates_target_and_preserves_other_lines() {
        let path = temp_known_hosts_path();
        let old_key = other_ed25519_key();
        let new_key = ed25519_key();
        let other_host_key = read_test_key("AAAAC3NzaC1lZDI1NTE5AAAAIA6rWI3G1sz07DnfFlrouTcysQlj2P+jpNSOEWD9OJ3X");

        let content = format!(
            "# keep-comment\n{}\n{}\n",
            render_known_host_line("example.com", 22, &old_key),
            render_known_host_line("other.example.com", 22, &other_host_key)
        );
        fs::write(&path, content).unwrap();

        write_trusted_host_with_path("example.com", 22, &new_key, true, &path).unwrap();

        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.contains("# keep-comment"));
        assert!(updated.contains("other.example.com"));
        assert!(updated.contains(&new_key.public_key_base64()));
        assert!(!updated.contains(&old_key.public_key_base64()));
    }
}
