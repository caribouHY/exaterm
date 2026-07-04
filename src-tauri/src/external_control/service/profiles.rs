use serde_json::{json, Value};

use crate::config::{AppConfig, SavedConnection};
use crate::serial;
use crate::ssh;

use super::{
    load_app_config, ConnectSavedProfileArgs, ConnectSerialConsoleArgs,
    ExternalControlConnectionProfile, ExternalControlError, ExternalControlService,
    ListConnectionProfilesArgs, PreparedConnection, PreparedConnectionKind,
    PreparedSerialConnection, SavedProfileConnectionType, DEFAULT_CONNECT_COLS,
    DEFAULT_CONNECT_ROWS, DEFAULT_SERIAL_BAUD_RATE, DEFAULT_SERIAL_DATA_BITS,
    DEFAULT_SERIAL_STOP_BITS, MAX_CONNECT_DIMENSION,
};
impl ExternalControlService {
    pub(crate) async fn list_connection_profiles(
        &self,
        args: ListConnectionProfilesArgs,
    ) -> Result<Value, ExternalControlError> {
        self.ensure_connect_enabled()?;
        let config = load_app_config(&self.runtime)?;
        Ok(json!({
            "profiles": list_connection_profiles_from_config(&config, args.connection_type),
        }))
    }
}

pub(crate) fn normalize_connect_dimension(value: Option<u32>, default_value: u32) -> u32 {
    value
        .unwrap_or(default_value)
        .clamp(1, MAX_CONNECT_DIMENSION)
}

pub(crate) fn normalize_profile_type(connection_type: &str) -> String {
    connection_type.trim().to_ascii_lowercase()
}

pub(crate) fn normalize_profile_host(profile: &SavedConnection) -> String {
    profile
        .host
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn normalize_profile_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn profile_external_control_enabled(profile: &SavedConnection) -> bool {
    profile.external_control_enabled
}

pub(crate) fn normalize_profile_encoding(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("shift-jis") => "shift-jis".into(),
        Some("euc-jp") => "euc-jp".into(),
        _ => "utf-8".into(),
    }
}

pub(crate) fn normalize_profile_terminal_mode(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some("cisco_ios") => "cisco_ios".into(),
        _ => "general".into(),
    }
}

pub(crate) fn normalize_profile_auth_method(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("password") => Ok("password".into()),
        Some("public_key") => Ok("public_key".into()),
        Some(_) => Err("SSH認証方式が不正です".into()),
    }
}

pub(crate) fn ssh_credential_required(
    auth_method: &str,
    private_key_path: Option<&str>,
) -> Result<bool, String> {
    match auth_method {
        "password" => Ok(true),
        "public_key" => {
            let private_key_path = private_key_path
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "保存済みSSHプロファイルに秘密鍵ファイルが設定されていません".to_string()
                })?;
            ssh::private_key_requires_passphrase(private_key_path)
        }
        _ => Err("SSH認証方式が不正です".into()),
    }
}

