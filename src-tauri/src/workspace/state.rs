use super::model::*;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct WorkspaceState {
    model: Arc<Mutex<WorkspaceModel>>,
    closing: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    pending_auto_close: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

impl WorkspaceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn preferred_window_id(&self) -> String {
        let model = self.model.lock().await;
        last_focused_existing_window(&model).unwrap_or_else(|| "main".to_string())
    }

    pub async fn owner_window_id_for_session(&self, session_id: &str) -> Option<String> {
        let model = self.model.lock().await;
        model
            .tabs
            .values()
            .find(|tab| tab.session_id == session_id)
            .map(|tab| tab.owner_window_id.clone())
    }

    pub async fn tab_for_session(&self, session_id: &str) -> Option<WorkspaceTab> {
        let model = self.model.lock().await;
        model
            .tabs
            .values()
            .find(|tab| tab.session_id == session_id)
            .cloned()
    }

    pub async fn activate_session(&self, session_id: &str) -> Option<WorkspaceSnapshot> {
        let mut model = self.model.lock().await;
        let tab = model
            .tabs
            .values()
            .find(|tab| tab.session_id == session_id)?
            .clone();
        let previous = model.clone();
        let window = model.windows.get_mut(&tab.owner_window_id)?;
        if !window.tab_order.contains(&tab.tab_id) {
            return None;
        }
        window.active_tab_id = Some(tab.tab_id);
        advance_revision_if_changed(&mut model, &previous);
        Some(snapshot_for_locked(&model, &tab.owner_window_id))
    }

    pub async fn focus_session_window<F, E>(
        &self,
        session_id: &str,
        window_id: &str,
        tab_id: &str,
        focus: F,
    ) -> Result<bool, E>
    where
        F: FnOnce() -> Result<(), E>,
    {
        let mut model = self.model.lock().await;
        let matches =
            model.tabs.get(tab_id).is_some_and(|tab| {
                tab.session_id == session_id && tab.owner_window_id == window_id
            }) && model
                .windows
                .get(window_id)
                .is_some_and(|window| window.active_tab_id.as_deref() == Some(tab_id));
        if !matches {
            return Ok(false);
        }
        // Keep placement locked until the native focus operation finishes so a moved tab cannot focus its old window.
        focus()?;
        let previous = model.clone();
        set_focused_window(&mut model, window_id);
        advance_revision_if_changed(&mut model, &previous);
        Ok(true)
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
        if model.drag.as_ref().is_some_and(|drag| {
            drag.source_window_id == window_id
                || drag.target_window_id.as_deref() == Some(window_id.as_str())
        }) {
            model.drag = None;
        }
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

    pub async fn connected_session_count(&self) -> usize {
        let model = self.model.lock().await;
        model.tabs.values().filter(|tab| tab.is_connected).count()
    }

    #[cfg(test)]
    pub async fn register_tab(&self, input: WorkspaceTabRegisterInput) -> WorkspaceSnapshot {
        let mut model = self.model.lock().await;
        register_tab_locked(&mut model, input, true, None)
    }

    pub async fn register_session_tab(
        &self,
        input: WorkspaceTabRegisterInput,
        terminals: &crate::terminal_control::TerminalControlState,
    ) -> WorkspaceSnapshot {
        // Hold placement through the status read. Disconnect marks terminal
        // state before taking this lock, so neither ordering can revive a tab.
        let mut model = self.model.lock().await;
        if self
            .closing
            .lock()
            .unwrap()
            .contains(input.tab_id.as_deref().unwrap_or(&input.session_id))
        {
            let owner = model
                .tabs
                .get(input.tab_id.as_deref().unwrap_or(&input.session_id))
                .map(|tab| tab.owner_window_id.as_str())
                .unwrap_or(input.window_id.as_deref().unwrap_or("main"));
            return snapshot_for_locked(&model, owner);
        }
        let session = terminals.workspace_session_state(&input.session_id).await;
        if session.as_ref().is_some_and(|(_, auto_close)| *auto_close) {
            // GUI finalization starts logging before registration and can arrive after EOF cleanup.
            terminals
                .finish_auto_closed_session_logs(&input.session_id)
                .await;
            return snapshot_for_locked(&model, input.window_id.as_deref().unwrap_or("main"));
        }
        let session = session.map(|(info, _)| info);
        let is_connected = session.as_ref().is_none_or(|session| {
            session.status == crate::terminal_control::TerminalStatus::Connected
        });
        let profile_id = session.and_then(|session| session.profile_id);
        register_tab_locked(&mut model, input, is_connected, profile_id)
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
        self.ensure_not_closing(&tab_id)?;
        move_tab_locked(
            &mut model,
            tab_id,
            from_window_id,
            to_window_id,
            target_index,
        )
    }

    pub async fn remove_tab(
        &self,
        window_id: String,
        tab_id: String,
    ) -> Result<WorkspaceSnapshot, String> {
        let mut model = self.model.lock().await;
        self.ensure_not_closing(&tab_id)?;
        remove_tab_locked(&mut model, &window_id, &tab_id)
    }

    fn ensure_not_closing(&self, tab_id: &str) -> Result<(), String> {
        if self.closing.lock().unwrap().contains(tab_id) {
            Err("The tab is being closed".into())
        } else {
            Ok(())
        }
    }

    pub(crate) async fn session_is_closing(&self, session_id: &str) -> bool {
        let model = self.model.lock().await;
        let closing = self.closing.lock().unwrap();
        model
            .tabs
            .values()
            .any(|tab| tab.session_id == session_id && closing.contains(&tab.tab_id))
    }

    pub(crate) async fn close_tab_owned<F, Fut, P>(
        &self,
        window_id: String,
        tab_id: String,
        close: F,
        publish: P,
    ) -> Result<WorkspaceSnapshot, String>
    where
        F: FnOnce(WorkspaceTab) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
        P: FnOnce(&WorkspaceSnapshot) + Send + 'static,
    {
        let state = self.clone();
        // Disconnect coordinators can outlive IPC cancellation. Keep the reservation and
        // snapshot publication with an owned task until teardown has actually completed.
        tokio::spawn(async move {
            let snapshot = state.close_tab_with(&window_id, &tab_id, close).await?;
            publish(&snapshot);
            Ok(snapshot)
        })
        .await
        .map_err(|_| "Tab close task did not complete".to_string())?
    }

    pub(crate) async fn close_tab_with<F, Fut>(
        &self,
        window_id: &str,
        tab_id: &str,
        close: F,
    ) -> Result<WorkspaceSnapshot, String>
    where
        F: FnOnce(WorkspaceTab) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let (tab, reservation) = {
            let model = self.model.lock().await;
            validate_tab_owner(&model, window_id, tab_id)?;
            self.ensure_not_closing(tab_id)?;
            self.closing.lock().unwrap().insert(tab_id.to_string());
            (
                model.tabs[tab_id].clone(),
                ClosingTabGuard {
                    closing: self.closing.clone(),
                    pending_auto_close: self.pending_auto_close.clone(),
                    tab_id: tab_id.to_string(),
                },
            )
        };
        let close_result = close(tab).await;
        let mut model = self.model.lock().await;
        if let Err(error) = close_result {
            if !self.pending_auto_close.lock().unwrap().contains(tab_id) {
                return Err(error);
            }
            // A confirmed disconnect still requires removal if the overlapping GUI close failed.
            log::warn!("Explicit tab cleanup failed after an automatic disconnect close");
        }
        // Window shutdown can rehome an accepted close; the reservation follows the tab.
        let owner = model
            .tabs
            .get(tab_id)
            .ok_or("Tab not found")?
            .owner_window_id
            .clone();
        let result = remove_tab_locked(&mut model, &owner, tab_id);
        // Release the reservation before unlocking placement, so a registration cannot
        // observe a closing tab whose model entry has already been removed.
        drop(reservation);
        result
    }

    pub(super) async fn detach_tab_with<F>(
        &self,
        tab_id: String,
        from_window_id: String,
        window_id: String,
        create: F,
    ) -> Result<Vec<WorkspaceSnapshot>, String>
    where
        F: FnOnce() -> Result<(), String>,
    {
        let mut model = self.model.lock().await;
        validate_tab_owner(&model, &from_window_id, &tab_id)?;
        self.ensure_not_closing(&tab_id)?;
        if model.windows.contains_key(&window_id) {
            return Err("Destination window already exists".into());
        }
        // Native creation is synchronous. Keeping placement locked removes the fallible move gap
        // without rolling back a window that another operation may already have populated.
        create()?;
        ensure_window(&mut model, &window_id, &window_id);
        set_focused_window(&mut model, &window_id);
        move_tab_locked(&mut model, tab_id, from_window_id, window_id, 0)
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

    pub async fn sync_log_state(
        &self,
        session_id: &str,
        active: bool,
        paused: bool,
        file_path: Option<String>,
    ) -> Option<WorkspaceSnapshot> {
        let mut model = self.model.lock().await;
        let previous = model.clone();
        let owner = {
            let tab = model
                .tabs
                .values_mut()
                .find(|tab| tab.session_id == session_id)?;
            tab.is_manual_logging = active;
            tab.is_manual_logging_paused = active && paused;
            tab.manual_log_file_path = if active { file_path } else { None };
            tab.owner_window_id.clone()
        };
        advance_revision_if_changed(&mut model, &previous);
        Some(snapshot_for_locked(&model, &owner))
    }

    pub async fn mark_disconnected(
        &self,
        session_id: &str,
        auto_close: bool,
    ) -> Option<WorkspaceSnapshot> {
        let mut model = self.model.lock().await;
        let previous = model.clone();
        let tab = model
            .tabs
            .values()
            .find(|tab| tab.session_id == session_id)?;
        if auto_close {
            if self.closing.lock().unwrap().contains(&tab.tab_id) {
                self.pending_auto_close
                    .lock()
                    .unwrap()
                    .insert(tab.tab_id.clone());
            } else {
                let owner = tab.owner_window_id.clone();
                let tab_id = tab.tab_id.clone();
                return remove_tab_locked(&mut model, &owner, &tab_id).ok();
            }
        }
        let owner_window_id = model
            .tabs
            .values_mut()
            .find(|tab| tab.session_id == session_id)
            .map(|tab| {
                tab.is_connected = false;
                if !tab.is_manual_logging {
                    tab.is_manual_logging_paused = false;
                }
                tab.owner_window_id.clone()
            })?;
        advance_revision_if_changed(&mut model, &previous);
        (*model != previous).then(|| snapshot_for_locked(&model, &owner_window_id))
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

fn register_tab_locked(
    model: &mut WorkspaceModel,
    input: WorkspaceTabRegisterInput,
    is_connected: bool,
    profile_id: Option<String>,
) -> WorkspaceSnapshot {
    let previous = model.clone();
    let owner_window_id = choose_owner_window(model, input.window_id.as_deref());
    let tab_id = input.tab_id.unwrap_or_else(|| input.session_id.clone());
    remove_tab_from_all_windows(model, &tab_id);

    let tab = WorkspaceTab {
        tab_id: tab_id.clone(),
        session_id: input.session_id,
        profile_id,
        connection_type: input.connection_type,
        title: input.title,
        owner_window_id: owner_window_id.clone(),
        encoding: input.encoding,
        terminal_mode: input.terminal_mode,
        connection_info: input.connection_info,
        is_connected,
        is_manual_logging: input.is_manual_logging,
        is_manual_logging_paused: false,
        manual_log_file_path: if input.is_manual_logging {
            input.manual_log_file_path
        } else {
            None
        },
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

    advance_revision_if_changed(model, &previous);
    let mut snapshot = snapshot_for_locked(model, &owner_window_id);
    snapshot.tab_update = Some(WorkspaceTabUpdate::Connected { tab_id });
    snapshot
}

struct ClosingTabGuard {
    closing: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    pending_auto_close: Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    tab_id: String,
}
impl Drop for ClosingTabGuard {
    fn drop(&mut self) {
        self.closing.lock().unwrap().remove(&self.tab_id);
        self.pending_auto_close.lock().unwrap().remove(&self.tab_id);
    }
}

fn validate_tab_owner(model: &WorkspaceModel, window_id: &str, tab_id: &str) -> Result<(), String> {
    let window = model.windows.get(window_id).ok_or("Window not found")?;
    let tab = model.tabs.get(tab_id).ok_or("Tab not found")?;
    if tab.owner_window_id != window_id || !window.tab_order.iter().any(|id| id == tab_id) {
        return Err("The tab does not belong to this window".into());
    }
    Ok(())
}

fn remove_tab_locked(
    model: &mut WorkspaceModel,
    window_id: &str,
    tab_id: &str,
) -> Result<WorkspaceSnapshot, String> {
    validate_tab_owner(model, window_id, tab_id)?;
    let previous = model.clone();
    model.tabs.remove(tab_id);
    remove_tab_from_window(model.windows.get_mut(window_id).unwrap(), tab_id);
    if model
        .drag
        .as_ref()
        .is_some_and(|drag| drag.tab_id == tab_id)
    {
        model.drag = None;
    }
    advance_revision_if_changed(model, &previous);
    Ok(snapshot_for_locked(model, window_id))
}

fn move_tab_locked(
    model: &mut WorkspaceModel,
    tab_id: String,
    from_window_id: String,
    to_window_id: String,
    target_index: usize,
) -> Result<Vec<WorkspaceSnapshot>, String> {
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
    if model
        .drag
        .as_ref()
        .is_some_and(|drag| drag.tab_id == tab_id)
    {
        model.drag = None;
    }

    if from_window_id == to_window_id {
        let window = model
            .windows
            .get_mut(&from_window_id)
            .ok_or_else(|| "Window not found".to_string())?;
        window.tab_order.retain(|id| id != &tab_id);
        let insert_index = target_index.min(window.tab_order.len());
        window.tab_order.insert(insert_index, tab_id.clone());
        advance_revision_if_changed(model, &previous);
        let mut snapshot = snapshot_for_locked(model, &from_window_id);
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

    advance_revision_if_changed(model, &previous);
    let source_snapshot = snapshot_for_locked(model, &from_window_id);
    let mut destination_snapshot = snapshot_for_locked(model, &to_window_id);
    destination_snapshot.tab_update = Some(WorkspaceTabUpdate::Moved {
        tab_id,
        target_index: destination_target_index,
    });

    Ok(vec![source_snapshot, destination_snapshot])
}
