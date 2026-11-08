use super::model::{
    WorkspaceDragDropResult, WorkspaceDragPreview, WorkspacePointerPosition, WorkspaceSnapshot,
    WorkspaceTabMetadataPatch, WorkspaceTabRegisterInput, WorkspaceWindowCloseResult,
    WorkspaceWindowCreateResult,
};
use super::state::WorkspaceState;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

fn command_result<T>(
    result: Result<T, String>,
) -> Result<T, crate::command_error::BackendCommandError> {
    result.map_err(Into::into)
}

pub fn emit_workspace_updated(app: &AppHandle, snapshot: &WorkspaceSnapshot) {
    if let Err(error) = app.emit("workspace://updated", snapshot) {
        log::warn!("Workspace update event failed: {error}");
    }
}

pub fn emit_workspace_updates(app: &AppHandle, snapshots: &[WorkspaceSnapshot]) {
    for snapshot in snapshots {
        emit_workspace_updated(app, snapshot);
    }
}

pub fn emit_workspace_window_closed(app: &AppHandle, result: &WorkspaceWindowCloseResult) {
    if let Err(error) = app.emit("workspace://window-closed", result) {
        log::warn!("Workspace window closed event failed: {error}");
    }
}

pub fn emit_workspace_drag_preview(app: &AppHandle, preview: &WorkspaceDragPreview) {
    if let Err(error) = app.emit("workspace://drag-preview", preview) {
        log::warn!("Workspace drag preview event failed: {error}");
    }
}

