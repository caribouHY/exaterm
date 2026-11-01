use super::{
    manual_log_status, set_manual_log_paused, start_manual_log, stop_manual_log, LoggerState,
    ManualLogStatus,
};
use crate::workspace::{WorkspaceSnapshot, WorkspaceState};

pub(super) enum ManualLogOperation {
    Start {
        connection_type: String,
        target: String,
        file_path: Option<String>,
        write_mode: Option<String>,
    },
    Stop,
    SetPaused(bool),
}

pub(super) async fn apply_manual_log_operation(
    state: &LoggerState,
    workspace: &WorkspaceState,
    session_id: &str,
    operation: ManualLogOperation,
) -> Result<(ManualLogStatus, Option<WorkspaceSnapshot>), String> {
    let _operation = state.operation_lock(session_id).await;
    match operation {
        ManualLogOperation::Start {
            connection_type,
            target,
            file_path,
            write_mode,
        } => {
            let tab = workspace
                .tab_for_session(session_id)
                .await
                .ok_or("Session not found.")?;
            if !tab.is_connected {
                return Err("Session is disconnected.".into());
            }
            start_manual_log(
                state,
                session_id.into(),
                connection_type,
                target,
                file_path,
                write_mode,
            )
            .await?;
            set_manual_log_paused(state, session_id, false).await?;
        }
        ManualLogOperation::Stop => stop_manual_log(state, session_id).await?,
        ManualLogOperation::SetPaused(paused) => {
            set_manual_log_paused(state, session_id, paused).await?
        }
    }
    let status = manual_log_status(state, session_id).await;
    let snapshot = workspace
        .sync_log_state(
            session_id,
            status.active,
            status.paused,
            status.file_path.clone(),
        )
        .await;
    Ok((status, snapshot))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_control::{TerminalControlState, TerminalProtocol};
    use crate::workspace::WorkspaceTabRegisterInput;
    use std::{fs, pin::pin, task::Poll};

    async fn fixture() -> (
        LoggerState,
        WorkspaceState,
        TerminalControlState,
        std::path::PathBuf,
    ) {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-logger")
            .join(uuid::Uuid::new_v4().to_string());
        let logger = LoggerState::with_paths(dir.clone(), dir.join("index.json"));
        let workspace = WorkspaceState::new();
        workspace
            .register_window("main".into(), "main".into(), true)
            .await;
        workspace
            .register_window("other".into(), "other".into(), true)
            .await;
        workspace
            .register_tab(WorkspaceTabRegisterInput {
                window_id: Some("main".into()),
                tab_id: Some("tab-1".into()),
                session_id: "s1".into(),
                connection_type: TerminalProtocol::Ssh,
                title: "Synthetic".into(),
                encoding: "utf-8".into(),
                terminal_mode: "general".into(),
                connection_info: None,
                is_manual_logging: false,
                manual_log_file_path: None,
            })
            .await;
        let terminals = TerminalControlState::new();
        terminals.set_logger(logger.clone());
        terminals
            .register_session("s1".into(), TerminalProtocol::Ssh, "synthetic".into())
            .await;
        (logger, workspace, terminals, dir)
    }

    async fn start(logger: &LoggerState, workspace: &WorkspaceState) -> String {
        let (status, _) = apply_manual_log_operation(
            logger,
            workspace,
            "s1",
            ManualLogOperation::Start {
                connection_type: "ssh".into(),
                target: "synthetic".into(),
                file_path: None,
                write_mode: None,
            },
        )
        .await
        .unwrap();
        logger
            .captures
            .lock()
            .await
            .get_mut("s1")
            .unwrap()
            .sanitizer = super::super::sanitizer::TerminalLogSanitizer::new("display");
        status.file_path.unwrap()
    }

    #[tokio::test]
    async fn pending_flush_and_pause_survive_moves_and_repeated_restoration() {
        let (logger, workspace, terminals, dir) = fixture().await;
        terminals.append_output("s1", b"before start\n").await;
        let path = start(&logger, &workspace).await;
        terminals.append_output("s1", b"partial").await;
        let blocked = logger.sessions.lock().await;
        let mut flush = pin!(super::super::flush_manual_log(&logger, "s1"));
        assert!(matches!(futures::poll!(&mut flush), Poll::Pending));
        workspace
            .move_tab("tab-1".into(), "main".into(), "other".into(), 0)
            .await
            .unwrap();
        let mut pause = pin!(apply_manual_log_operation(
            &logger,
            &workspace,
            "s1",
            ManualLogOperation::SetPaused(true)
        ));
        assert!(matches!(futures::poll!(&mut pause), Poll::Pending));
        drop(blocked);
        flush.await.unwrap();
        let (_, snapshot) = pause.await.unwrap();
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.window_id, "other");
        assert!(snapshot.tabs[0].is_manual_logging_paused);
        terminals.append_output("s1", b"paused secret\n").await;
        workspace
            .move_tab("tab-1".into(), "other".into(), "main".into(), 0)
            .await
            .unwrap();
        apply_manual_log_operation(
            &logger,
            &workspace,
            "s1",
            ManualLogOperation::SetPaused(false),
        )
        .await
        .unwrap();
        terminals
            .append_output("s1", "same界😀\nsame界😀\n".as_bytes())
            .await;
        for _ in 0..3 {
            terminals.read_output("s1", 1000).await.unwrap();
        }
        let (_, snapshot) =
            apply_manual_log_operation(&logger, &workspace, "s1", ManualLogOperation::Stop)
                .await
                .unwrap();
        assert!(!snapshot.unwrap().tabs[0].is_manual_logging);
        let text = fs::read_to_string(path).unwrap();
        assert!(text.ends_with("partialsame界😀\nsame界😀\n"));
        assert_eq!(text.matches("same界😀").count(), 2);
        assert!(!text.contains("before start"));
        assert!(!text.contains("paused secret"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn destination_stop_waits_for_source_operation_and_wins_metadata() {
        let (logger, workspace, terminals, dir) = fixture().await;
        let path = start(&logger, &workspace).await;
        terminals.append_output("s1", b"tail").await;
        let blocked = logger.sessions.lock().await;
        let mut source = pin!(apply_manual_log_operation(
            &logger,
            &workspace,
            "s1",
            ManualLogOperation::SetPaused(true)
        ));
        assert!(matches!(futures::poll!(&mut source), Poll::Pending));
        workspace
            .move_tab("tab-1".into(), "main".into(), "other".into(), 0)
            .await
            .unwrap();
        let mut destination = pin!(apply_manual_log_operation(
            &logger,
            &workspace,
            "s1",
            ManualLogOperation::Stop
        ));
        assert!(matches!(futures::poll!(&mut destination), Poll::Pending));
        drop(blocked);
        source.await.unwrap();
        let (status, snapshot) = destination.await.unwrap();
        assert!(!status.active);
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.window_id, "other");
        assert!(!snapshot.tabs[0].is_manual_logging);
        assert!(!snapshot.tabs[0].is_manual_logging_paused);
        assert!(snapshot.tabs[0].manual_log_file_path.is_none());
        assert!(fs::read_to_string(path).unwrap().ends_with("tail"));
        fs::remove_dir_all(dir).unwrap();
    }
}