pub(crate) fn available_serial_port_names(ports: &[serial::PortInfo]) -> String {
    if ports.is_empty() {
        "なし".into()
    } else {
        ports
            .iter()
            .map(|port| port.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(crate) fn normalize_serial_data_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_DATA_BITS);
    match value {
        5 | 6 | 7 | 8 => Ok(value),
        _ => Err("data_bits は 5, 6, 7, 8 のいずれかを指定してください".into()),
    }
}

pub(crate) fn normalize_serial_stop_bits(value: Option<u8>) -> Result<u8, String> {
    let value = value.unwrap_or(DEFAULT_SERIAL_STOP_BITS);
    match value {
        1 | 2 => Ok(value),
        _ => Err("stop_bits は 1 または 2 を指定してください".into()),
    }
}

pub(crate) fn prepare_serial_console_connection(
    args: ConnectSerialConsoleArgs,
    available_ports: &[serial::PortInfo],
) -> Result<PreparedSerialConnection, String> {
    let port = args.port.trim().to_string();
    if port.is_empty() {
        return Err("port を指定してください".into());
    }
    if !available_ports
        .iter()
        .any(|available| available.name == port)
    {
        return Err(format!(
            "指定されたシリアルポートが見つかりません: {port}。利用可能: {}",
            available_serial_port_names(available_ports)
        ));
    }

    let baud_rate = args.baud_rate.unwrap_or(DEFAULT_SERIAL_BAUD_RATE);
    if baud_rate == 0 {
        return Err("baud_rate は 1 以上で指定してください".into());
    }

    let terminal_mode = args.terminal_mode.unwrap_or_default().as_str().to_string();
    let _cols = normalize_connect_dimension(args.cols, DEFAULT_CONNECT_COLS);
    let _rows = normalize_connect_dimension(args.rows, DEFAULT_CONNECT_ROWS);

    Ok(PreparedSerialConnection {
        port: port.clone(),
        config: serial::SerialConfig {
            baud_rate,
            data_bits: normalize_serial_data_bits(args.data_bits)?,
            parity: args.parity.unwrap_or_default().as_str().to_string(),
            stop_bits: normalize_serial_stop_bits(args.stop_bits)?,
            flow_control: args.flow_control.unwrap_or_default().as_str().to_string(),
        },
        target: port.clone(),
        title: port,
        encoding: "utf-8".into(),
        terminal_mode,
    })
}

pub(crate) fn list_connection_profiles_from_config(
    config: &AppConfig,
    connection_type_filter: Option<SavedProfileConnectionType>,
) -> Vec<ExternalControlConnectionProfile> {
    config
        .saved_connections
        .iter()
        .filter_map(|profile| {
            if !profile_external_control_enabled(profile) {
                return None;
            }
            let connection_type = normalize_profile_type(&profile.connection_type);
            if connection_type_filter_mismatch(&connection_type, connection_type_filter.as_ref()) {
                return None;
            }
            let memo = normalize_profile_string(profile.memo.as_deref());
            match connection_type.as_str() {
                "ssh" => Some(ExternalControlConnectionProfile {
                    id: profile.id.clone(),
                    connection_type,
                    host: normalize_profile_host(profile),
                    port: profile.port.unwrap_or(22),
                    username: normalize_profile_string(profile.username.as_deref()),
                    auth_method: Some(
                        profile
                            .auth_method
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .unwrap_or("password")
                            .to_string(),
                    ),
                    encoding: Some(normalize_profile_encoding(profile.encoding.as_deref())),
                    terminal_mode: Some(normalize_profile_terminal_mode(
                        profile.terminal_mode.as_deref(),
                    )),
                    private_key_configured: Some(
                        profile
                            .private_key_path
                            .as_deref()
                            .map(str::trim)
                            .is_some_and(|value| !value.is_empty()),
                    ),
                    jump_profile_id: normalize_profile_string(profile.jump_profile_id.as_deref()),
                    memo,
                }),
                "telnet" => Some(ExternalControlConnectionProfile {
                    id: profile.id.clone(),
                    connection_type,
                    host: normalize_profile_host(profile),
                    port: profile.port.unwrap_or(23),
                    username: None,
                    auth_method: None,
                    encoding: Some(normalize_profile_encoding(profile.encoding.as_deref())),
                    terminal_mode: Some(normalize_profile_terminal_mode(
                        profile.terminal_mode.as_deref(),
                    )),
                    private_key_configured: None,
                    jump_profile_id: None,
                    memo,
                }),
                _ => None,
            }
        })
        .collect()
}

fn connection_type_filter_mismatch(
    profile_type: &str,
    filter: Option<&SavedProfileConnectionType>,
) -> bool {
    filter.is_some_and(|filter| profile_type != filter.as_str())
}

pub(crate) fn prepare_saved_profile_connection(
    config: &AppConfig,
    args: ConnectSavedProfileArgs,
) -> Result<PreparedConnection, String> {
    let profile_id = args.profile_id.trim();
    if profile_id.is_empty() {
        return Err("profile_id を指定してください".into());
    }

    let profile = find_saved_profile(config, profile_id, &args.connection_type)?;
    if !profile_external_control_enabled(profile) {
        return Err("この保存済みプロファイルは外部制御からの利用が無効です".into());
    }

    let metadata = prepare_profile_metadata(profile, &args)?;
    match metadata.connection_type.as_str() {
        "ssh" => prepare_ssh_profile_connection(config, profile, metadata),
        "telnet" => prepare_telnet_profile_connection(profile, metadata),
        _ => Err("外部制御の新規接続は保存済みSSH/Telnetプロファイルのみ対応しています".into()),
    }
}

fn find_saved_profile<'a>(
    config: &'a AppConfig,
    profile_id: &str,
    connection_type: &SavedProfileConnectionType,
) -> Result<&'a SavedConnection, String> {
    config
        .saved_connections
        .iter()
        .find(|profile| {
            profile.id == profile_id
                && normalize_profile_type(&profile.connection_type) == connection_type.as_str()
        })
        .ok_or_else(|| "保存済みプロファイルが見つかりません".to_string())
}

