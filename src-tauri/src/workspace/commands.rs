use super::model::{
    WorkspaceConnectionInfo, WorkspaceDragDropResult, WorkspaceDragPreview,
    WorkspacePointerPosition, WorkspaceSnapshot, WorkspaceTabMetadataPatch,
    WorkspaceTabRegisterInput, WorkspaceWindowCloseResult, WorkspaceWindowCreateResult,
};
use super::state::WorkspaceState;
use crate::terminal_control::TerminalProtocol;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

fn localize<T>(
    language: &tauri::State<'_, crate::i18n::BackendLanguageState>,
    result: Result<T, String>,
) -> Result<T, String> {
    result.map_err(|error| crate::i18n::translate_gui_error(language.inner(), &error))
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
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
) -> Result<WorkspaceWindowCreateResult, String> {
    let window_id = format!("workspace-{}", Uuid::new_v4().simple());
    localize(&language, create_workspace_window(&app, &window_id))?;

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
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    tab_id: String,
    from_window_id: String,
    to_window_id: String,
    target_index: usize,
) -> Result<WorkspaceSnapshot, String> {
    let snapshots = localize(
        &language,
        state
            .move_tab(tab_id, from_window_id, to_window_id.clone(), target_index)
            .await,
    )?;
    emit_workspace_updates(&app, &snapshots);
    localize(
        &language,
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
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    window_id: String,
    tab_id: String,
    pointer_screen_position: WorkspacePointerPosition,
) -> Result<WorkspaceDragPreview, String> {
    let preview = localize(
        &language,
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
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    pointer_screen_position: WorkspacePointerPosition,
) -> Result<WorkspaceDragDropResult, String> {
    let intent = match state.drag_drop_prepare(pointer_screen_position).await {
        Ok(intent) => intent,
        Err(error) => {
            let preview = state.drag_cancel().await;
            emit_workspace_drag_preview(&app, &preview);
            return Err(crate::i18n::translate_gui_error(language.inner(), &error));
        }
    };

    let mut created_window_id = None;
    let target_window_id = match intent.target_window_id {
        Some(window_id) => window_id,
        None => {
            let window_id = format!("workspace-{}", Uuid::new_v4().simple());
            if let Err(error) = create_workspace_window(&app, &window_id) {
                let preview = state.drag_cancel().await;
                emit_workspace_drag_preview(&app, &preview);
                return Err(crate::i18n::translate_gui_error(language.inner(), &error));
            }
            let snapshot = state
                .register_window(window_id.clone(), window_id.clone(), true)
                .await;
            emit_workspace_updated(&app, &snapshot);
            created_window_id = Some(window_id.clone());
            window_id
        }
    };

    let move_result = state
        .move_tab(
            intent.tab_id.clone(),
            intent.source_window_id.clone(),
            target_window_id.clone(),
            intent.target_index,
        )
        .await;
    let preview = state.drag_cancel().await;
    emit_workspace_drag_preview(&app, &preview);

    let snapshots = localize(&language, move_result)?;
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
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    tab_id: String,
    from_window_id: String,
) -> Result<WorkspaceDragDropResult, String> {
    localize(
        &language,
        state
            .validate_tab_move_source(&tab_id, &from_window_id)
            .await,
    )?;

    let window_id = format!("workspace-{}", Uuid::new_v4().simple());
    localize(&language, create_workspace_window(&app, &window_id))?;
    let snapshot = state
        .register_window(window_id.clone(), window_id.clone(), true)
        .await;
    emit_workspace_updated(&app, &snapshot);

    let snapshots = localize(
        &language,
        state
            .move_tab(tab_id.clone(), from_window_id.clone(), window_id.clone(), 0)
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
    window_id: Option<String>,
    session_id: String,
    connection_type: TerminalProtocol,
    title: String,
    encoding: String,
    terminal_mode: String,
    connection_info: Option<WorkspaceConnectionInfo>,
    is_auto_logging: bool,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state
        .register_tab(WorkspaceTabRegisterInput {
            window_id,
            tab_id: None,
            session_id,
            connection_type,
            title,
            encoding,
            terminal_mode,
            connection_info,
            is_auto_logging,
        })
        .await;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_activate(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    window_id: String,
    tab_id: String,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = localize(&language, state.activate_tab(window_id, tab_id).await)?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_reorder(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    window_id: String,
    dragged_tab_id: String,
    target_tab_id: String,
    drop_side: String,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = localize(
        &language,
        state
            .reorder_tab(window_id, dragged_tab_id, target_tab_id, drop_side)
            .await,
    )?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_remove(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    window_id: String,
    tab_id: String,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = localize(&language, state.remove_tab(window_id, tab_id).await)?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_update_metadata(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    language: tauri::State<'_, crate::i18n::BackendLanguageState>,
    tab_id: String,
    patch: WorkspaceTabMetadataPatch,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = localize(&language, state.update_tab_metadata(tab_id, patch).await)?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}
