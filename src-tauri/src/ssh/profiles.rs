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
        None | Some("password") => Ok("password".into()),
        Some("public_key") => Ok("public_key".into()),
        Some(_) => Err("SSH認証方式が不正です".into()),
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
    Ok(Some(build_jump_profile(profile)?))
}

fn reject_self_referencing_jump(
    jump_profile_id: &str,
    target_profile_id: Option<&str>,
) -> Result<(), String> {
    if normalize_profile_string(target_profile_id).as_deref() == Some(jump_profile_id) {
        return Err("SSH踏み台プロファイルに自分自身は指定できません".into());
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
        .ok_or_else(|| "SSH踏み台プロファイルが見つかりません".to_string())
}

fn validate_jump_profile(profile: &SavedConnection) -> Result<(), String> {
    if normalize_connection_type(profile) != "ssh" {
        return Err("SSH踏み台にはSSHプロファイルのみ指定できます".into());
    }
    if normalize_profile_string(profile.jump_profile_id.as_deref()).is_some() {
        return Err("SSH踏み台の多段指定には対応していません".into());
    }
    Ok(())
}

fn build_jump_profile(profile: &SavedConnection) -> Result<SshJumpProfile, String> {
    let host = normalize_profile_string(profile.host.as_deref())
        .ok_or_else(|| "SSH踏み台プロファイルにホストが設定されていません".to_string())?;
    let username = normalize_profile_string(profile.username.as_deref())
        .ok_or_else(|| "SSH踏み台プロファイルにユーザー名が設定されていません".to_string())?;
    let auth_method = normalize_profile_auth_method(profile.auth_method.as_deref())?;
    let private_key_path = normalize_profile_string(profile.private_key_path.as_deref());
    if auth_method == "public_key" && private_key_path.is_none() {
        return Err("SSH踏み台プロファイルに秘密鍵ファイルが設定されていません".to_string());
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