#[derive(Debug, Clone)]
struct PreparedProfileMetadata {
    profile_id: String,
    connection_type: String,
    host: String,
    encoding: String,
    terminal_mode: String,
    cols: u32,
    rows: u32,
}

fn prepare_profile_metadata(
    profile: &SavedConnection,
    args: &ConnectSavedProfileArgs,
) -> Result<PreparedProfileMetadata, String> {
    let connection_type = normalize_profile_type(&profile.connection_type);
    let host = normalize_profile_host(profile);
    if host.is_empty() {
        return Err("保存済みプロファイルにホストが設定されていません".into());
    }

    Ok(PreparedProfileMetadata {
        profile_id: profile.id.clone(),
        connection_type,
        host,
        encoding: normalize_profile_encoding(profile.encoding.as_deref()),
        terminal_mode: normalize_profile_terminal_mode(profile.terminal_mode.as_deref()),
        cols: normalize_connect_dimension(args.cols, DEFAULT_CONNECT_COLS),
        rows: normalize_connect_dimension(args.rows, DEFAULT_CONNECT_ROWS),
    })
}

fn prepare_ssh_profile_connection(
    config: &AppConfig,
    profile: &SavedConnection,
    metadata: PreparedProfileMetadata,
) -> Result<PreparedConnection, String> {
    let username = normalize_profile_string(profile.username.as_deref())
        .ok_or_else(|| "保存済みSSHプロファイルにユーザー名が設定されていません".to_string())?;
    let auth_method = normalize_profile_auth_method(profile.auth_method.as_deref())?;
    let private_key_path = normalize_profile_string(profile.private_key_path.as_deref());
    if auth_method == "public_key" && private_key_path.is_none() {
        return Err("保存済みSSHプロファイルに秘密鍵ファイルが設定されていません".to_string());
    }

    let port = profile.port.unwrap_or(22);
    Ok(PreparedConnection {
        kind: PreparedConnectionKind::Ssh {
            host: metadata.host.clone(),
            port,
            username: username.clone(),
            auth_method,
            private_key_path,
            jump_profile: ssh::resolve_jump_profile(
                config,
                profile.jump_profile_id.as_deref(),
                Some(profile.id.as_str()),
            )?,
        },
        profile_id: metadata.profile_id,
        connection_type: metadata.connection_type,
        target: format!("{}@{}:{}", username, metadata.host, port),
        title: format!("{}@{}", username, metadata.host),
        encoding: metadata.encoding,
        terminal_mode: metadata.terminal_mode,
        cols: metadata.cols,
        rows: metadata.rows,
    })
}

fn prepare_telnet_profile_connection(
    profile: &SavedConnection,
    metadata: PreparedProfileMetadata,
) -> Result<PreparedConnection, String> {
    let port = profile.port.unwrap_or(23);
    Ok(PreparedConnection {
        kind: PreparedConnectionKind::Telnet {
            host: metadata.host.clone(),
            port,
        },
        profile_id: metadata.profile_id,
        connection_type: metadata.connection_type,
        target: format!("{}:{}", metadata.host, port),
        title: format!("{}:{}", metadata.host, port),
        encoding: metadata.encoding,
        terminal_mode: metadata.terminal_mode,
        cols: metadata.cols,
        rows: metadata.rows,
    })
}
