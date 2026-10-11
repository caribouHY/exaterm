use super::*;
use uuid::Uuid;

fn test_dir(prefix: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-logger")
        .join(format!("{prefix}_{}", Uuid::new_v4()))
}

fn temp_index_path() -> PathBuf {
    test_dir("test").join("index.json")
}

fn sample_session(session_id: &str, started_at: &str, target: &str) -> LogSession {
    LogSession {
        session_id: session_id.into(),
        connection_type: "ssh".into(),
        target: target.into(),
        started_at: started_at.into(),
        file_path: format!("C:\\logs\\{}.log", session_id),
        log_mode: "auto".into(),
    }
}

fn cleanup(path: &PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir_all(parent);
    }
}

fn empty_active_keys() -> HashSet<ActiveLogKey> {
    HashSet::new()
}

#[test]
fn read_log_index_returns_empty_when_missing() {
    let path = temp_index_path();
    let sessions = read_log_index(&path).expect("missing index should read as empty");

    assert!(sessions.is_empty());
    cleanup(&path);
}

#[test]
fn write_and_read_log_index_round_trips_sessions() {
    let path = temp_index_path();
    let sessions = vec![sample_session(
        "session-1",
        "2026-04-25T10:00:00+09:00",
        "user@host:22",
    )];

    write_log_index(&path, &sessions).expect("index should write");
    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].session_id, "session-1");
    assert_eq!(loaded[0].target, "user@host:22");
    assert_eq!(loaded[0].log_mode, "auto");
    cleanup(&path);
}

#[test]
fn upsert_log_session_replaces_existing_session_id() {
    let path = temp_index_path();
    upsert_log_session(
        &path,
        sample_session("session-1", "2026-04-25T10:00:00+09:00", "old@host:22"),
    )
    .expect("initial upsert should write");
    upsert_log_session(
        &path,
        sample_session("session-1", "2026-04-25T11:00:00+09:00", "new@host:22"),
    )
    .expect("second upsert should replace");

    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].target, "new@host:22");
    assert_eq!(loaded[0].started_at, "2026-04-25T11:00:00+09:00");
    cleanup(&path);
}

#[test]
fn upsert_log_session_keeps_auto_and_manual_entries() {
    let path = temp_index_path();
    let auto = sample_session("session-1", "2026-04-25T10:00:00+09:00", "host");
    let mut manual = sample_session("session-1", "2026-04-25T10:01:00+09:00", "host");
    manual.log_mode = "manual".into();
    manual.file_path = "C:\\manual\\session-1.log".into();

    upsert_log_session(&path, auto).expect("auto upsert should write");
    upsert_log_session(&path, manual).expect("manual upsert should write");
    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(loaded.len(), 2);
    assert!(loaded.iter().any(|entry| entry.log_mode == "auto"));
    assert!(loaded.iter().any(|entry| entry.log_mode == "manual"));
    cleanup(&path);
}

#[test]
fn upsert_log_session_keeps_multiple_manual_entries_for_same_session() {
    let path = temp_index_path();
    let mut first = sample_session("session-1", "2026-04-25T10:00:00+09:00", "host");
    first.log_mode = "manual".into();
    first.file_path = "C:\\manual\\first.log".into();
    let mut second = sample_session("session-1", "2026-04-25T11:00:00+09:00", "host");
    second.log_mode = "manual".into();
    second.file_path = "C:\\manual\\second.log".into();

    upsert_log_session(&path, first).expect("first manual upsert should write");
    upsert_log_session(&path, second).expect("second manual upsert should append");
    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].file_path, "C:\\manual\\second.log");
    assert_eq!(loaded[1].file_path, "C:\\manual\\first.log");
    cleanup(&path);
}

