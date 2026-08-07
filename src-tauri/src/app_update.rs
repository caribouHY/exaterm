use crate::workspace::WorkspaceState;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex as AsyncMutex;

const ERROR_BUSY: &str = "busy";
const ERROR_NO_PENDING_UPDATE: &str = "no_pending_update";
const ERROR_ACTIVE_SESSIONS: &str = "active_sessions";
const ERROR_CHECK_FAILED: &str = "check_failed";
const ERROR_INSTALL_FAILED: &str = "install_failed";

pub struct AppUpdateState {
    operation: AsyncMutex<()>,
    pending_update: Mutex<Option<Update>>,
}

impl AppUpdateState {
    pub fn new() -> Self {
        Self {
            operation: AsyncMutex::new(()),
            pending_update: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateMetadata {
    pub version: String,
    pub current_version: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateCommandError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_session_count: Option<usize>,
}

impl AppUpdateCommandError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            active_session_count: None,
        }
    }

    fn active_sessions(count: usize) -> Self {
        Self {
            code: ERROR_ACTIVE_SESSIONS.into(),
            message: "Connected terminal sessions require confirmation before updating.".into(),
            active_session_count: Some(count),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum AppUpdateDownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
    },
    Finished,
}

fn ensure_active_sessions_allowed(
    active_session_count: usize,
    allow_active_sessions: bool,
) -> Result<(), AppUpdateCommandError> {
    if active_session_count > 0 && !allow_active_sessions {
        return Err(AppUpdateCommandError::active_sessions(active_session_count));
    }
    Ok(())
}

fn replace_pending_update<T>(
    pending_update: &Mutex<Option<T>>,
    update: Option<T>,
) -> Result<(), AppUpdateCommandError> {
    *pending_update.lock().map_err(|_| {
        AppUpdateCommandError::new(ERROR_CHECK_FAILED, "Update state is unavailable.")
    })? = update;
    Ok(())
}

fn take_pending_update<T>(pending_update: &Mutex<Option<T>>) -> Result<T, AppUpdateCommandError> {
    pending_update
        .lock()
        .map_err(|_| {
            AppUpdateCommandError::new(ERROR_NO_PENDING_UPDATE, "Update state is unavailable.")
        })?
        .take()
        .ok_or_else(|| {
            AppUpdateCommandError::new(ERROR_NO_PENDING_UPDATE, "There is no pending update.")
        })
}

fn restore_pending_update<T>(pending_update: &Mutex<Option<T>>, update: T) {
    if let Ok(mut pending_update) = pending_update.lock() {
        *pending_update = Some(update);
    }
}

#[tauri::command]
pub async fn app_update_check(
    app: AppHandle,
    state: State<'_, AppUpdateState>,
) -> Result<Option<AppUpdateMetadata>, AppUpdateCommandError> {
    let _operation = state.operation.try_lock().map_err(|_| {
        AppUpdateCommandError::new(ERROR_BUSY, "An update operation is in progress.")
    })?;

    let update = app
        .updater()
        .map_err(|error| AppUpdateCommandError::new(ERROR_CHECK_FAILED, error.to_string()))?
        .check()
        .await
        .map_err(|error| AppUpdateCommandError::new(ERROR_CHECK_FAILED, error.to_string()))?;

    let metadata = update.as_ref().map(|update| AppUpdateMetadata {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        published_at: update.date.map(|date| date.to_string()),
    });

    replace_pending_update(&state.pending_update, update)?;

    Ok(metadata)
}

#[tauri::command]
pub async fn app_update_install(
    state: State<'_, AppUpdateState>,
    workspace: State<'_, WorkspaceState>,
    allow_active_sessions: bool,
    on_event: Channel<AppUpdateDownloadEvent>,
) -> Result<(), AppUpdateCommandError> {
    let _operation = state.operation.try_lock().map_err(|_| {
        AppUpdateCommandError::new(ERROR_BUSY, "An update operation is in progress.")
    })?;

    ensure_active_sessions_allowed(
        workspace.connected_session_count().await,
        allow_active_sessions,
    )?;

    let update = take_pending_update(&state.pending_update)?;

    let mut started = false;
    let result = update
        .download_and_install(
            |chunk_length, content_length| {
                if !started {
                    let _ = on_event.send(AppUpdateDownloadEvent::Started { content_length });
                    started = true;
                }
                let _ = on_event.send(AppUpdateDownloadEvent::Progress { chunk_length });
            },
            || {
                let _ = on_event.send(AppUpdateDownloadEvent::Finished);
            },
        )
        .await;

    if let Err(error) = result {
        restore_pending_update(&state.pending_update, update);
        return Err(AppUpdateCommandError::new(
            ERROR_INSTALL_FAILED,
            error.to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_sessions_require_explicit_confirmation() {
        let error = ensure_active_sessions_allowed(2, false).unwrap_err();
        assert_eq!(error.code, ERROR_ACTIVE_SESSIONS);
        assert_eq!(error.active_session_count, Some(2));
    }

    #[test]
    fn confirmed_or_disconnected_updates_are_allowed() {
        assert!(ensure_active_sessions_allowed(0, false).is_ok());
        assert!(ensure_active_sessions_allowed(2, true).is_ok());
    }

    #[tokio::test]
    async fn only_one_update_operation_can_run_at_a_time() {
        let state = AppUpdateState::new();
        let operation = state.operation.try_lock().unwrap();
        assert!(state.operation.try_lock().is_err());
        drop(operation);
        assert!(state.operation.try_lock().is_ok());
    }

    #[test]
    fn pending_update_tracks_available_and_no_update_results() {
        let pending = Mutex::new(None);
        replace_pending_update(&pending, Some("0.9.0")).unwrap();
        assert_eq!(take_pending_update(&pending).unwrap(), "0.9.0");

        replace_pending_update(&pending, Some("stale")).unwrap();
        replace_pending_update(&pending, None).unwrap();
        let error = take_pending_update::<&str>(&pending).unwrap_err();
        assert_eq!(error.code, ERROR_NO_PENDING_UPDATE);
    }

    #[test]
    fn failed_install_can_restore_pending_update_for_retry() {
        let pending = Mutex::new(Some("0.9.0"));
        let update = take_pending_update(&pending).unwrap();
        restore_pending_update(&pending, update);
        assert_eq!(take_pending_update(&pending).unwrap(), "0.9.0");
    }
}
