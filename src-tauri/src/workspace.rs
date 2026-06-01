use crate::terminal_control::TerminalProtocol;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

const MAIN_WINDOW_ID: &str = "main";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub window_id: String,
    pub window: WindowWorkspace,
    pub tabs: Vec<WorkspaceTab>,
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
            model.last_focused_window = Some(window_id.clone());
        }
        snapshot_for_locked(&model, &window_id)
    }

    pub async fn focus_window(&self, window_id: String) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        ensure_window(&mut model, &window_id, &window_id);
        model.last_focused_window = Some(window_id.clone());
        snapshot_for_locked(&model, &window_id)
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
        model.last_focused_window = Some(window_id.to_string());
        return window_id.to_string();
    }
    if let Some(window_id) = model.last_focused_window.clone() {
        ensure_window(model, &window_id, &window_id);
        return window_id;
    }
    ensure_window(model, MAIN_WINDOW_ID, MAIN_WINDOW_ID);
    MAIN_WINDOW_ID.to_string()
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

pub fn emit_workspace_updated(app: &AppHandle, snapshot: &WorkspaceSnapshot) {
    if let Err(error) = app.emit("workspace://updated", snapshot) {
        log::warn!("Workspace update event failed: {error}");
    }
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
