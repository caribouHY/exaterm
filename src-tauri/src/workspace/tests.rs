use super::model::*;
use super::state::WorkspaceState;
use crate::terminal_control::TerminalProtocol;

fn input(session_id: &str, window_id: Option<&str>) -> WorkspaceTabRegisterInput {
    WorkspaceTabRegisterInput {
        window_id: window_id.map(str::to_string),
        tab_id: None,
        session_id: session_id.to_string(),
        connection_type: TerminalProtocol::Ssh,
        title: format!("{session_id}-title"),
        encoding: "utf-8".into(),
        terminal_mode: "general".into(),
        connection_info: None,
        is_auto_logging: false,
    }
}

fn model_with_tab() -> WorkspaceModel {
    let mut model = WorkspaceModel::default();
    ensure_window(&mut model, "main", "main");
    model.tabs.insert(
        "s1".into(),
        WorkspaceTab {
            tab_id: "s1".into(),
            session_id: "s1".into(),
            connection_type: TerminalProtocol::Ssh,
            title: "s1-title".into(),
            owner_window_id: "main".into(),
            encoding: "utf-8".into(),
            terminal_mode: "general".into(),
            connection_info: None,
            is_connected: true,
            is_auto_logging: false,
            is_manual_logging: false,
            is_logging_paused: false,
            manual_log_file_path: None,
        },
    );
    let window = model.windows.get_mut("main").unwrap();
    window.tab_order.push("s1".into());
    window.active_tab_id = Some("s1".into());
    set_focused_window(&mut model, "main");
    model
}

#[test]
fn workspace_model_invariants_accept_consistent_state() {
    assert_eq!(model_with_tab().validate_invariants(), Ok(()));
}

#[test]
fn workspace_model_invariants_reject_duplicate_tab_ownership() {
    let mut model = model_with_tab();
    ensure_window(&mut model, "other", "other");
    model
        .windows
        .get_mut("other")
        .unwrap()
        .tab_order
        .push("s1".into());

    assert!(model.validate_invariants().is_err());
}

#[test]
fn workspace_model_invariants_reject_invalid_active_and_focus_state() {
    let mut model = model_with_tab();
    model.windows.get_mut("main").unwrap().active_tab_id = Some("missing".into());
    assert!(model
        .validate_invariants()
        .unwrap_err()
        .contains("active tab"));

    let mut model = model_with_tab();
    model.focused_window_order.push("missing".into());
    assert!(model
        .validate_invariants()
        .unwrap_err()
        .contains("focus order"));
}

#[test]
fn workspace_model_invariants_reject_invalid_drag_source() {
    let mut model = model_with_tab();
    model.drag = Some(WorkspaceDragSession {
        tab_id: "s1".into(),
        source_window_id: "missing".into(),
        pointer_screen_position: WorkspacePointerPosition { x: 0.0, y: 0.0 },
        target_window_id: None,
        target_index: None,
    });

    assert!(model
        .validate_invariants()
        .unwrap_err()
        .contains("source window"));
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
async fn preferred_window_tracks_the_last_focused_existing_window() {
    let state = WorkspaceState::new();
    assert_eq!(state.preferred_window_id().await, "main");

    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state
        .register_window("other".into(), "other".into(), true)
        .await;
    assert_eq!(state.preferred_window_id().await, "other");

    state.unregister_window("other".into()).await;
    assert_eq!(state.preferred_window_id().await, "main");
}

#[tokio::test]
async fn connected_session_count_spans_windows_and_ignores_disconnected_tabs() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state.register_tab(input("s1", Some("main"))).await;
    state
        .register_window("other".into(), "other".into(), true)
        .await;
    state.register_tab(input("s2", Some("other"))).await;

    assert_eq!(state.connected_session_count().await, 2);
    state.mark_disconnected("s1").await;
    assert_eq!(state.connected_session_count().await, 1);
}

