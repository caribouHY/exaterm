use crate::terminal_control::TerminalProtocol;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(any(debug_assertions, test))]
use std::collections::HashSet;

const MAIN_WINDOW_ID: &str = "main";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub revision: u64,
    pub window_id: String,
    pub window: WindowWorkspace,
    pub tabs: Vec<WorkspaceTab>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_update: Option<WorkspaceTabUpdate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceTabUpdate {
    Connected { tab_id: String },
    Moved { tab_id: String, target_index: usize },
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
    pub connection_info: Option<WorkspaceConnectionInfo>,
    pub is_connected: bool,
    pub is_manual_logging: bool,
    pub is_manual_logging_paused: bool,
    pub manual_log_file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceConnectionInfo {
    Ssh {
        host: String,
        port: u16,
        username: String,
        auth_method: String,
        private_key_path: Option<String>,
        jump_profile_id: Option<String>,
    },
    Telnet {
        host: String,
        port: u16,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTabMetadataPatch {
    pub title: Option<String>,
    pub encoding: Option<String>,
    pub terminal_mode: Option<String>,
    pub is_connected: Option<bool>,
    pub is_manual_logging: Option<bool>,
    pub is_manual_logging_paused: Option<bool>,
    pub manual_log_file_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct WorkspaceModel {
    pub(super) revision: u64,
    pub(super) windows: HashMap<String, WindowWorkspace>,
    pub(super) tabs: HashMap<String, WorkspaceTab>,
    pub(super) last_focused_window: Option<String>,
    pub(super) focused_window_order: Vec<String>,
    pub(super) drag: Option<WorkspaceDragSession>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct WorkspaceDragSession {
    pub(super) tab_id: String,
    pub(super) source_window_id: String,
    pub(super) pointer_screen_position: WorkspacePointerPosition,
    pub(super) target_window_id: Option<String>,
    pub(super) target_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct WorkspaceDragDropIntent {
    pub(super) tab_id: String,
    pub(super) source_window_id: String,
    pub(super) target_window_id: Option<String>,
    pub(super) target_index: usize,
}

impl WorkspaceModel {
    #[cfg(any(debug_assertions, test))]
    pub(super) fn validate_invariants(&self) -> Result<(), String> {
        let mut ordered_tab_ids = HashSet::new();

        for (window_id, window) in &self.windows {
            if window.window_id != *window_id {
                return Err("Window map key does not match the window ID".to_string());
            }

            let mut window_tab_ids = HashSet::new();
            for tab_id in &window.tab_order {
                if !window_tab_ids.insert(tab_id) {
                    return Err("A window tab order contains a duplicate tab".to_string());
                }
                if !ordered_tab_ids.insert(tab_id) {
                    return Err("A tab appears in more than one window".to_string());
                }

                let tab = self
                    .tabs
                    .get(tab_id)
                    .ok_or_else(|| "A window references an unknown tab".to_string())?;
                if tab.owner_window_id != *window_id {
                    return Err("A tab owner does not match its window".to_string());
                }
            }

            if let Some(active_tab_id) = &window.active_tab_id {
                if !window_tab_ids.contains(active_tab_id) {
                    return Err("A window active tab is not in its tab order".to_string());
                }
            }
        }

        if !self.windows.is_empty() {
            for (tab_id, tab) in &self.tabs {
                if tab.tab_id != *tab_id {
                    return Err("Tab map key does not match the tab ID".to_string());
                }
                if !self.windows.contains_key(&tab.owner_window_id) {
                    return Err("A tab owner window is not registered".to_string());
                }
                if !ordered_tab_ids.contains(tab_id) {
                    return Err("A tab is not present in its owner window order".to_string());
                }
            }
        }

        if let Some(last_focused_window) = &self.last_focused_window {
            if !self.windows.contains_key(last_focused_window) {
                return Err("The last focused window is not registered".to_string());
            }
        }

        let mut focused_window_ids = HashSet::new();
        for window_id in &self.focused_window_order {
            if !self.windows.contains_key(window_id) {
                return Err("The focus order references an unknown window".to_string());
            }
            if !focused_window_ids.insert(window_id) {
                return Err("The focus order contains a duplicate window".to_string());
            }
        }

        if let Some(drag) = &self.drag {
            let tab = self
                .tabs
                .get(&drag.tab_id)
                .ok_or_else(|| "The drag session references an unknown tab".to_string())?;
            let source_window = self
                .windows
                .get(&drag.source_window_id)
                .ok_or_else(|| "The drag source window is not registered".to_string())?;
            if tab.owner_window_id != drag.source_window_id
                || !source_window.tab_order.contains(&drag.tab_id)
            {
                return Err("The drag source does not own the dragged tab".to_string());
            }
            match (&drag.target_window_id, drag.target_index) {
                (Some(window_id), Some(_)) if self.windows.contains_key(window_id) => {}
                (None, None) => {}
                _ => return Err("The drag target is inconsistent".to_string()),
            }
        }

        Ok(())
    }
}

#[cfg(debug_assertions)]
fn debug_assert_invariants(model: &WorkspaceModel) {
    if let Err(error) = model.validate_invariants() {
        panic!("Workspace model invariant violation: {error}");
    }
}

#[cfg(not(debug_assertions))]
fn debug_assert_invariants(_: &WorkspaceModel) {}

#[derive(Debug, Clone)]
pub struct WorkspaceTabRegisterInput {
    pub window_id: Option<String>,
    pub tab_id: Option<String>,
    pub session_id: String,
    pub connection_type: TerminalProtocol,
    pub title: String,
    pub encoding: String,
    pub terminal_mode: String,
    pub connection_info: Option<WorkspaceConnectionInfo>,
    pub is_manual_logging: bool,
    pub manual_log_file_path: Option<String>,
}

pub(super) fn apply_metadata_patch(tab: &mut WorkspaceTab, patch: WorkspaceTabMetadataPatch) {
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
    if let Some(is_manual_logging) = patch.is_manual_logging {
        tab.is_manual_logging = is_manual_logging;
    }
    if let Some(is_manual_logging_paused) = patch.is_manual_logging_paused {
        tab.is_manual_logging_paused = is_manual_logging_paused;
    }
    if let Some(manual_log_file_path) = patch.manual_log_file_path {
        tab.manual_log_file_path = Some(manual_log_file_path);
    }
    if !tab.is_manual_logging {
        tab.is_manual_logging_paused = false;
        tab.manual_log_file_path = None;
    }
}

pub(super) fn choose_owner_window(
    model: &mut WorkspaceModel,
    requested_window_id: Option<&str>,
) -> String {
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

pub(super) fn set_focused_window(model: &mut WorkspaceModel, window_id: &str) {
    model.last_focused_window = Some(window_id.to_string());
    model
        .focused_window_order
        .retain(|focused_window_id| focused_window_id != window_id);
    model.focused_window_order.push(window_id.to_string());
}

pub(super) fn remove_focused_window(model: &mut WorkspaceModel, window_id: &str) {
    model
        .focused_window_order
        .retain(|focused_window_id| focused_window_id != window_id);
    model.last_focused_window = last_focused_existing_window(model);
}

pub(super) fn last_focused_existing_window(model: &WorkspaceModel) -> Option<String> {
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

pub(super) fn choose_rehome_window(model: &WorkspaceModel) -> Option<String> {
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

pub(super) fn ensure_window(model: &mut WorkspaceModel, window_id: &str, label: &str) {
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

pub(super) fn remove_tab_from_all_windows(model: &mut WorkspaceModel, tab_id: &str) {
    for window in model.windows.values_mut() {
        remove_tab_from_window(window, tab_id);
    }
}

pub(super) fn remove_tab_from_window(window: &mut WindowWorkspace, tab_id: &str) {
    let Some(removed_index) = window.tab_order.iter().position(|id| id == tab_id) else {
        return;
    };
    let removed_active_tab = window.active_tab_id.as_deref() == Some(tab_id);
    window.tab_order.remove(removed_index);
    if removed_active_tab {
        window.active_tab_id = window
            .tab_order
            .get(removed_index)
            .or_else(|| {
                removed_index
                    .checked_sub(1)
                    .and_then(|index| window.tab_order.get(index))
            })
            .cloned();
    }
}

pub(super) fn advance_revision_if_changed(model: &mut WorkspaceModel, previous: &WorkspaceModel) {
    if model != previous {
        advance_revision(model);
    }
}

pub(super) fn advance_revision(model: &mut WorkspaceModel) {
    model.revision = model
        .revision
        .checked_add(1)
        .expect("workspace revision overflowed");
}

pub(super) fn snapshot_for_locked(model: &WorkspaceModel, window_id: &str) -> WorkspaceSnapshot {
    debug_assert_invariants(model);
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
        revision: model.revision,
        window_id: window_id.to_string(),
        window,
        tabs,
        tab_update: None,
    }
}

pub(super) fn drag_preview_for_locked(model: &WorkspaceModel) -> WorkspaceDragPreview {
    debug_assert_invariants(model);
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
