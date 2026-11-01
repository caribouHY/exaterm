use crate::logger::{finish_all_logs, LoggerState};
use crate::workspace::WorkspaceState;
use serde::Serialize;
use tauri::{AppHandle, State};

const ERROR_ACTIVE_SESSIONS: &str = "active_sessions";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppExitCommandError {
    pub code: String,
    pub message: String,
    pub active_session_count: usize,
}

impl AppExitCommandError {
    fn active_sessions(count: usize) -> Self {
        Self {
            code: ERROR_ACTIVE_SESSIONS.into(),
            message: "Connected terminal sessions require confirmation before exiting.".into(),
            active_session_count: count,
        }
    }
}

fn ensure_active_sessions_allowed(
    active_session_count: usize,
    allow_active_sessions: bool,
) -> Result<(), AppExitCommandError> {
    if active_session_count > 0 && !allow_active_sessions {
        return Err(AppExitCommandError::active_sessions(active_session_count));
    }
    Ok(())
}

#[tauri::command]
pub async fn app_exit(
    app: AppHandle,
    workspace: State<'_, WorkspaceState>,
    logger: State<'_, LoggerState>,
    allow_active_sessions: bool,
) -> Result<(), AppExitCommandError> {
    let active_session_count = workspace.connected_session_count().await;
    ensure_active_sessions_allowed(active_session_count, allow_active_sessions)?;
    finish_all_logs(&logger)
        .await
        .map_err(|_| AppExitCommandError {
            code: "log_flush_failed".into(),
            message: "Failed to save pending terminal log output before exiting.".into(),
            active_session_count,
        })?;
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_app_can_exit_without_confirmation() {
        assert!(ensure_active_sessions_allowed(0, false).is_ok());
    }

    #[test]
    fn active_sessions_require_confirmation() {
        let error = ensure_active_sessions_allowed(2, false).unwrap_err();
        assert_eq!(error.code, ERROR_ACTIVE_SESSIONS);
        assert_eq!(error.active_session_count, 2);
    }

    #[test]
    fn confirmed_active_sessions_can_exit() {
        assert!(ensure_active_sessions_allowed(2, true).is_ok());
    }
}