#[test]
fn append_to_log_sessions_writes_to_active_target() {
    let dir = test_dir("test");
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let log_path = dir.join("session.log");
    fs::write(&log_path, "log\n").expect("log file should be created");
    let sessions = vec![LogSession {
        file_path: log_path.to_string_lossy().to_string(),
        ..sample_session("session-1", "2026-04-25T10:00:00+09:00", "host")
    }];

    append_to_log_sessions(&sessions, "data\n").expect("append should write the log");

    assert_eq!(fs::read_to_string(&log_path).unwrap(), "log\ndata\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_log_session_writes_header_when_enabled() {
    let dir = test_dir("test");
    fs::create_dir_all(&dir).expect("temp dir should be created");

    let session = create_log_session(
        &dir,
        LogSessionCreateOptions {
            session_id: "session-1".into(),
            connection_type: "ssh".into(),
            target: "user@host:22".into(),
            file_path: None,
            log_mode: "auto".into(),
            include_header: true,
            write_mode: LogWriteMode::Overwrite,
        },
    )
    .expect("log session should be created");
    let data = fs::read_to_string(&session.file_path).expect("log should read");

    assert!(data.starts_with("# ExaTerm Log\n"));
    assert!(data.contains("# Type: ssh\n"));
    assert!(data.contains("# Target: user@host:22\n"));
    assert!(data.contains("# Mode: auto\n"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_log_session_skips_header_when_disabled() {
    let dir = test_dir("test");
    fs::create_dir_all(&dir).expect("temp dir should be created");

    let session = create_log_session(
        &dir,
        LogSessionCreateOptions {
            session_id: "session-1".into(),
            connection_type: "ssh".into(),
            target: "user@host:22".into(),
            file_path: None,
            log_mode: "auto".into(),
            include_header: false,
            write_mode: LogWriteMode::Overwrite,
        },
    )
    .expect("log session should be created");
    let data = fs::read_to_string(&session.file_path).expect("log should read");

    assert_eq!(data, "");
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn start_manual_log_without_file_path_uses_log_dir() {
    let dir = test_dir("test");
    let index_path = dir.join("index.json");
    let state = LoggerState::with_paths(dir.clone(), index_path.clone());

    let file_path = start_manual_log(
        &state,
        "session-1".into(),
        "ssh".into(),
        "user@host:22".into(),
        None,
        None,
    )
    .await
    .expect("manual log should start");
    let path = PathBuf::from(&file_path);
    let parent = path.parent().expect("manual log should have parent");

    assert_eq!(parent, dir.as_path());
    assert!(path.exists());

    let loaded = read_log_index(&index_path).expect("index should read");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].log_mode, "manual");
    assert_eq!(loaded[0].file_path, file_path);
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn repeated_manual_start_reuses_the_active_file_without_overwriting_it() {
    let dir = test_dir("test");
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let index_path = dir.join("index.json");
    let first_path = dir.join("first.log");
    let second_path = dir.join("second.log");
    let state = LoggerState::with_paths(dir.clone(), index_path);

    let active_path = start_manual_log(
        &state,
        "session-1".into(),
        "ssh".into(),
        "host".into(),
        Some(first_path.to_string_lossy().to_string()),
        Some("overwrite".into()),
    )
    .await
    .expect("first manual log should start");
    fs::write(&first_path, "preserved content\n").expect("active log should be writable");

    let repeated_path = start_manual_log(
        &state,
        "session-1".into(),
        "ssh".into(),
        "host".into(),
        Some(second_path.to_string_lossy().to_string()),
        Some("overwrite".into()),
    )
    .await
    .expect("repeated manual log start should be idempotent");

    assert_eq!(repeated_path, active_path);
    assert_eq!(
        fs::read_to_string(first_path).unwrap(),
        "preserved content\n"
    );
    assert!(!second_path.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn connection_and_manual_starts_share_one_active_target() {
    let dir = test_dir("test");
    let index_path = dir.join("index.json");
    let state = LoggerState::with_paths(dir.clone(), index_path.clone());

    start_log_on_connection(
        &state,
        "session-1".into(),
        "ssh".into(),
        "user@host:22".into(),
    )
    .await
    .expect("connection log should start");
    let auto_session = manual_log_session(&state, "session-1")
        .await
        .expect("connection log should be active");
    assert_eq!(auto_session.log_mode, "auto");
    start_manual_log(
        &state,
        "session-1".into(),
        "ssh".into(),
        "user@host:22".into(),
        None,
        None,
    )
    .await
    .expect("manual log should start");
    let manual_session = manual_log_session(&state, "session-1")
        .await
        .expect("manual log should replace the active target");
    assert_eq!(manual_session.log_mode, "manual");
    assert_ne!(manual_session.file_path, auto_session.file_path);
    assert_eq!(state.sessions.lock().await.len(), 1);

    clear_session_logs(&state, "session-1").await;
    let active_keys = {
        let sessions = state.sessions.lock().await;
        active_log_keys(&sessions)
    };
    let result = bulk_delete_log_sessions(&index_path, &dir, &active_keys, false)
        .expect("bulk delete should succeed");
    let loaded = read_log_index(&index_path).expect("index should read");

    assert_eq!(result.skipped_active_count, 0);
    assert_eq!(result.removed_history_count, 2);
    assert!(loaded.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_log_session_overwrite_replaces_existing_file() {
    let dir = test_dir("test");
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let path = dir.join("manual.log");
    fs::write(&path, "existing content\n").expect("manual file should be created");

    let session = create_log_session(
        &dir,
        LogSessionCreateOptions {
            session_id: "session-1".into(),
            connection_type: "ssh".into(),
            target: "user@host:22".into(),
            file_path: Some(path.to_string_lossy().to_string()),
            log_mode: "manual".into(),
            include_header: true,
            write_mode: LogWriteMode::Overwrite,
        },
    )
    .expect("log session should be created");
    let data = fs::read_to_string(&session.file_path).expect("log should read");

    assert!(data.starts_with("# ExaTerm Log\n"));
    assert!(data.contains("# Mode: manual\n"));
    assert!(!data.contains("existing content"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_log_session_appends_header_to_existing_file() {
    let dir = test_dir("test");
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let path = dir.join("manual.log");
    fs::write(&path, "existing content").expect("manual file should be created");

    let session = create_log_session(
        &dir,
        LogSessionCreateOptions {
            session_id: "session-1".into(),
            connection_type: "ssh".into(),
            target: "user@host:22".into(),
            file_path: Some(path.to_string_lossy().to_string()),
            log_mode: "manual".into(),
            include_header: true,
            write_mode: LogWriteMode::Append,
        },
    )
    .expect("log session should be created");
    let data = fs::read_to_string(&session.file_path).expect("log should read");

    assert!(data.starts_with("existing content\n# ExaTerm Log Append\n"));
    assert!(data.contains("# Started: "));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn create_log_session_append_skips_header_when_disabled() {
    let dir = test_dir("test");
    fs::create_dir_all(&dir).expect("temp dir should be created");
    let path = dir.join("manual.log");
    fs::write(&path, "existing content\n").expect("manual file should be created");

    let session = create_log_session(
        &dir,
        LogSessionCreateOptions {
            session_id: "session-1".into(),
            connection_type: "ssh".into(),
            target: "user@host:22".into(),
            file_path: Some(path.to_string_lossy().to_string()),
            log_mode: "manual".into(),
            include_header: false,
            write_mode: LogWriteMode::Append,
        },
    )
    .expect("log session should be created");
    append_to_log_sessions(&[session.clone()], "new content\n")
        .expect("manual append should write");
    let data = fs::read_to_string(&session.file_path).expect("log should read");

    assert_eq!(data, "existing content\nnew content\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn read_log_index_sorts_sessions_by_started_at_desc() {
    let path = temp_index_path();
    let sessions = vec![
        sample_session("older", "2026-04-25T09:00:00+09:00", "older@host:22"),
        sample_session("newer", "2026-04-25T11:00:00+09:00", "newer@host:22"),
        sample_session("middle", "2026-04-25T10:00:00+09:00", "middle@host:22"),
    ];

    write_log_index(&path, &sessions).expect("index should write");
    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(loaded[0].session_id, "newer");
    assert_eq!(loaded[1].session_id, "middle");
    assert_eq!(loaded[2].session_id, "older");
    cleanup(&path);
}

#[test]
fn bulk_delete_removes_inactive_auto_and_manual_history() {
    let path = temp_index_path();
    let log_dir = path.parent().unwrap().to_path_buf();
    let auto = sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host");
    let mut manual = sample_session("manual-1", "2026-04-25T11:00:00+09:00", "host");
    manual.log_mode = "manual".into();

    write_log_index(&path, &[auto, manual]).expect("index should write");
    let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), false)
        .expect("bulk delete should succeed");
    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(result.removed_history_count, 2);
    assert!(loaded.is_empty());
    cleanup(&path);
}

#[test]
fn bulk_delete_history_only_leaves_files() {
    let path = temp_index_path();
    let log_dir = path.parent().unwrap().to_path_buf();
    fs::create_dir_all(&log_dir).expect("log dir should be created");
    let auto_path = log_dir.join("auto.log");
    let manual_path = log_dir.join("manual.log");
    fs::write(&auto_path, "auto").expect("auto file should be created");
    fs::write(&manual_path, "manual").expect("manual file should be created");
    let auto = LogSession {
        file_path: auto_path.to_string_lossy().to_string(),
        ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
    };
    let manual = LogSession {
        file_path: manual_path.to_string_lossy().to_string(),
        log_mode: "manual".into(),
        ..sample_session("manual-1", "2026-04-25T11:00:00+09:00", "host")
    };

    write_log_index(&path, &[auto, manual]).expect("index should write");
    let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), false)
        .expect("bulk delete should succeed");

    assert_eq!(result.removed_history_count, 2);
    assert!(auto_path.exists());
    assert!(manual_path.exists());
    cleanup(&path);
}

#[test]
fn bulk_delete_with_files_removes_only_auto_files() {
    let path = temp_index_path();
    let log_dir = path.parent().unwrap().to_path_buf();
    fs::create_dir_all(&log_dir).expect("log dir should be created");
    let auto_path = log_dir.join("auto.log");
    let manual_path = log_dir.join("manual.log");
    fs::write(&auto_path, "auto").expect("auto file should be created");
    fs::write(&manual_path, "manual").expect("manual file should be created");
    let auto = LogSession {
        file_path: auto_path.to_string_lossy().to_string(),
        ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
    };
    let manual = LogSession {
        file_path: manual_path.to_string_lossy().to_string(),
        log_mode: "manual".into(),
        ..sample_session("manual-1", "2026-04-25T11:00:00+09:00", "host")
    };

    write_log_index(&path, &[auto, manual]).expect("index should write");
    let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), true)
        .expect("bulk delete should succeed");

    assert_eq!(result.removed_history_count, 2);
    assert_eq!(result.removed_auto_file_count, 1);
    assert_eq!(result.skipped_manual_file_count, 1);
    assert!(!auto_path.exists());
    assert!(manual_path.exists());
    cleanup(&path);
}

#[test]
fn bulk_delete_keeps_active_logs() {
    let path = temp_index_path();
    let log_dir = path.parent().unwrap().to_path_buf();
    fs::create_dir_all(&log_dir).expect("log dir should be created");
    let auto_path = log_dir.join("auto.log");
    let manual_path = log_dir.join("manual.log");
    fs::write(&auto_path, "auto").expect("auto file should be created");
    fs::write(&manual_path, "manual").expect("manual file should be created");
    let auto = LogSession {
        file_path: auto_path.to_string_lossy().to_string(),
        ..sample_session("active", "2026-04-25T10:00:00+09:00", "host")
    };
    let manual = LogSession {
        file_path: manual_path.to_string_lossy().to_string(),
        log_mode: "manual".into(),
        ..sample_session("active", "2026-04-25T11:00:00+09:00", "host")
    };
    let active = HashSet::from([
        ActiveLogKey {
            session_id: "active".into(),
            log_mode: "auto".into(),
            file_path: auto_path.to_string_lossy().to_string(),
        },
        ActiveLogKey {
            session_id: "active".into(),
            log_mode: "manual".into(),
            file_path: manual_path.to_string_lossy().to_string(),
        },
    ]);

    write_log_index(&path, &[auto, manual]).expect("index should write");
    let result = bulk_delete_log_sessions(&path, &log_dir, &active, true)
        .expect("bulk delete should succeed");
    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(result.removed_history_count, 0);
    assert_eq!(result.skipped_active_count, 2);
    assert_eq!(loaded.len(), 2);
    assert!(auto_path.exists());
    assert!(manual_path.exists());
    cleanup(&path);
}

#[test]
fn bulk_delete_removes_old_manual_history_for_same_active_session() {
    let path = temp_index_path();
    let log_dir = path.parent().unwrap().to_path_buf();
    fs::create_dir_all(&log_dir).expect("log dir should be created");
    let old_path = log_dir.join("old_manual.log");
    let active_path = log_dir.join("active_manual.log");
    fs::write(&old_path, "old").expect("old manual file should be created");
    fs::write(&active_path, "active").expect("active manual file should be created");
    let old_manual = LogSession {
        file_path: old_path.to_string_lossy().to_string(),
        log_mode: "manual".into(),
        ..sample_session("active", "2026-04-25T10:00:00+09:00", "host")
    };
    let active_manual = LogSession {
        file_path: active_path.to_string_lossy().to_string(),
        log_mode: "manual".into(),
        ..sample_session("active", "2026-04-25T11:00:00+09:00", "host")
    };
    let active = HashSet::from([ActiveLogKey {
        session_id: "active".into(),
        log_mode: "manual".into(),
        file_path: active_path.to_string_lossy().to_string(),
    }]);

    write_log_index(&path, &[old_manual, active_manual]).expect("index should write");
    let result = bulk_delete_log_sessions(&path, &log_dir, &active, false)
        .expect("bulk delete should succeed");
    let loaded = read_log_index(&path).expect("index should read");

    assert_eq!(result.removed_history_count, 1);
    assert_eq!(result.skipped_active_count, 1);
    assert_eq!(loaded.len(), 1);
    assert_eq!(
        loaded[0].file_path,
        active_path.to_string_lossy().to_string()
    );
    cleanup(&path);
}

#[test]
fn bulk_delete_counts_missing_auto_file_as_skipped() {
    let path = temp_index_path();
    let log_dir = path.parent().unwrap().to_path_buf();
    fs::create_dir_all(&log_dir).expect("log dir should be created");
    let auto_path = log_dir.join("missing.log");
    let auto = LogSession {
        file_path: auto_path.to_string_lossy().to_string(),
        ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
    };

    write_log_index(&path, &[auto]).expect("index should write");
    let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), true)
        .expect("bulk delete should succeed");

    assert_eq!(result.removed_history_count, 1);
    assert_eq!(result.skipped_missing_file_count, 1);
    cleanup(&path);
}

#[tokio::test]
async fn bulk_delete_preserves_auto_file_reused_by_manual_logging() {
    for (manual_id, keep_manual_history) in [("auto", true), ("manual", true), ("manual", false)] {
        let index_path = temp_index_path();
        let dir = index_path.parent().unwrap().to_path_buf();
        let state = LoggerState::with_paths(dir.clone(), index_path.clone());
        let file_path = start_log_on_connection(&state, "auto".into(), "ssh".into(), "host".into())
            .await
            .unwrap();
        stop_manual_log(&state, "auto").await.unwrap();
        start_manual_log(
            &state,
            manual_id.into(),
            "ssh".into(),
            "host".into(),
            Some(file_path.clone()),
            Some("append".into()),
        )
        .await
        .unwrap();
        let manual = active_log_session(&state, manual_id).await.unwrap();
        append_to_log_sessions(std::slice::from_ref(&manual), "before deletion\n").unwrap();
        if !keep_manual_history {
            let mut history = read_log_index(&index_path).unwrap();
            history.retain(|entry| entry.log_mode == "auto");
            write_log_index(&index_path, &history).unwrap();
        }
        let before = fs::read_to_string(&file_path).unwrap();

        let result = delete_log_sessions(&state, true).await.unwrap();

        let expected_rows = if keep_manual_history { 2 } else { 1 };
        assert_eq!(result.skipped_active_count, expected_rows);
        assert_eq!(result.removed_history_count, 0);
        assert_eq!(result.removed_auto_file_count, 0);
        assert_eq!(read_log_index(&index_path).unwrap().len(), expected_rows);
        append_to_log_sessions(&[manual], "after deletion\n").unwrap();
        assert_eq!(
            fs::read_to_string(file_path).unwrap(),
            format!("{before}after deletion\n")
        );
        cleanup(&index_path);
    }
}

#[tokio::test]
async fn bulk_delete_history_only_removes_old_row_for_shared_active_file() {
    let index_path = temp_index_path();
    let dir = index_path.parent().unwrap().to_path_buf();
    let state = LoggerState::with_paths(dir, index_path.clone());
    let file_path = start_log_on_connection(&state, "auto".into(), "ssh".into(), "host".into())
        .await
        .unwrap();
    stop_manual_log(&state, "auto").await.unwrap();
    start_manual_log(
        &state,
        "manual".into(),
        "ssh".into(),
        "host".into(),
        Some(file_path.clone()),
        Some("append".into()),
    )
    .await
    .unwrap();

    let result = delete_log_sessions(&state, false).await.unwrap();

    assert_eq!(result.removed_history_count, 1);
    assert_eq!(result.skipped_active_count, 1);
    assert_eq!(read_log_index(&index_path).unwrap()[0].log_mode, "manual");
    assert!(Path::new(&file_path).exists());
    cleanup(&index_path);
}

#[tokio::test]
async fn bulk_delete_protects_normalized_path_aliases() {
    let index_path = temp_index_path();
    let dir = index_path.parent().unwrap().to_path_buf();
    let state = LoggerState::with_paths(dir.clone(), index_path.clone());
    let file = dir.join("MixedCase.log");
    start_manual_log(
        &state,
        "manual".into(),
        "ssh".into(),
        "host".into(),
        Some(file.to_string_lossy().into_owned()),
        None,
    )
    .await
    .unwrap();
    let subdir = dir.join("nested");
    fs::create_dir(&subdir).unwrap();
    let linked_file = dir.join("linked.log");
    fs::hard_link(&file, &linked_file).unwrap();
    let aliases = vec![
        dir.join(".").join("MixedCase.log"),
        subdir.join("..").join("MixedCase.log"),
        fs::canonicalize(&file).unwrap(),
        linked_file,
    ];
    #[cfg(windows)]
    let aliases = {
        let mut aliases = aliases;
        aliases.push(PathBuf::from(file.to_string_lossy().to_uppercase()));
        aliases.push(PathBuf::from(file.to_string_lossy().replace('\\', "/")));
        aliases
    };
    for alias in aliases {
        let old = LogSession {
            file_path: alias.to_string_lossy().into_owned(),
            ..sample_session("old-auto", "2026-04-25T10:00:00+09:00", "host")
        };
        let active = active_log_session(&state, "manual").await.unwrap();
        write_log_index(&index_path, &[old, active]).unwrap();

        let result = delete_log_sessions(&state, true).await.unwrap();

        assert_eq!(result.skipped_active_count, 2);
        assert_eq!(result.removed_auto_file_count, 0);
        assert!(file.exists());
    }
    cleanup(&index_path);
}

#[tokio::test]
async fn bulk_delete_protects_relative_path_alias() {
    let relative_dir =
        PathBuf::from("target").join(format!("exaterm_logger_test_{}", Uuid::new_v4()));
    let dir = std::env::current_dir().unwrap().join(&relative_dir);
    let index_path = dir.join("index.json");
    let state = LoggerState::with_paths(dir.clone(), index_path.clone());
    let file = dir.join("active.log");
    start_manual_log(
        &state,
        "manual".into(),
        "ssh".into(),
        "host".into(),
        Some(file.to_string_lossy().into_owned()),
        None,
    )
    .await
    .unwrap();
    let old = LogSession {
        file_path: relative_dir
            .join("active.log")
            .to_string_lossy()
            .into_owned(),
        ..sample_session("old-auto", "2026-04-25T10:00:00+09:00", "host")
    };
    upsert_log_session(&index_path, old).unwrap();

    let result = delete_log_sessions(&state, true).await.unwrap();

    assert_eq!(result.skipped_active_count, 2);
    assert!(file.exists());
    cleanup(&index_path);
}

#[cfg(windows)]
#[tokio::test]
async fn bulk_delete_protects_unicode_case_alias_and_removes_distinct_file() {
    let index_path = temp_index_path();
    let dir = index_path.parent().unwrap().to_path_buf();
    let state = LoggerState::with_paths(dir.clone(), index_path.clone());
    let active_file = dir.join("Écho.log");
    start_manual_log(
        &state,
        "manual".into(),
        "ssh".into(),
        "host".into(),
        Some(active_file.to_string_lossy().into_owned()),
        None,
    )
    .await
    .unwrap();
    let alias = LogSession {
        file_path: dir.join("éCHO.LOG").to_string_lossy().into_owned(),
        ..sample_session("old-auto", "2026-04-25T10:00:00+09:00", "host")
    };
    upsert_log_session(&index_path, alias).unwrap();
    let other_file = dir.join("other.log");
    fs::write(&other_file, "inactive").unwrap();
    upsert_log_session(
        &index_path,
        LogSession {
            file_path: other_file.to_string_lossy().into_owned(),
            ..sample_session("other-auto", "2026-04-25T10:00:00+09:00", "host")
        },
    )
    .unwrap();

    let result = delete_log_sessions(&state, true).await.unwrap();

    assert_eq!(result.skipped_active_count, 2);
    assert_eq!(result.removed_auto_file_count, 1);
    assert!(active_file.exists());
    assert!(!other_file.exists());
    cleanup(&index_path);
}

#[tokio::test]
async fn bulk_delete_fails_before_mutation_if_active_file_cannot_be_resolved() {
    let index_path = temp_index_path();
    let dir = index_path.parent().unwrap().to_path_buf();
    let state = LoggerState::with_paths(dir.clone(), index_path.clone());
    let file_path = start_manual_log(
        &state,
        "manual".into(),
        "ssh".into(),
        "host".into(),
        None,
        None,
    )
    .await
    .unwrap();
    fs::remove_file(file_path).unwrap();
    let inactive_file = dir.join("inactive.log");
    fs::write(&inactive_file, "preserve me").unwrap();
    upsert_log_session(
        &index_path,
        LogSession {
            file_path: inactive_file.to_string_lossy().into_owned(),
            ..sample_session("inactive", "2027-04-25T10:00:00+09:00", "host")
        },
    )
    .unwrap();
    let index_before = fs::read(&index_path).unwrap();

    assert!(delete_log_sessions(&state, true).await.is_err());

    assert_eq!(fs::read(&index_path).unwrap(), index_before);
    assert_eq!(fs::read_to_string(inactive_file).unwrap(), "preserve me");
    assert_eq!(
        delete_log_sessions(&state, false)
            .await
            .unwrap()
            .removed_history_count,
        1
    );
    cleanup(&index_path);
}

#[tokio::test]
async fn bulk_delete_and_log_start_are_serialized() {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    let index_path = temp_index_path();
    let dir = index_path.parent().unwrap().to_path_buf();
    let state = LoggerState::with_paths(dir.clone(), index_path.clone());
    let file = dir.join("old.log");
    fs::write(&file, "old content").unwrap();
    upsert_log_session(
        &index_path,
        LogSession {
            file_path: file.to_string_lossy().into_owned(),
            ..sample_session("old-auto", "2026-04-25T10:00:00+09:00", "host")
        },
    )
    .unwrap();
    let guard = state.sessions.lock().await;
    let mut deletion = std::pin::pin!(delete_log_sessions(&state, true));
    let mut start = std::pin::pin!(start_manual_log(
        &state,
        "new-manual".into(),
        "ssh".into(),
        "host".into(),
        Some(file.to_string_lossy().into_owned()),
        Some("append".into()),
    ));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(deletion.as_mut().poll(&mut cx).is_pending());
    assert!(start.as_mut().poll(&mut cx).is_pending());
    drop(guard);

    let Poll::Ready(Ok(result)) = deletion.as_mut().poll(&mut cx) else {
        panic!("deletion should complete before the queued log start");
    };
    assert_eq!(result.removed_auto_file_count, 1);
    assert!(!file.exists());
    assert!(matches!(start.as_mut().poll(&mut cx), Poll::Ready(Ok(_))));
    let active = active_log_session(&state, "new-manual").await.unwrap();
    append_to_log_sessions(&[active], "new content\n").unwrap();
    assert!(fs::read_to_string(&file)
        .unwrap()
        .ends_with("new content\n"));
    let history = read_log_index(&index_path).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].session_id, "new-manual");
    cleanup(&index_path);
}

#[test]
fn bulk_delete_skips_auto_file_outside_log_dir() {
    let path = temp_index_path();
    let log_dir = path.parent().unwrap().to_path_buf();
    fs::create_dir_all(&log_dir).expect("log dir should be created");
    let outside_dir = test_dir("outside");
    fs::create_dir_all(&outside_dir).expect("outside dir should be created");
    let outside_path = outside_dir.join("auto.log");
    fs::write(&outside_path, "auto").expect("outside file should be created");
    let auto = LogSession {
        file_path: outside_path.to_string_lossy().to_string(),
        ..sample_session("auto-1", "2026-04-25T10:00:00+09:00", "host")
    };

    write_log_index(&path, &[auto]).expect("index should write");
    let result = bulk_delete_log_sessions(&path, &log_dir, &empty_active_keys(), true)
        .expect("bulk delete should succeed");

    assert_eq!(result.removed_history_count, 1);
    assert_eq!(result.skipped_unsafe_path_count, 1);
    assert!(outside_path.exists());
    cleanup(&path);
    let _ = fs::remove_dir_all(outside_dir);
}
