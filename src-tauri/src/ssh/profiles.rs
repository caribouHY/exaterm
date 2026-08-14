use crate::config::{AppConfig, SavedConnection};
use crate::ssh::types::SshJumpProfile;

pub(super) fn normalize_profile_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn normalize_profile_auth_method(value: Option<&str>) -> Result<String, String> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("auto") => Ok("auto".into()),
        Some("password") => Ok("password".into()),
        Some("keyboard_interactive") => Ok("keyboard_interactive".into()),
        Some("public_key") => Ok("public_key".into()),
        Some(_) => Err("The SSH authentication method is invalid".into()),
    }
}

fn normalize_connection_type(profile: &SavedConnection) -> String {
    profile.connection_type.trim().to_ascii_lowercase()
}

pub fn resolve_jump_profile(
    config: &AppConfig,
    jump_profile_id: Option<&str>,
    target_profile_id: Option<&str>,
) -> Result<Option<SshJumpProfile>, String> {
    let Some(jump_profile_id) = normalize_profile_string(jump_profile_id) else {
        return Ok(None);
    };
    reject_self_referencing_jump(&jump_profile_id, target_profile_id)?;
    let profile = find_saved_jump_profile(config, &jump_profile_id)?;
    validate_jump_profile(profile)?;
    Ok(Some(build_jump_profile(
        profile,
        &config.ssh.default_private_key_path,
    )?))
}

fn reject_self_referencing_jump(
    jump_profile_id: &str,
    target_profile_id: Option<&str>,
) -> Result<(), String> {
    if normalize_profile_string(target_profile_id).as_deref() == Some(jump_profile_id) {
        return Err("An SSH jump profile cannot reference itself".into());
    }
    Ok(())
}

fn find_saved_jump_profile<'a>(
    config: &'a AppConfig,
    jump_profile_id: &str,
) -> Result<&'a SavedConnection, String> {
    config
        .saved_connections
        .iter()
        .find(|profile| profile.id == jump_profile_id)
        .ok_or_else(|| "SSH jump profile not found".to_string())
}

fn validate_jump_profile(profile: &SavedConnection) -> Result<(), String> {
    if normalize_connection_type(profile) != "ssh" {
        return Err("Only SSH profiles can be used as SSH jump profiles".into());
    }
    if normalize_profile_string(profile.jump_profile_id.as_deref()).is_some() {
        return Err("Nested SSH jump profiles are not supported".into());
    }
    Ok(())
}

fn build_jump_profile(
    profile: &SavedConnection,
    default_private_key_path: &str,
) -> Result<SshJumpProfile, String> {
    let host = normalize_profile_string(profile.host.as_deref())
        .ok_or_else(|| "The SSH jump profile does not have a host configured".to_string())?;
    let username = normalize_profile_string(profile.username.as_deref())
        .ok_or_else(|| "The SSH jump profile does not have a username configured".to_string())?;
    let auth_method = normalize_profile_auth_method(profile.auth_method.as_deref())?;
    let mut private_key_path = normalize_profile_string(profile.private_key_path.as_deref());
    if auth_method == "public_key" && private_key_path.is_none() {
        return Err("The SSH jump profile does not have a private key file configured".to_string());
    }
    if auth_method == "auto" && private_key_path.is_none() {
        private_key_path = normalize_profile_string(Some(default_private_key_path));
    }
    Ok(SshJumpProfile {
        id: profile.id.clone(),
        host,
        port: profile.port.unwrap_or(22),
        username,
        auth_method,
        private_key_path,
    })
}
