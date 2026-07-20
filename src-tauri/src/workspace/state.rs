use super::model::*;
use std::sync::Arc;
use tokio::sync::Mutex;

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
        let previous = model.clone();
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
        advance_revision_if_changed(&mut model, &previous);
        snapshot_for_locked(&model, &window_id)
    }

    pub async fn focus_window(&self, window_id: String) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        let previous = model.clone();
        ensure_window(&mut model, &window_id, &window_id);
        set_focused_window(&mut model, &window_id);
        advance_revision_if_changed(&mut model, &previous);
        snapshot_for_locked(&model, &window_id)
    }

    pub async fn unregister_window(&self, window_id: String) -> WorkspaceWindowCloseResult {
        let mut model = self.model.lock().await;
        let previous = model.clone();
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

                    rehome_window_id = Some(destination_window_id);
                }
            }
        }

        advance_revision_if_changed(&mut model, &previous);
        if let Some(destination_window_id) = &rehome_window_id {
            snapshots.push(snapshot_for_locked(&model, destination_window_id));
        }

        WorkspaceWindowCloseResult {
            window_id,
            rehome_window_id,
            remaining_window_count: model.windows.len(),
            snapshots,
        }
    }

    pub async fn snapshot_for_window(&self, window_id: String) -> WorkspaceSnapshot {
        let model = self.model.lock().await;
        snapshot_for_locked(&model, &window_id)
    }

    pub async fn register_tab(&self, input: WorkspaceTabRegisterInput) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        let previous = model.clone();
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
            connection_info: input.connection_info,
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
        window.active_tab_id = Some(tab_id.clone());

        advance_revision_if_changed(&mut model, &previous);
        let mut snapshot = snapshot_for_locked(&model, &owner_window_id);
        snapshot.tab_update = Some(WorkspaceTabUpdate::Connected { tab_id });
        snapshot
    }

    pub async fn activate_tab(
        &self,
        window_id: String,
        tab_id: String,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        let previous = model.clone();
        let window = model
            .windows
            .get_mut(&window_id)
            .ok_or_else(|| "Window not found".to_string())?;
        if !window.tab_order.contains(&tab_id) {
            return Err("The tab does not belong to this window".to_string());
        }
        window.active_tab_id = Some(tab_id);
        model.last_focused_window = Some(window_id.clone());
        advance_revision_if_changed(&mut model, &previous);
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
        let previous = model.clone();
        let window = model
            .windows
            .get_mut(&window_id)
            .ok_or_else(|| "Window not found".to_string())?;
        if dragged_tab_id == target_tab_id {
            return Ok(snapshot_for_locked(&model, &window_id));
        }
        let dragged_index = window
            .tab_order
            .iter()
            .position(|id| id == &dragged_tab_id)
            .ok_or_else(|| "Source tab not found".to_string())?;
        let _target_index = window
            .tab_order
            .iter()
            .position(|id| id == &target_tab_id)
            .ok_or_else(|| "Destination tab not found".to_string())?;

        let dragged = window.tab_order.remove(dragged_index);
        let target_index_after_removal = window
            .tab_order
            .iter()
            .position(|id| id == &target_tab_id)
            .ok_or_else(|| "Destination tab not found".to_string())?;
        let insert_index = if drop_side == "after" {
            target_index_after_removal + 1
        } else {
            target_index_after_removal
        };
        window.tab_order.insert(insert_index, dragged);
        advance_revision_if_changed(&mut model, &previous);
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
            return Err("Destination window not found".to_string());
        }

        let source_window = model
            .windows
            .get(&from_window_id)
            .ok_or_else(|| "Source window not found".to_string())?;
        if !source_window.tab_order.contains(&tab_id) {
            return Err("Source tab not found".to_string());
        }

        let previous = model.clone();

        let tab = model
            .tabs
            .get_mut(&tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        if tab.owner_window_id != from_window_id {
            return Err("The tab owner window does not match".to_string());
        }
        tab.owner_window_id = to_window_id.clone();

        if from_window_id == to_window_id {
            let window = model
                .windows
                .get_mut(&from_window_id)
                .ok_or_else(|| "Window not found".to_string())?;
            window.tab_order.retain(|id| id != &tab_id);
            let insert_index = target_index.min(window.tab_order.len());
            window.tab_order.insert(insert_index, tab_id.clone());
            advance_revision_if_changed(&mut model, &previous);
            let mut snapshot = snapshot_for_locked(&model, &from_window_id);
            snapshot.tab_update = Some(WorkspaceTabUpdate::Moved {
                tab_id,
                target_index: insert_index,
            });
            return Ok(vec![snapshot]);
        }

        {
            let source = model
                .windows
                .get_mut(&from_window_id)
                .ok_or_else(|| "Source window not found".to_string())?;
            remove_tab_from_window(source, &tab_id);
        }

        let destination_target_index;
        {
            let destination = model
                .windows
                .get_mut(&to_window_id)
                .ok_or_else(|| "Destination window not found".to_string())?;
            destination.tab_order.retain(|id| id != &tab_id);
            let insert_index = target_index.min(destination.tab_order.len());
            destination_target_index = insert_index;
            destination.tab_order.insert(insert_index, tab_id.clone());
            destination.active_tab_id = Some(tab_id.clone());
        }

        advance_revision_if_changed(&mut model, &previous);
        let source_snapshot = snapshot_for_locked(&model, &from_window_id);
        let mut destination_snapshot = snapshot_for_locked(&model, &to_window_id);
        destination_snapshot.tab_update = Some(WorkspaceTabUpdate::Moved {
            tab_id,
            target_index: destination_target_index,
        });

        Ok(vec![source_snapshot, destination_snapshot])
    }

    pub async fn validate_tab_move_source(
        &self,
        tab_id: &str,
        from_window_id: &str,
    ) -> Result<(), String> {
        let model = self.model.lock().await;
        let source_window = model
            .windows
            .get(from_window_id)
            .ok_or_else(|| "Source window not found".to_string())?;
        if !source_window.tab_order.iter().any(|id| id == tab_id) {
            return Err("Source tab not found".to_string());
        }
        let tab = model
            .tabs
            .get(tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        if tab.owner_window_id != from_window_id {
            return Err("The tab owner window does not match".to_string());
        }
        Ok(())
    }

    pub async fn remove_tab(
        &self,
        window_id: String,
        tab_id: String,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        if !model.windows.contains_key(&window_id) {
            return Err("Window not found".to_string());
        }
        let previous = model.clone();
        model.tabs.remove(&tab_id);
        let window = model
            .windows
            .get_mut(&window_id)
            .ok_or_else(|| "Window not found".to_string())?;
        remove_tab_from_window(window, &tab_id);
        advance_revision_if_changed(&mut model, &previous);
        Ok(snapshot_for_locked(&model, &window_id))
    }

    pub async fn update_tab_metadata(
        &self,
        tab_id: String,
        patch: WorkspaceTabMetadataPatch,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        let previous = model.clone();
        let owner_window_id = {
            let tab = model
                .tabs
                .get_mut(&tab_id)
                .ok_or_else(|| "Tab not found".to_string())?;
            apply_metadata_patch(tab, patch);
            tab.owner_window_id.clone()
        };
        advance_revision_if_changed(&mut model, &previous);
        Ok(snapshot_for_locked(&model, &owner_window_id))
    }

    pub async fn mark_disconnected(&self, session_id: &str) -> Option<WorkspaceSnapshot> {
        let mut model = self.model.lock().await;
        let previous = model.clone();
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
        advance_revision_if_changed(&mut model, &previous);
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
            .ok_or_else(|| "Window not found".to_string())?;
        if !window.tab_order.contains(&tab_id) {
            return Err("The tab does not belong to this window".to_string());
        }
        let tab = model
            .tabs
            .get(&tab_id)
            .ok_or_else(|| "Tab not found".to_string())?;
        if tab.owner_window_id != window_id {
            return Err("The tab owner window does not match".to_string());
        }

        let previous_drag = model.drag.clone();
        model.drag = Some(WorkspaceDragSession {
            tab_id,
            source_window_id: window_id,
            pointer_screen_position,
            target_window_id: None,
            target_index: None,
        });

        if model.drag != previous_drag {
            advance_revision(&mut model);
        }
        Ok(drag_preview_for_locked(&model))
    }

    pub async fn drag_update(
        &self,
        pointer_screen_position: WorkspacePointerPosition,
    ) -> WorkspaceDragPreview {
        let mut model = self.model.lock().await;
        let previous_drag = model.drag.clone();
        if let Some(drag) = model.drag.as_mut() {
            drag.pointer_screen_position = pointer_screen_position;
        }
        if model.drag != previous_drag {
            advance_revision(&mut model);
        }
        drag_preview_for_locked(&model)
    }

    pub async fn drag_hover(
        &self,
        window_id: String,
        target_index: Option<usize>,
    ) -> WorkspaceDragPreview {
        let mut model = self.model.lock().await;
        let previous_drag = model.drag.clone();
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
        if model.drag != previous_drag {
            advance_revision(&mut model);
        }
        drag_preview_for_locked(&model)
    }

    pub async fn drag_cancel(&self) -> WorkspaceDragPreview {
        let mut model = self.model.lock().await;
        if model.drag.take().is_some() {
            advance_revision(&mut model);
        }
        drag_preview_for_locked(&model)
    }

    pub(super) async fn drag_drop_prepare(
        &self,
        pointer_screen_position: WorkspacePointerPosition,
    ) -> Result<WorkspaceDragDropIntent, String> {
        let mut model = self.model.lock().await;
        let mut drag = model
            .drag
            .clone()
            .ok_or_else(|| "No tab is currently being dragged".to_string())?;
        drag.pointer_screen_position = pointer_screen_position;
        if !model.tabs.contains_key(&drag.tab_id) {
            return Err("Tab not found".to_string());
        }
        if !model.windows.contains_key(&drag.source_window_id) {
            return Err("Source window not found".to_string());
        }

        model.drag = None;
        advance_revision(&mut model);

        Ok(WorkspaceDragDropIntent {
            tab_id: drag.tab_id,
            source_window_id: drag.source_window_id,
            target_window_id: drag.target_window_id,
            target_index: drag.target_index.unwrap_or(0),
        })
    }
}
