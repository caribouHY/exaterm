use crate::terminal_control::TerminalProtocol;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::Mutex;
use uuid::Uuid;

const MAIN_WINDOW_ID: &str = "main";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub window_id: String,
    pub window: WindowWorkspace,
    pub tabs: Vec<WorkspaceTab>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceWindowCreateResult {
    pub window_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceWindowCloseResult {
    pub window_id: String,
    pub rehome_window_id: Option<String>,
    pub remaining_window_count: usize,
    pub snapshots: Vec<WorkspaceSnapshot>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct WorkspacePointerPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WorkspaceDragPreview {
    pub active: bool,
    pub tab_id: Option<String>,
    pub source_window_id: Option<String>,
    pub pointer_screen_position: Option<WorkspacePointerPosition>,
    pub target_window_id: Option<String>,
    pub target_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceDragDropResult {
    pub action: String,
    pub tab_id: String,
    pub source_window_id: String,
    pub target_window_id: Option<String>,
    pub created_window_id: Option<String>,
    pub snapshots: Vec<WorkspaceSnapshot>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WindowWorkspace {
    pub window_id: String,
    pub label: String,
    pub tab_order: Vec<String>,
    pub active_tab_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceTab {
    pub tab_id: String,
    pub session_id: String,
    pub connection_type: TerminalProtocol,
    pub title: String,
    pub owner_window_id: String,
    pub encoding: String,
    pub terminal_mode: String,
    pub is_connected: bool,
    pub is_auto_logging: bool,
    pub is_manual_logging: bool,
    pub is_logging_paused: bool,
    pub manual_log_file_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTabMetadataPatch {
    pub title: Option<String>,
    pub encoding: Option<String>,
    pub terminal_mode: Option<String>,
    pub is_connected: Option<bool>,
    pub is_auto_logging: Option<bool>,
    pub is_manual_logging: Option<bool>,
    pub is_logging_paused: Option<bool>,
    pub manual_log_file_path: Option<String>,
}

#[derive(Debug, Default)]
struct WorkspaceModel {
    windows: HashMap<String, WindowWorkspace>,
    tabs: HashMap<String, WorkspaceTab>,
    last_focused_window: Option<String>,
    focused_window_order: Vec<String>,
    drag: Option<WorkspaceDragSession>,
}

#[derive(Debug, Clone, PartialEq)]
struct WorkspaceDragSession {
    tab_id: String,
    source_window_id: String,
    pointer_screen_position: WorkspacePointerPosition,
    target_window_id: Option<String>,
    target_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
struct WorkspaceDragDropIntent {
    tab_id: String,
    source_window_id: String,
    target_window_id: Option<String>,
    target_index: usize,
}

#[derive(Clone, Default)]
pub struct WorkspaceState {
    model: Arc<Mutex<WorkspaceModel>>,
}

impl WorkspaceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_window(
        &self,
        window_id: String,
        label: String,
        focused: bool,
    ) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        model
            .windows
            .entry(window_id.clone())
            .and_modify(|window| {
                window.label = label.clone();
            })
            .or_insert_with(|| WindowWorkspace {
                window_id: window_id.clone(),
                label,
                tab_order: Vec::new(),
                active_tab_id: None,
            });
        if focused {
            set_focused_window(&mut model, &window_id);
        }
        snapshot_for_locked(&model, &window_id)
    }

    pub async fn focus_window(&self, window_id: String) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        ensure_window(&mut model, &window_id, &window_id);
        set_focused_window(&mut model, &window_id);
        snapshot_for_locked(&model, &window_id)
    }

    pub async fn unregister_window(&self, window_id: String) -> WorkspaceWindowCloseResult {
        let mut model = self.model.lock().await;
        let removed_window = model.windows.remove(&window_id);
        remove_focused_window(&mut model, &window_id);

        let mut snapshots = Vec::new();
        let mut rehome_window_id = None;

        if let Some(removed_window) = removed_window {
            if !model.windows.is_empty() && !removed_window.tab_order.is_empty() {
                let destination_window_id = choose_rehome_window(&model);
                if let Some(destination_window_id) = destination_window_id {
                    let moved_tab_ids = removed_window
                        .tab_order
                        .into_iter()
                        .filter(|tab_id| model.tabs.contains_key(tab_id))
                        .collect::<Vec<_>>();

                    if !moved_tab_ids.is_empty() {
                        {
                            let destination = model
                                .windows
                                .get_mut(&destination_window_id)
                                .expect("rehome destination is selected from registered windows");
                            let had_active_tab = destination.active_tab_id.is_some();
                            for tab_id in &moved_tab_ids {
                                if !destination.tab_order.contains(tab_id) {
                                    destination.tab_order.push(tab_id.clone());
                                }
                            }
                            if !had_active_tab {
                                destination.active_tab_id = moved_tab_ids.first().cloned();
                            }
                        }

                        for tab_id in &moved_tab_ids {
                            if let Some(tab) = model.tabs.get_mut(tab_id) {
                                tab.owner_window_id = destination_window_id.clone();
                            }
                        }
                    }

                    rehome_window_id = Some(destination_window_id.clone());
                    snapshots.push(snapshot_for_locked(&model, &destination_window_id));
                }
            }
        }

        WorkspaceWindowCloseResult {
            window_id,
            rehome_window_id,
            remaining_window_count: model.windows.len(),
            snapshots,
        }
    }

    pub async fn snapshot_for_window(&self, window_id: String) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        ensure_window(&mut model, &window_id, &window_id);
        snapshot_for_locked(&model, &window_id)
    }

    pub async fn register_tab(&self, input: WorkspaceTabRegisterInput) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        let owner_window_id = choose_owner_window(&mut model, input.window_id.as_deref());
        let tab_id = input.tab_id.unwrap_or_else(|| input.session_id.clone());
        remove_tab_from_all_windows(&mut model, &tab_id);

        let tab = WorkspaceTab {
            tab_id: tab_id.clone(),
            session_id: input.session_id,
            connection_type: input.connection_type,
            title: input.title,
            owner_window_id: owner_window_id.clone(),
            encoding: input.encoding,
            terminal_mode: input.terminal_mode,
            is_connected: true,
            is_auto_logging: input.is_auto_logging,
            is_manual_logging: false,
            is_logging_paused: false,
            manual_log_file_path: None,
        };
        model.tabs.insert(tab_id.clone(), tab);

        let window = model
            .windows
            .get_mut(&owner_window_id)
            .expect("owner window is ensured before tab registration");
        if !window.tab_order.contains(&tab_id) {
            window.tab_order.push(tab_id.clone());
        }
        window.active_tab_id = Some(tab_id);

        snapshot_for_locked(&model, &owner_window_id)
    }

    pub async fn activate_tab(
        &self,
        window_id: String,
        tab_id: String,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        let window = model
            .windows
            .get_mut(&window_id)
            .ok_or_else(|| "ウィンドウが見つかりません".to_string())?;
        if !window.tab_order.contains(&tab_id) {
            return Err("タブがこのウィンドウにありません".to_string());
        }
        window.active_tab_id = Some(tab_id);
        model.last_focused_window = Some(window_id.clone());
        Ok(snapshot_for_locked(&model, &window_id))
    }

    pub async fn reorder_tab(
        &self,
        window_id: String,
        dragged_tab_id: String,
        target_tab_id: String,
        drop_side: String,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        let window = model
            .windows
            .get_mut(&window_id)
            .ok_or_else(|| "ウィンドウが見つかりません".to_string())?;
        if dragged_tab_id == target_tab_id {
            return Ok(snapshot_for_locked(&model, &window_id));
        }
        let dragged_index = window
            .tab_order
            .iter()
            .position(|id| id == &dragged_tab_id)
            .ok_or_else(|| "移動元タブが見つかりません".to_string())?;
        let _target_index = window
            .tab_order
            .iter()
            .position(|id| id == &target_tab_id)
            .ok_or_else(|| "移動先タブが見つかりません".to_string())?;

        let dragged = window.tab_order.remove(dragged_index);
        let target_index_after_removal = window
            .tab_order
            .iter()
            .position(|id| id == &target_tab_id)
            .ok_or_else(|| "移動先タブが見つかりません".to_string())?;
        let insert_index = if drop_side == "after" {
            target_index_after_removal + 1
        } else {
            target_index_after_removal
        };
        window.tab_order.insert(insert_index, dragged);
        Ok(snapshot_for_locked(&model, &window_id))
    }

    pub async fn move_tab(
        &self,
        tab_id: String,
        from_window_id: String,
        to_window_id: String,
        target_index: usize,
    ) -> Result<Vec<WorkspaceSnapshot>, String> {
        let mut model = self.model.lock().await;
        if !model.windows.contains_key(&to_window_id) {
            return Err("移動先ウィンドウが見つかりません".to_string());
        }

        let source_window = model
            .windows
            .get(&from_window_id)
            .ok_or_else(|| "移動元ウィンドウが見つかりません".to_string())?;
        if !source_window.tab_order.contains(&tab_id) {
            return Err("移動元タブが見つかりません".to_string());
        }

        let tab = model
            .tabs
            .get_mut(&tab_id)
            .ok_or_else(|| "タブが見つかりません".to_string())?;
        if tab.owner_window_id != from_window_id {
            return Err("タブの所有ウィンドウが一致しません".to_string());
        }
        tab.owner_window_id = to_window_id.clone();

        if from_window_id == to_window_id {
            let window = model
                .windows
                .get_mut(&from_window_id)
                .ok_or_else(|| "ウィンドウが見つかりません".to_string())?;
            window.tab_order.retain(|id| id != &tab_id);
            let insert_index = target_index.min(window.tab_order.len());
            window.tab_order.insert(insert_index, tab_id);
            return Ok(vec![snapshot_for_locked(&model, &from_window_id)]);
        }

        {
            let source = model
                .windows
                .get_mut(&from_window_id)
                .ok_or_else(|| "移動元ウィンドウが見つかりません".to_string())?;
            source.tab_order.retain(|id| id != &tab_id);
            if source.active_tab_id.as_deref() == Some(tab_id.as_str()) {
                source.active_tab_id = source.tab_order.last().cloned();
            }
        }

        {
            let destination = model
                .windows
                .get_mut(&to_window_id)
                .ok_or_else(|| "移動先ウィンドウが見つかりません".to_string())?;
            destination.tab_order.retain(|id| id != &tab_id);
            let insert_index = target_index.min(destination.tab_order.len());
            destination.tab_order.insert(insert_index, tab_id.clone());
            destination.active_tab_id = Some(tab_id);
        }

        Ok(vec![
            snapshot_for_locked(&model, &from_window_id),
            snapshot_for_locked(&model, &to_window_id),
        ])
    }

    pub async fn remove_tab(
        &self,
        window_id: String,
        tab_id: String,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        model.tabs.remove(&tab_id);
        let window = model
            .windows
            .get_mut(&window_id)
            .ok_or_else(|| "ウィンドウが見つかりません".to_string())?;
        window.tab_order.retain(|id| id != &tab_id);
        if window.active_tab_id.as_deref() == Some(tab_id.as_str()) {
            window.active_tab_id = window.tab_order.last().cloned();
        }
        Ok(snapshot_for_locked(&model, &window_id))
    }

    pub async fn update_tab_metadata(
        &self,
        tab_id: String,
        patch: WorkspaceTabMetadataPatch,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        let owner_window_id = {
            let tab = model
                .tabs
                .get_mut(&tab_id)
                .ok_or_else(|| "タブが見つかりません".to_string())?;
            apply_metadata_patch(tab, patch);
            tab.owner_window_id.clone()
        };
        Ok(snapshot_for_locked(&model, &owner_window_id))
    }

    pub async fn mark_disconnected(&self, session_id: &str) -> Option<WorkspaceSnapshot> {
        let mut model = self.model.lock().await;
        let owner_window_id = model
            .tabs
            .values_mut()
            .find(|tab| tab.session_id == session_id)
            .map(|tab| {
                tab.is_connected = false;
                if !tab.is_auto_logging && !tab.is_manual_logging {
                    tab.is_logging_paused = false;
                }
                tab.owner_window_id.clone()
            })?;
        Some(snapshot_for_locked(&model, &owner_window_id))
    }

    pub async fn drag_start(
        &self,
        window_id: String,
        tab_id: String,
        pointer_screen_position: WorkspacePointerPosition,
    ) -> Result<WorkspaceDragPreview, String> {
        let mut model = self.model.lock().await;
        let window = model
            .windows
            .get(&window_id)
            .ok_or_else(|| "ウィンドウが見つかりません".to_string())?;
        if !window.tab_order.contains(&tab_id) {
            return Err("タブがこのウィンドウにありません".to_string());
        }
        let tab = model
            .tabs
            .get(&tab_id)
            .ok_or_else(|| "タブが見つかりません".to_string())?;
        if tab.owner_window_id != window_id {
            return Err("タブの所有ウィンドウが一致しません".to_string());
        }

        model.drag = Some(WorkspaceDragSession {
            tab_id,
            source_window_id: window_id,
            pointer_screen_position,
            target_window_id: None,
            target_index: None,
        });

        Ok(drag_preview_for_locked(&model))
    }

    pub async fn drag_update(
        &self,
        pointer_screen_position: WorkspacePointerPosition,
    ) -> WorkspaceDragPreview {
        let mut model = self.model.lock().await;
        if let Some(drag) = model.drag.as_mut() {
            drag.pointer_screen_position = pointer_screen_position;
        }
        drag_preview_for_locked(&model)
    }

    pub async fn drag_hover(
        &self,
        window_id: String,
        target_index: Option<usize>,
    ) -> WorkspaceDragPreview {
        let mut model = self.model.lock().await;
        let window_exists = model.windows.contains_key(&window_id);
        if let Some(drag) = model.drag.as_mut() {
            if window_exists && target_index.is_some() {
                drag.target_window_id = Some(window_id);
                drag.target_index = target_index;
            } else if drag.target_window_id.as_deref() == Some(&window_id) {
                drag.target_window_id = None;
                drag.target_index = None;
            }
        }
        drag_preview_for_locked(&model)
    }

    pub async fn drag_cancel(&self) -> WorkspaceDragPreview {
        let mut model = self.model.lock().await;
        model.drag = None;
        drag_preview_for_locked(&model)
    }

    async fn drag_drop_prepare(
        &self,
        pointer_screen_position: WorkspacePointerPosition,
    ) -> Result<WorkspaceDragDropIntent, String> {
        let mut model = self.model.lock().await;
        let mut drag = model
            .drag
            .take()
            .ok_or_else(|| "ドラッグ中のタブがありません".to_string())?;
        drag.pointer_screen_position = pointer_screen_position;
        if !model.tabs.contains_key(&drag.tab_id) {
            return Err("タブが見つかりません".to_string());
        }
        if !model.windows.contains_key(&drag.source_window_id) {
            return Err("移動元ウィンドウが見つかりません".to_string());
        }

        Ok(WorkspaceDragDropIntent {
            tab_id: drag.tab_id,
            source_window_id: drag.source_window_id,
            target_window_id: drag.target_window_id,
            target_index: drag.target_index.unwrap_or(0),
        })
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceTabRegisterInput {
    pub window_id: Option<String>,
    pub tab_id: Option<String>,
    pub session_id: String,
    pub connection_type: TerminalProtocol,
    pub title: String,
    pub encoding: String,
    pub terminal_mode: String,
    pub is_auto_logging: bool,
}

fn apply_metadata_patch(tab: &mut WorkspaceTab, patch: WorkspaceTabMetadataPatch) {
    if let Some(title) = patch.title {
        tab.title = title;
    }
    if let Some(encoding) = patch.encoding {
        tab.encoding = encoding;
    }
    if let Some(terminal_mode) = patch.terminal_mode {
        tab.terminal_mode = terminal_mode;
    }
    if let Some(is_connected) = patch.is_connected {
        tab.is_connected = is_connected;
    }
    if let Some(is_auto_logging) = patch.is_auto_logging {
        tab.is_auto_logging = is_auto_logging;
    }
    if let Some(is_manual_logging) = patch.is_manual_logging {
        tab.is_manual_logging = is_manual_logging;
    }
    if let Some(is_logging_paused) = patch.is_logging_paused {
        tab.is_logging_paused = is_logging_paused;
    }
    if let Some(manual_log_file_path) = patch.manual_log_file_path {
        tab.manual_log_file_path = Some(manual_log_file_path);
    }
}

fn choose_owner_window(model: &mut WorkspaceModel, requested_window_id: Option<&str>) -> String {
    if let Some(window_id) = requested_window_id {
        ensure_window(model, window_id, window_id);
        set_focused_window(model, window_id);
        return window_id.to_string();
    }
    if let Some(window_id) = last_focused_existing_window(model) {
        ensure_window(model, &window_id, &window_id);
        return window_id;
    }
    ensure_window(model, MAIN_WINDOW_ID, MAIN_WINDOW_ID);
    MAIN_WINDOW_ID.to_string()
}

fn set_focused_window(model: &mut WorkspaceModel, window_id: &str) {
    model.last_focused_window = Some(window_id.to_string());
    model
        .focused_window_order
        .retain(|focused_window_id| focused_window_id != window_id);
    model.focused_window_order.push(window_id.to_string());
}

fn remove_focused_window(model: &mut WorkspaceModel, window_id: &str) {
    model
        .focused_window_order
        .retain(|focused_window_id| focused_window_id != window_id);
    model.last_focused_window = last_focused_existing_window(model);
}

fn last_focused_existing_window(model: &WorkspaceModel) -> Option<String> {
    model
        .focused_window_order
        .iter()
        .rev()
        .find(|window_id| model.windows.contains_key(*window_id))
        .cloned()
        .or_else(|| {
            model
                .last_focused_window
                .as_ref()
                .filter(|window_id| model.windows.contains_key(*window_id))
                .cloned()
        })
}

fn choose_rehome_window(model: &WorkspaceModel) -> Option<String> {
    last_focused_existing_window(model)
        .or_else(|| {
            model
                .windows
                .contains_key(MAIN_WINDOW_ID)
                .then(|| MAIN_WINDOW_ID.to_string())
        })
        .or_else(|| {
            let mut window_ids = model.windows.keys().cloned().collect::<Vec<_>>();
            window_ids.sort();
            window_ids.into_iter().next()
        })
}

fn ensure_window(model: &mut WorkspaceModel, window_id: &str, label: &str) {
    model
        .windows
        .entry(window_id.to_string())
        .or_insert_with(|| WindowWorkspace {
            window_id: window_id.to_string(),
            label: label.to_string(),
            tab_order: Vec::new(),
            active_tab_id: None,
        });
}

fn remove_tab_from_all_windows(model: &mut WorkspaceModel, tab_id: &str) {
    for window in model.windows.values_mut() {
        window.tab_order.retain(|id| id != tab_id);
        if window.active_tab_id.as_deref() == Some(tab_id) {
            window.active_tab_id = window.tab_order.last().cloned();
        }
    }
}

fn snapshot_for_locked(model: &WorkspaceModel, window_id: &str) -> WorkspaceSnapshot {
    let window = model
        .windows
        .get(window_id)
        .cloned()
        .unwrap_or_else(|| WindowWorkspace {
            window_id: window_id.to_string(),
            label: window_id.to_string(),
            tab_order: Vec::new(),
            active_tab_id: None,
        });
    let tabs = window
        .tab_order
        .iter()
        .filter_map(|tab_id| model.tabs.get(tab_id).cloned())
        .collect();

    WorkspaceSnapshot {
        window_id: window_id.to_string(),
        window,
        tabs,
    }
}

fn drag_preview_for_locked(model: &WorkspaceModel) -> WorkspaceDragPreview {
    if let Some(drag) = &model.drag {
        WorkspaceDragPreview {
            active: true,
            tab_id: Some(drag.tab_id.clone()),
            source_window_id: Some(drag.source_window_id.clone()),
            pointer_screen_position: Some(drag.pointer_screen_position),
            target_window_id: drag.target_window_id.clone(),
            target_index: drag.target_index,
        }
    } else {
        WorkspaceDragPreview {
            active: false,
            tab_id: None,
            source_window_id: None,
            pointer_screen_position: None,
            target_window_id: None,
            target_index: None,
        }
    }
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
        .map_err(|error| format!("ウィンドウ作成エラー: {error}"))?;

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
) -> Result<WorkspaceWindowCreateResult, String> {
    let window_id = format!("workspace-{}", Uuid::new_v4().simple());
    create_workspace_window(&app, &window_id)?;

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
) -> Result<WorkspaceSnapshot, String> {
    let snapshots = state
        .move_tab(tab_id, from_window_id, to_window_id.clone(), target_index)
        .await?;
    emit_workspace_updates(&app, &snapshots);
    snapshots
        .into_iter()
        .find(|snapshot| snapshot.window_id == to_window_id)
        .ok_or_else(|| "移動先スナップショットが見つかりません".to_string())
}

#[tauri::command]
pub async fn workspace_tab_drag_start(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    tab_id: String,
    pointer_screen_position: WorkspacePointerPosition,
) -> Result<WorkspaceDragPreview, String> {
    let preview = state
        .drag_start(window_id, tab_id, pointer_screen_position)
        .await?;
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
) -> Result<WorkspaceDragDropResult, String> {
    let intent = match state.drag_drop_prepare(pointer_screen_position).await {
        Ok(intent) => intent,
        Err(error) => {
            let preview = state.drag_cancel().await;
            emit_workspace_drag_preview(&app, &preview);
            return Err(error);
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
                return Err(error);
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

    let snapshots = move_result?;
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
    window_id: String,
    tab_id: String,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state.activate_tab(window_id, tab_id).await?;
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
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state
        .reorder_tab(window_id, dragged_tab_id, target_tab_id, drop_side)
        .await?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_remove(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    window_id: String,
    tab_id: String,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state.remove_tab(window_id, tab_id).await?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn workspace_tab_update_metadata(
    app: AppHandle,
    state: tauri::State<'_, WorkspaceState>,
    tab_id: String,
    patch: WorkspaceTabMetadataPatch,
) -> Result<WorkspaceSnapshot, String> {
    let snapshot = state.update_tab_metadata(tab_id, patch).await?;
    emit_workspace_updated(&app, &snapshot);
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(session_id: &str, window_id: Option<&str>) -> WorkspaceTabRegisterInput {
        WorkspaceTabRegisterInput {
            window_id: window_id.map(str::to_string),
            tab_id: None,
            session_id: session_id.to_string(),
            connection_type: TerminalProtocol::Ssh,
            title: format!("{session_id}-title"),
            encoding: "utf-8".into(),
            terminal_mode: "general".into(),
            is_auto_logging: false,
        }
    }

    #[tokio::test]
    async fn window_registration_returns_empty_snapshot() {
        let state = WorkspaceState::new();
        let snapshot = state
            .register_window("main".into(), "main".into(), true)
            .await;

        assert_eq!(snapshot.window_id, "main");
        assert!(snapshot.tabs.is_empty());
        assert!(snapshot.window.tab_order.is_empty());
    }

    #[tokio::test]
    async fn tab_registration_inserts_once_and_activates() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;

        let snapshot = state.register_tab(input("s1", Some("main"))).await;
        let duplicate = state.register_tab(input("s1", Some("main"))).await;

        assert_eq!(snapshot.window.active_tab_id.as_deref(), Some("s1"));
        assert_eq!(duplicate.window.tab_order, vec!["s1"]);
        assert_eq!(duplicate.tabs.len(), 1);
    }

    #[tokio::test]
    async fn tab_cannot_appear_in_multiple_window_orders() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state
            .register_window("other".into(), "other".into(), true)
            .await;

        state.register_tab(input("s1", Some("main"))).await;
        let other = state.register_tab(input("s1", Some("other"))).await;
        let main = state.snapshot_for_window("main".into()).await;

        assert!(main.window.tab_order.is_empty());
        assert_eq!(other.window.tab_order, vec!["s1"]);
    }

    #[tokio::test]
    async fn same_window_reorder_preserves_single_ownership() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;
        state.register_tab(input("s2", Some("main"))).await;
        state.register_tab(input("s3", Some("main"))).await;

        let snapshot = state
            .reorder_tab("main".into(), "s3".into(), "s1".into(), "before".into())
            .await
            .unwrap();

        assert_eq!(snapshot.window.tab_order, vec!["s3", "s1", "s2"]);
        assert!(snapshot
            .tabs
            .iter()
            .all(|tab| tab.owner_window_id == "main"));
    }

    #[tokio::test]
    async fn tab_move_updates_source_destination_and_owner() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state
            .register_window("other".into(), "other".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;
        state.register_tab(input("s2", Some("other"))).await;

        let snapshots = state
            .move_tab("s1".into(), "main".into(), "other".into(), 0)
            .await
            .unwrap();
        let main = snapshots
            .iter()
            .find(|snapshot| snapshot.window_id == "main")
            .unwrap();
        let other = snapshots
            .iter()
            .find(|snapshot| snapshot.window_id == "other")
            .unwrap();

        assert!(main.window.tab_order.is_empty());
        assert_eq!(other.window.tab_order, vec!["s1", "s2"]);
        assert_eq!(other.tabs[0].owner_window_id, "other");
        assert_eq!(other.window.active_tab_id.as_deref(), Some("s1"));
    }

    #[tokio::test]
    async fn tab_move_to_missing_destination_fails_without_rehoming() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;

        let error = state
            .move_tab("s1".into(), "main".into(), "missing".into(), 0)
            .await
            .unwrap_err();
        let main = state.snapshot_for_window("main".into()).await;

        assert!(error.contains("移動先ウィンドウ"));
        assert_eq!(main.window.tab_order, vec!["s1"]);
        assert_eq!(main.tabs[0].owner_window_id, "main");
        assert!(main.tabs[0].is_connected);
    }

    #[tokio::test]
    async fn drag_start_hover_and_cancel_manage_preview_state() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state
            .register_window("other".into(), "other".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;

        let preview = state
            .drag_start(
                "main".into(),
                "s1".into(),
                WorkspacePointerPosition { x: 10.0, y: 20.0 },
            )
            .await
            .unwrap();

        assert!(preview.active);
        assert_eq!(preview.tab_id.as_deref(), Some("s1"));
        assert_eq!(preview.source_window_id.as_deref(), Some("main"));

        let preview = state.drag_hover("other".into(), Some(1)).await;

        assert_eq!(preview.target_window_id.as_deref(), Some("other"));
        assert_eq!(preview.target_index, Some(1));

        let preview = state.drag_hover("main".into(), None).await;

        assert_eq!(preview.target_window_id.as_deref(), Some("other"));
        assert_eq!(preview.target_index, Some(1));

        let preview = state.drag_hover("other".into(), None).await;

        assert_eq!(preview.target_window_id, None);
        assert_eq!(preview.target_index, None);

        let preview = state.drag_cancel().await;

        assert!(!preview.active);
        assert_eq!(preview.tab_id, None);
    }

    #[tokio::test]
    async fn drag_drop_prepare_clears_drag_without_moving_tab() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state
            .register_window("other".into(), "other".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;

        state
            .drag_start(
                "main".into(),
                "s1".into(),
                WorkspacePointerPosition { x: 10.0, y: 20.0 },
            )
            .await
            .unwrap();
        state.drag_hover("other".into(), Some(0)).await;

        let intent = state
            .drag_drop_prepare(WorkspacePointerPosition { x: 30.0, y: 40.0 })
            .await
            .unwrap();
        let main = state.snapshot_for_window("main".into()).await;
        let preview = state
            .drag_update(WorkspacePointerPosition { x: 50.0, y: 60.0 })
            .await;

        assert_eq!(intent.tab_id, "s1");
        assert_eq!(intent.source_window_id, "main");
        assert_eq!(intent.target_window_id.as_deref(), Some("other"));
        assert_eq!(intent.target_index, 0);
        assert_eq!(main.window.tab_order, vec!["s1"]);
        assert!(!preview.active);
    }

    #[tokio::test]
    async fn closing_non_last_window_rehomes_tabs_to_last_focused_window() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state
            .register_window("other".into(), "other".into(), true)
            .await;
        state
            .register_window("third".into(), "third".into(), true)
            .await;
        state.focus_window("main".into()).await;
        state.register_tab(input("s1", Some("other"))).await;

        let result = state.unregister_window("other".into()).await;
        let main = state.snapshot_for_window("main".into()).await;

        assert_eq!(result.rehome_window_id.as_deref(), Some("main"));
        assert_eq!(result.remaining_window_count, 2);
        assert_eq!(main.window.tab_order, vec!["s1"]);
        assert_eq!(main.tabs[0].owner_window_id, "main");
        assert!(main.tabs[0].is_connected);
    }

    #[tokio::test]
    async fn closing_last_window_does_not_rehome() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;

        let result = state.unregister_window("main".into()).await;

        assert_eq!(result.rehome_window_id, None);
        assert_eq!(result.remaining_window_count, 0);
        assert!(result.snapshots.is_empty());
    }

    #[tokio::test]
    async fn rehome_preserves_log_metadata() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state
            .register_window("other".into(), "other".into(), true)
            .await;
        state.register_tab(input("s1", Some("other"))).await;
        state
            .update_tab_metadata(
                "s1".into(),
                WorkspaceTabMetadataPatch {
                    title: None,
                    encoding: None,
                    terminal_mode: None,
                    is_connected: None,
                    is_auto_logging: Some(true),
                    is_manual_logging: Some(true),
                    is_logging_paused: Some(true),
                    manual_log_file_path: Some("C:\\logs\\s1.log".into()),
                },
            )
            .await
            .unwrap();
        state.focus_window("main".into()).await;

        state.unregister_window("other".into()).await;
        let main = state.snapshot_for_window("main".into()).await;

        assert!(main.tabs[0].is_auto_logging);
        assert!(main.tabs[0].is_manual_logging);
        assert!(main.tabs[0].is_logging_paused);
        assert_eq!(
            main.tabs[0].manual_log_file_path.as_deref(),
            Some("C:\\logs\\s1.log")
        );
    }

    #[tokio::test]
    async fn mark_disconnected_updates_projection() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;

        let snapshot = state.mark_disconnected("s1").await.unwrap();

        assert!(!snapshot.tabs[0].is_connected);
    }

    #[tokio::test]
    async fn removing_active_tab_selects_valid_fallback_or_null() {
        let state = WorkspaceState::new();
        state
            .register_window("main".into(), "main".into(), true)
            .await;
        state.register_tab(input("s1", Some("main"))).await;
        state.register_tab(input("s2", Some("main"))).await;

        let first_remove = state.remove_tab("main".into(), "s2".into()).await.unwrap();
        let second_remove = state.remove_tab("main".into(), "s1".into()).await.unwrap();

        assert_eq!(first_remove.window.active_tab_id.as_deref(), Some("s1"));
        assert_eq!(second_remove.window.active_tab_id, None);
    }
}