#[tokio::test]
async fn workspace_revision_increases_only_when_model_changes() {
    let state = WorkspaceState::new();
    let initial = state.snapshot_for_window("main".into()).await;
    let registered = state
        .register_window("main".into(), "main".into(), true)
        .await;
    let duplicate_registration = state
        .register_window("main".into(), "main".into(), true)
        .await;
    let tab_registered = state.register_tab(input("s1", Some("main"))).await;
    let duplicate_activation = state
        .activate_tab("main".into(), "s1".into())
        .await
        .unwrap();
    let metadata_updated = state
        .update_tab_metadata(
            "s1".into(),
            WorkspaceTabMetadataPatch {
                title: Some("renamed".into()),
                encoding: None,
                terminal_mode: None,
                is_connected: None,
                is_auto_logging: None,
                is_manual_logging: None,
                is_logging_paused: None,
                manual_log_file_path: None,
            },
        )
        .await
        .unwrap();
    state
        .drag_start(
            "main".into(),
            "s1".into(),
            WorkspacePointerPosition { x: 10.0, y: 20.0 },
        )
        .await
        .unwrap();
    let drag_started = state.snapshot_for_window("main".into()).await;
    state
        .drag_update(WorkspacePointerPosition { x: 10.0, y: 20.0 })
        .await;
    let duplicate_drag_update = state.snapshot_for_window("main".into()).await;
    state.drag_cancel().await;
    let drag_cancelled = state.snapshot_for_window("main".into()).await;

    assert_eq!(initial.revision, 0);
    assert!(registered.revision > initial.revision);
    assert_eq!(duplicate_registration.revision, registered.revision);
    assert!(tab_registered.revision > registered.revision);
    assert_eq!(duplicate_activation.revision, tab_registered.revision);
    assert!(metadata_updated.revision > tab_registered.revision);
    assert!(drag_started.revision > metadata_updated.revision);
    assert_eq!(duplicate_drag_update.revision, drag_started.revision);
    assert!(drag_cancelled.revision > drag_started.revision);
}

#[tokio::test]
async fn read_only_snapshots_do_not_advance_revision() {
    let state = WorkspaceState::new();
    let registered = state
        .register_window("main".into(), "main".into(), true)
        .await;

    let first = state.snapshot_for_window("main".into()).await;
    let second = state.snapshot_for_window("main".into()).await;
    let missing_window = state.snapshot_for_window("missing".into()).await;

    assert_eq!(first.revision, registered.revision);
    assert_eq!(second.revision, registered.revision);
    assert_eq!(missing_window.revision, registered.revision);
}

#[tokio::test]
async fn connection_info_survives_workspace_move() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state
        .register_window("other".into(), "other".into(), false)
        .await;
    let mut ssh_input = input("s1", Some("main"));
    ssh_input.connection_info = Some(WorkspaceConnectionInfo::Ssh {
        host: "example.invalid".into(),
        port: 2222,
        username: "operator".into(),
        auth_method: "public_key".into(),
        private_key_path: Some("C:\\keys\\id_ed25519".into()),
        jump_profile_id: Some("bastion".into()),
    });
    state.register_tab(ssh_input).await;

    let snapshots = state
        .move_tab("s1".into(), "main".into(), "other".into(), 0)
        .await
        .unwrap();
    let moved = snapshots
        .iter()
        .find(|snapshot| snapshot.window_id == "other")
        .unwrap();

    assert_eq!(
        moved.tabs[0].connection_info,
        Some(WorkspaceConnectionInfo::Ssh {
            host: "example.invalid".into(),
            port: 2222,
            username: "operator".into(),
            auth_method: "public_key".into(),
            private_key_path: Some("C:\\keys\\id_ed25519".into()),
            jump_profile_id: Some("bastion".into()),
        })
    );
}

#[tokio::test]
async fn workspace_supports_telnet_info_and_serial_without_info() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    let mut telnet_input = input("t1", Some("main"));
    telnet_input.connection_type = TerminalProtocol::Telnet;
    telnet_input.connection_info = Some(WorkspaceConnectionInfo::Telnet {
        host: "example.invalid".into(),
        port: 2323,
    });
    state.register_tab(telnet_input).await;
    let mut serial_input = input("serial1", Some("main"));
    serial_input.connection_type = TerminalProtocol::Serial;
    state.register_tab(serial_input).await;

    let snapshot = state.snapshot_for_window("main".into()).await;
    let telnet = snapshot
        .tabs
        .iter()
        .find(|tab| tab.session_id == "t1")
        .unwrap();
    let serial = snapshot
        .tabs
        .iter()
        .find(|tab| tab.session_id == "serial1")
        .unwrap();
    assert_eq!(
        telnet.connection_info,
        Some(WorkspaceConnectionInfo::Telnet {
            host: "example.invalid".into(),
            port: 2323,
        })
    );
    assert_eq!(serial.connection_info, None);
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
    assert_eq!(
        snapshot.tab_update,
        Some(WorkspaceTabUpdate::Connected {
            tab_id: "s1".into()
        })
    );
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
    assert_eq!(snapshot.window.active_tab_id.as_deref(), Some("s3"));
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
    let before_move_revision = state.snapshot_for_window("main".into()).await.revision;

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
    assert!(main.revision > before_move_revision);
    assert_eq!(main.revision, other.revision);
    assert_eq!(main.tab_update, None);
    assert_eq!(
        other.tab_update,
        Some(WorkspaceTabUpdate::Moved {
            tab_id: "s1".into(),
            target_index: 0
        })
    );
}