fn create_workspace_window(app: &AppHandle, window_id: &str) -> Result<(), String> {
    WebviewWindowBuilder::new(app, window_id, WebviewUrl::default())
        .title("ExaTerm")
        .inner_size(1280.0, 800.0)
        .min_inner_size(320.0, 240.0)
        .decorations(false)
        .transparent(false)
        .focused(true)
        .build()
        .map_err(|error| format!("Failed to create the window: {error}"))?;

    if let Some(window) = app.get_webview_window(window_id) {
        if let Err(error) = window.set_focus() {
            log::warn!("New workspace window focus failed: {error}");
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn workspace_window_create(
    app: AppHandle,
) -> Result<WorkspaceWindowCreateResult, crate::command_error::BackendCommandError> {
    let window_id = format!("workspace-{}", Uuid::new_v4().simple());
    command_result(create_workspace_window(&app, &window_id))?;

    Ok(WorkspaceWindowCreateResult { window_id })
}

#[tauri::command]
pub async fn workspace_window_register(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    label: String,
    focused: bool,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state.register_window(window_id, label, focused).await;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_window_unregister(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
) -> Result<WorkspaceWindowCloseResult, String> {
    let result = state.unregister_window(window_id).await;
    emit_workspace_updates(&app, &result.snapshots);
    emit_workspace_window_closed(&app, &result);
    Ok(result)
}

#[tauri::command]
pub async fn workspace_window_focus(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state.focus_window(window_id).await;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_move(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    tab_id: String,
    from_window_id: String,
    to_window_id: String,
    target_index: usize,
) -> Result<WorkspaceSnapshot, crate::command_error::BackendCommandError> {
    let snapshots = command_result(
        state
            .move_tab(tab_id, from_window_id, to_window_id.clone(), target_index)
            .await,
    )?;
    emit_workspace_updates(&app, &snapshots);
    command_result(
        snapshots
            .into_iter()
            .find(|snapshot| snapshot.window_id == to_window_id)
            .ok_or_else(|| "Destination snapshot not found".to_string()),
    )
}

#[tauri::command]
pub async fn workspace_tab_drag_start(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    tab_id: String,
    pointer_screen_position: WorkspacePointerPosition,
) -> Result<WorkspaceDragPreview, crate::command_error::BackendCommandError> {
    let preview = command_result(
        state
            .drag_start(window_id, tab_id, pointer_screen_position)
            .await,
    )?;
    emit_workspace_drag_preview(&app, &preview);
    Ok(preview)
}

#[tauri::command]
pub async fn workspace_tab_drag_update(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    pointer_screen_position: WorkspacePointerPosition,
) -> Result<WorkspaceDragPreview, String> {
    let preview = state.drag_update(pointer_screen_position).await;
    emit_workspace_drag_preview(&app, &preview);
    Ok(preview)
}

#[tauri::command]
pub async fn workspace_tab_drag_hover(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    target_index: Option<usize>,
) -> Result<WorkspaceDragPreview, String> {
    let preview = state.drag_hover(window_id, target_index).await;
    emit_workspace_drag_preview(&app, &preview);
    Ok(preview)
}

#[tauri::command]
pub async fn workspace_tab_drag_drop(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    pointer_screen_position: WorkspacePointerPosition,
) -> Result<WorkspaceDragDropResult, crate::command_error::BackendCommandError> {
    let intent = match state.drag_drop_prepare(pointer_screen_position).await {
        Ok(intent) => intent,
        Err(error) => {
            let preview = state.drag_cancel().await;
            emit_workspace_drag_preview(&app, &preview);
            return Err(error.into());
        }
    };

    let (target_window_id, created_window_id, move_result) = match intent.target_window_id {
        Some(window_id) => {
            let result = state
                .move_tab(
                    intent.tab_id.clone(),
                    intent.source_window_id.clone(),
                    window_id.clone(),
                    intent.target_index,
                )
                .await;
            (window_id, None, result)
        }
        None => {
            let window_id = format!("workspace-{}", Uuid::new_v4().simple());
            let result = state
                .detach_tab_with(
                    intent.tab_id.clone(),
                    intent.source_window_id.clone(),
                    window_id.clone(),
                    || create_workspace_window(&app, &window_id),
                )
                .await;
            (window_id.clone(), Some(window_id), result)
        }
    };
    let preview = state.drag_cancel().await;
    emit_workspace_drag_preview(&app, &preview);
    let snapshots = command_result(move_result)?;
    emit_workspace_updates(&app, &snapshots);

    Ok(WorkspaceDragDropResult {
        action: if created_window_id.is_some() {
            "detach".to_string()
        } else {
            "move".to_string()
        },
        tab_id: intent.tab_id,
        source_window_id: intent.source_window_id,
        target_window_id: Some(target_window_id),
        created_window_id,
        snapshots,
    })
}

#[tauri::command]
pub async fn workspace_tab_detach_to_new_window(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    tab_id: String,
    from_window_id: String,
) -> Result<WorkspaceDragDropResult, crate::command_error::BackendCommandError> {
    let window_id = format!("workspace-{}", Uuid::new_v4().simple());
    let snapshots = command_result(
        state
            .detach_tab_with(
                tab_id.clone(),
                from_window_id.clone(),
                window_id.clone(),
                || create_workspace_window(&app, &window_id),
            )
            .await,
    )?;
    emit_workspace_updates(&app, &snapshots);

    Ok(WorkspaceDragDropResult {
        action: "detach".to_string(),
        tab_id,
        source_window_id: from_window_id,
        target_window_id: Some(window_id.clone()),
        created_window_id: Some(window_id),
        snapshots,
    })
}

#[tauri::command]
pub async fn workspace_tab_drag_cancel(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
) -> Result<WorkspaceDragPreview, String> {
    let preview = state.drag_cancel().await;
    emit_workspace_drag_preview(&app, &preview);
    Ok(preview)
}

#[tauri::command]
pub async fn workspace_snapshot_get(
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
) -> Result<WorkspaceSnapshot, String> {
    Ok(state.snapshot_for_window(window_id).await)
}

#[tauri::command]
pub async fn workspace_tab_register(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    input: WorkspaceTabRegisterInput,
    terminals: tauri::State<'_, crate::terminal_control::TerminalControlState>,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state.register_session_tab(input, &terminals).await;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_activate(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    tab_id: String,
) -> Result<WorkspaceSnapshot, crate::command_error::BackendCommandError> {
    let snapshot = command_result(state.activate_tab(window_id, tab_id).await)?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_reorder(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    dragged_tab_id: String,
    target_tab_id: String,
    drop_side: String,
) -> Result<WorkspaceSnapshot, crate::command_error::BackendCommandError> {
    let snapshot = command_result(
        state
            .reorder_tab(window_id, dragged_tab_id, target_tab_id, drop_side)
            .await,
    )?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_remove(
    window: tauri::WebviewWindow,
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    tab_id: String,
) -> Result<WorkspaceSnapshot, crate::command_error::BackendCommandError> {
    if window.label() != window_id {
        return Err("The tab does not belong to this window".to_string().into());
    }
    let snapshot = command_result(state.remove_tab(window_id, tab_id).await)?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_update_metadata(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    tab_id: String,
    patch: WorkspaceTabMetadataPatch,
) -> Result<WorkspaceSnapshot, crate::command_error::BackendCommandError> {
    let snapshot = command_result(state.update_tab_metadata(tab_id, patch).await)?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_close(
    app: AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, WorkspaceState>,
    tab_id: String,
) -> Result<WorkspaceSnapshot, crate::command_error::BackendCommandError> {
    let close_app = app.clone();
    command_result(
        state
            .close_tab_owned(
                window.label().to_string(),
                tab_id,
                move |tab| async move {
                    let logger = close_app.state::<crate::logger::LoggerState>();
                    let workspace = close_app.state::<WorkspaceState>();
                    crate::logger::stop_for_tab_close(
                        &close_app,
                        &logger,
                        &workspace,
                        &tab.session_id,
                    )
                    .await?;
                    let terminals =
                        close_app.state::<crate::terminal_control::TerminalControlState>();
                    match tab.connection_type {
                        crate::terminal_control::TerminalProtocol::Ssh => {
                            crate::ssh::disconnect(
                                &close_app,
                                &close_app.state::<crate::ssh::SshState>(),
                                &terminals,
                                &workspace,
                                Some(&logger),
                                &tab.session_id,
                            )
                            .await
                        }
                        crate::terminal_control::TerminalProtocol::Serial => {
                            crate::serial::disconnect(
                                &close_app,
                                &close_app.state::<crate::serial::SerialState>(),
                                &terminals,
                                &workspace,
                                Some(&logger),
                                &tab.session_id,
                            )
                            .await
                        }
                        crate::terminal_control::TerminalProtocol::Telnet => {
                            crate::telnet::disconnect(
                                &close_app,
                                &close_app.state::<crate::telnet::TelnetState>(),
                                &terminals,
                                &workspace,
                                Some(&logger),
                                &tab.session_id,
                            )
                            .await
                        }
                    }
                },
                move |snapshot| emit_workspace_updated(&app, snapshot),
            )
            .await,
    )
}