#[tokio::test]
async fn moving_active_middle_tab_selects_right_neighbor_in_source() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state
        .register_window("other".into(), "other".into(), false)
        .await;
    state.register_tab(input("s1", Some("main"))).await;
    state.register_tab(input("s2", Some("main"))).await;
    state.register_tab(input("s3", Some("main"))).await;
    state
        .activate_tab("main".into(), "s2".into())
        .await
        .unwrap();

    let snapshots = state
        .move_tab("s2".into(), "main".into(), "other".into(), 0)
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

    assert_eq!(main.window.tab_order, vec!["s1", "s3"]);
    assert_eq!(main.window.active_tab_id.as_deref(), Some("s3"));
    assert_eq!(other.window.active_tab_id.as_deref(), Some("s2"));
}

#[tokio::test]
async fn moving_active_last_tab_selects_left_neighbor_in_source() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state
        .register_window("other".into(), "other".into(), false)
        .await;
    state.register_tab(input("s1", Some("main"))).await;
    state.register_tab(input("s2", Some("main"))).await;
    state.register_tab(input("s3", Some("main"))).await;

    let snapshots = state
        .move_tab("s3".into(), "main".into(), "other".into(), 0)
        .await
        .unwrap();
    let main = snapshots
        .iter()
        .find(|snapshot| snapshot.window_id == "main")
        .unwrap();

    assert_eq!(main.window.tab_order, vec!["s1", "s2"]);
    assert_eq!(main.window.active_tab_id.as_deref(), Some("s2"));
}

#[tokio::test]
async fn moving_inactive_tab_preserves_source_active_and_activates_destination() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state
        .register_window("other".into(), "other".into(), false)
        .await;
    state.register_tab(input("s1", Some("main"))).await;
    state.register_tab(input("s2", Some("main"))).await;
    state.register_tab(input("d1", Some("other"))).await;

    let snapshots = state
        .move_tab("s1".into(), "main".into(), "other".into(), 1)
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

    assert_eq!(main.window.active_tab_id.as_deref(), Some("s2"));
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

    assert!(error.contains("Destination window"));
    assert_eq!(main.window.tab_order, vec!["s1"]);
    assert_eq!(main.tabs[0].owner_window_id, "main");
    assert!(main.tabs[0].is_connected);
}

#[tokio::test]
async fn tab_move_source_validation_rejects_wrong_window_without_changes() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state
        .register_window("other".into(), "other".into(), true)
        .await;
    state.register_tab(input("s1", Some("main"))).await;

    let error = state
        .validate_tab_move_source("s1", "other")
        .await
        .unwrap_err();
    let main = state.snapshot_for_window("main".into()).await;
    let other = state.snapshot_for_window("other".into()).await;

    assert!(error.contains("Source tab"));
    assert_eq!(main.window.tab_order, vec!["s1"]);
    assert!(other.window.tab_order.is_empty());
    assert_eq!(main.tabs[0].owner_window_id, "main");
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
async fn removing_active_middle_tab_selects_right_neighbor() {
    let state = WorkspaceState::new();
    state
        .register_window("main".into(), "main".into(), true)
        .await;
    state.register_tab(input("s1", Some("main"))).await;
    state.register_tab(input("s2", Some("main"))).await;
    state.register_tab(input("s3", Some("main"))).await;
    state
        .activate_tab("main".into(), "s2".into())
        .await
        .unwrap();

    let snapshot = state.remove_tab("main".into(), "s2".into()).await.unwrap();

    assert_eq!(snapshot.window.tab_order, vec!["s1", "s3"]);
    assert_eq!(snapshot.window.active_tab_id.as_deref(), Some("s3"));
}

#[tokio::test]
async fn removing_active_last_tab_selects_left_neighbor_then_null() {
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
