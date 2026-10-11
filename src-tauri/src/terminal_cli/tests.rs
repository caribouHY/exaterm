use super::*;
use crate::external_control::service::ListTerminalSessionsResult;
use serde_json::{json, Value};

fn parse(args: &[&str]) -> RootCommand {
    Cli::try_parse_from(args).unwrap().command
}

fn client_diagnostic(
    gui_started: bool,
    control_plane_reachable: bool,
    protocol_compatible: Option<bool>,
) -> ExternalControlClientDiagnostic {
    ExternalControlClientDiagnostic {
        gui_started,
        control_plane_reachable,
        protocol_compatible,
    }
}

fn check<'a>(report: &'a DoctorReport, id: &str) -> &'a DoctorCheck {
    report.checks.iter().find(|check| check.id == id).unwrap()
}

#[test]
fn doctor_parses_as_root_command() {
    assert!(matches!(
        parse(&["exaterm-cli", "doctor"]),
        RootCommand::Doctor
    ));
}

#[test]
fn doctor_report_succeeds_only_when_every_check_passes() {
    let report = build_doctor_report(
        Ok((true, true)),
        true,
        client_diagnostic(false, true, Some(true)),
    );

    assert!(report.ok);
    assert_eq!(doctor_exit_code(&report), 0);
    assert_eq!(report.checks.len(), 6);
    assert!(report
        .checks
        .iter()
        .all(|check| check.status == DoctorCheckStatus::Pass));
}

#[test]
fn doctor_report_continues_independent_checks_after_config_failure() {
    let report = build_doctor_report(Err(()), true, client_diagnostic(true, true, Some(true)));

    assert!(!report.ok);
    assert_eq!(doctor_exit_code(&report), 1);
    assert_eq!(check(&report, "config").status, DoctorCheckStatus::Fail);
    assert_eq!(
        check(&report, "external_control").status,
        DoctorCheckStatus::Skipped
    );
    assert_eq!(
        check(&report, "cli_permission").status,
        DoctorCheckStatus::Skipped
    );
    assert_eq!(
        check(&report, "control_plane").status,
        DoctorCheckStatus::Pass
    );
    assert_eq!(check(&report, "protocol").status, DoctorCheckStatus::Pass);
    assert!(report.gui_started);
}

#[test]
fn doctor_report_marks_disabled_permissions_as_failures() {
    let report = build_doctor_report(
        Ok((false, false)),
        true,
        client_diagnostic(false, true, Some(true)),
    );

    assert_eq!(
        check(&report, "external_control").status,
        DoctorCheckStatus::Fail
    );
    assert_eq!(
        check(&report, "cli_permission").status,
        DoctorCheckStatus::Fail
    );
    assert_eq!(doctor_exit_code(&report), 1);
}

#[test]
fn doctor_report_skips_protocol_when_control_plane_is_unavailable() {
    let report = build_doctor_report(
        Ok((true, true)),
        false,
        client_diagnostic(false, false, None),
    );

    assert_eq!(
        check(&report, "gui_executable").status,
        DoctorCheckStatus::Fail
    );
    assert_eq!(
        check(&report, "control_plane").status,
        DoctorCheckStatus::Fail
    );
    assert_eq!(
        check(&report, "protocol").status,
        DoctorCheckStatus::Skipped
    );
}

#[test]
fn doctor_report_marks_protocol_mismatch_without_claiming_gui_start() {
    let report = build_doctor_report(
        Ok((true, true)),
        true,
        client_diagnostic(false, true, Some(false)),
    );

    assert!(!report.gui_started);
    assert_eq!(check(&report, "protocol").status, DoctorCheckStatus::Fail);
}

#[test]
fn doctor_report_json_does_not_contain_local_paths_or_configuration_values() {
    let report = build_doctor_report(Err(()), false, client_diagnostic(false, false, None));
    let serialized = serde_json::to_string(&report).unwrap();

    assert!(!serialized.contains(r"C:\\Users\\example"));
    assert!(!serialized.contains("config.json"));
    assert!(!serialized.contains("session_id"));
    assert_eq!(
        serde_json::from_str::<Value>(&serialized).unwrap()["ok"],
        false
    );
}

#[test]
fn profile_connect_includes_connection_type() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "profiles",
            "connect",
            "--type",
            "telnet",
            "--profile-id",
            "router",
        ]),
        &mut io::empty(),
    )
    .unwrap();
    assert_eq!(
        request,
        ExternalControlRequest::ConnectSavedProfile(ConnectSavedProfileArgs {
            profile_id: "router".into(),
            connection_type: SavedProfileConnectionType::Telnet,
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn direct_ssh_connect_builds_a_typed_request() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "ssh",
            "connect",
            "--host",
            "router.example.test",
            "--port",
            "2222",
            "--username",
            "admin",
            "--auth-method",
            "public-key",
            "--private-key-path",
            "id_ed25519",
            "--jump-profile-id",
            "bastion",
            "--encoding",
            "shift-jis",
            "--terminal-mode",
            "juniper-junos",
            "--cols",
            "132",
            "--rows",
            "43",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSsh(ConnectSshArgs {
            host: "router.example.test".into(),
            port: Some(2222),
            username: "admin".into(),
            auth_method: Some(ExternalControlSshAuthMethod::PublicKey),
            private_key_path: Some("id_ed25519".into()),
            jump_profile_id: Some("bastion".into()),
            encoding: Some(ExternalControlEncoding::ShiftJis),
            terminal_mode: Some(ExternalControlTerminalMode::JuniperJunos),
            cols: Some(132),
            rows: Some(43),
        })
    );
}

#[test]
fn direct_telnet_connect_builds_a_typed_request() {
    let request = build_request(
        parse(&["exaterm-cli", "telnet", "connect", "--host", "192.0.2.10"]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectTelnet(ConnectTelnetArgs {
            host: "192.0.2.10".into(),
            port: None,
            encoding: None,
            terminal_mode: None,
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn direct_connect_rejects_zero_port_and_missing_username() {
    let port_error = build_request(
        parse(&[
            "exaterm-cli",
            "telnet",
            "connect",
            "--host",
            "router.example.test",
            "--port",
            "0",
        ]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(port_error.contains("--port"));

    let username_error = build_request(
        parse(&[
            "exaterm-cli",
            "ssh",
            "connect",
            "--host",
            "router.example.test",
            "--username",
            " ",
        ]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(username_error.contains("--username"));

    let host_error = build_request(
        parse(&[
            "exaterm-cli",
            "telnet",
            "connect",
            "--host",
            "router.example.test:23",
        ]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(host_error.contains("port"));
}

#[test]
fn profile_list_without_type_requests_all_profiles() {
    let request = build_request(
        parse(&["exaterm-cli", "profiles", "list"]),
        &mut io::empty(),
    )
    .unwrap();
    assert_eq!(
        request,
        ExternalControlRequest::ListConnectionProfiles(ListConnectionProfilesArgs {
            connection_type: None,
        })
    );
}

#[test]
fn profile_list_includes_connection_type() {
    let ssh_request = build_request(
        parse(&["exaterm-cli", "profiles", "list", "--type", "ssh"]),
        &mut io::empty(),
    )
    .unwrap();
    let telnet_request = build_request(
        parse(&["exaterm-cli", "profiles", "list", "--type", "telnet"]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        ssh_request,
        ExternalControlRequest::ListConnectionProfiles(ListConnectionProfilesArgs {
            connection_type: Some(SavedProfileConnectionType::Ssh),
        })
    );
    assert_eq!(
        telnet_request,
        ExternalControlRequest::ListConnectionProfiles(ListConnectionProfilesArgs {
            connection_type: Some(SavedProfileConnectionType::Telnet),
        })
    );
}

#[test]
fn profile_list_rejects_unknown_connection_type() {
    let error =
        Cli::try_parse_from(["exaterm-cli", "profiles", "list", "--type", "serial"]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidValue);
}

#[test]
fn output_delta_requires_cursor() {
    let error = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "output",
            "--session-id",
            "s1",
            "--mode",
            "delta",
        ]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(error.contains("requires --cursor"));
}

#[test]
fn output_recent_rejects_wait_arguments() {
    let error = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "output",
            "--session-id",
            "s1",
            "--mode",
            "recent",
            "--timeout-ms",
            "1000",
        ]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(error.contains("recent mode"));
}

#[test]
fn send_reads_dash_value_from_stdin() {
    let mut input = "show version\n".as_bytes();
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "send",
            "--session-id",
            "s1",
            "--data",
            "-",
        ]),
        &mut input,
    )
    .unwrap();
    assert_eq!(
        request,
        ExternalControlRequest::SendTerminalInput(SendTerminalInputArgs {
            session_id: "s1".into(),
            data: "show version\n".into(),
        })
    );
}

#[test]
fn serial_rejects_invalid_data_bits() {
    let error = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--data-bits",
            "9",
        ]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(error.contains("--data-bits"));
}

#[test]
fn log_rejects_empty_session_id() {
    let error = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "log",
            "start",
            "--session-id",
            " ",
        ]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(error.contains("--session-id"));
}

#[test]
fn sessions_list_builds_request() {
    assert_eq!(
        build_request(
            parse(&["exaterm-cli", "sessions", "list"]),
            &mut io::empty()
        )
        .unwrap(),
        ExternalControlRequest::ListTerminalSessions
    );
}

#[test]
fn sessions_disconnect_builds_request() {
    assert_eq!(
        build_request(
            parse(&[
                "exaterm-cli",
                "sessions",
                "disconnect",
                "--session-id",
                "s1",
            ]),
            &mut io::empty(),
        )
        .unwrap(),
        ExternalControlRequest::DisconnectTerminalSession(DisconnectTerminalSessionArgs {
            session_id: "s1".into(),
        })
    );
}

#[test]
fn sessions_focus_builds_request_and_rejects_missing_or_empty_id() {
    assert_eq!(
        build_request(
            parse(&["exaterm-cli", "sessions", "focus", "--session-id", "s1"]),
            &mut io::empty(),
        )
        .unwrap(),
        ExternalControlRequest::FocusTerminalSession(
            crate::external_control::FocusTerminalSessionArgs {
                session_id: "s1".into()
            },
        )
    );
    assert!(Cli::try_parse_from(["exaterm-cli", "sessions", "focus"]).is_err());
    assert!(build_request(
        parse(&["exaterm-cli", "sessions", "focus", "--session-id", " "]),
        &mut io::empty(),
    )
    .unwrap_err()
    .contains("--session-id"));
}

#[test]
fn sessions_disconnect_rejects_empty_session_id() {
    let error = build_request(
        parse(&["exaterm-cli", "sessions", "disconnect", "--session-id", " "]),
        &mut io::empty(),
    )
    .unwrap_err();
    assert!(error.contains("--session-id"));
}

#[test]
fn serial_ports_builds_request() {
    assert_eq!(
        build_request(parse(&["exaterm-cli", "serial", "ports"]), &mut io::empty()).unwrap(),
        ExternalControlRequest::ListSerialPorts
    );
}

#[test]
fn serial_connect_builds_request() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--baud-rate",
            "115200",
            "--data-bits",
            "7",
            "--parity",
            "even",
            "--stop-bits",
            "2",
            "--flow-control",
            "hardware",
            "--terminal-mode",
            "cisco-ios",
            "--cols",
            "140",
            "--rows",
            "40",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSerialConsole(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: Some(115200),
            data_bits: Some(7),
            parity: Some(ExternalControlSerialParity::Even),
            stop_bits: Some(2),
            flow_control: Some(ExternalControlSerialFlowControl::Hardware),
            terminal_mode: Some(ExternalControlTerminalMode::CiscoIos),
            cols: Some(140),
            rows: Some(40),
        })
    );
}

#[test]
fn serial_connect_accepts_arista_eos_terminal_mode() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--terminal-mode",
            "arista-eos",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSerialConsole(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(ExternalControlTerminalMode::AristaEos),
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn serial_connect_accepts_juniper_junos_terminal_mode() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--terminal-mode",
            "juniper-junos",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSerialConsole(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(ExternalControlTerminalMode::JuniperJunos),
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn serial_connect_accepts_vyos_terminal_mode() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--terminal-mode",
            "vyos",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSerialConsole(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(ExternalControlTerminalMode::Vyos),
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn serial_connect_accepts_fujitsu_sir_terminal_mode() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--terminal-mode",
            "fujitsu-sir",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSerialConsole(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(ExternalControlTerminalMode::FujitsuSir),
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn serial_connect_accepts_allied_telesis_awplus_terminal_mode() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--terminal-mode",
            "allied-telesis-awplus",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSerialConsole(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(ExternalControlTerminalMode::AlliedTelesisAwplus),
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn serial_connect_accepts_furukawa_fitelnet_terminal_mode() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "serial",
            "connect",
            "--port",
            "COM3",
            "--terminal-mode",
            "furukawa-fitelnet",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ConnectSerialConsole(ConnectSerialConsoleArgs {
            port: "COM3".into(),
            baud_rate: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            terminal_mode: Some(ExternalControlTerminalMode::FurukawaFitelnet),
            cols: None,
            rows: None,
        })
    );
}

#[test]
fn output_recent_builds_request() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "output",
            "--session-id",
            "s1",
            "--mode",
            "recent",
            "--max-chars",
            "1200",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Recent {
            session_id: "s1".into(),
            max_chars: Some(1200),
        })
    );
}

#[test]
fn output_delta_builds_request() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "output",
            "--session-id",
            "s1",
            "--mode",
            "delta",
            "--cursor",
            "120",
            "--max-chars",
            "800",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Delta {
            session_id: "s1".into(),
            cursor: 120,
            max_chars: Some(800),
        })
    );
}

#[test]
fn output_wait_builds_request() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "output",
            "--session-id",
            "s1",
            "--mode",
            "wait",
            "--cursor",
            "121",
            "--contains",
            "router#",
            "--timeout-ms",
            "30000",
            "--max-chars",
            "900",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Wait {
            session_id: "s1".into(),
            cursor: Some(121),
            contains: Some("router#".into()),
            timeout_ms: Some(30000),
            max_chars: Some(900),
        })
    );
}

#[test]
fn terminal_run_builds_request() {
    let request = build_request(
        parse(&[
            "exaterm-cli",
            "terminal",
            "run",
            "--session-id",
            "s1",
            "--command",
            "show version",
            "--append-newline",
            "false",
            "--wait-contains",
            "router#",
            "--timeout-ms",
            "5000",
            "--settle-ms",
            "10",
            "--max-chars",
            "1500",
        ]),
        &mut io::empty(),
    )
    .unwrap();

    assert_eq!(
        request,
        ExternalControlRequest::RunTerminalCommand(RunTerminalCommandArgs {
            session_id: "s1".into(),
            command: "show version".into(),
            append_newline: Some(false),
            wait_contains: Some("router#".into()),
            timeout_ms: Some(5000),
            settle_ms: Some(10),
            max_chars: Some(1500),
        })
    );
}

#[test]
fn terminal_log_start_builds_request() {
    assert_eq!(
        build_request(
            parse(&[
                "exaterm-cli",
                "terminal",
                "log",
                "start",
                "--session-id",
                "s1",
            ]),
            &mut io::empty()
        )
        .unwrap(),
        ExternalControlRequest::StartTerminalLog(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: None,
            write_mode: None,
        })
    );
}

#[test]
fn terminal_log_start_builds_custom_path_and_mode() {
    let base_dir = std::env::current_dir().unwrap();
    let expected = base_dir.join("logs").join("session.log");
    assert_eq!(
        build_request(
            parse(&[
                "exaterm-cli",
                "terminal",
                "log",
                "start",
                "--session-id",
                "s1",
                "--file-path",
                "logs\\session.log",
                "--write-mode",
                "append",
            ]),
            &mut io::empty()
        )
        .unwrap(),
        ExternalControlRequest::StartTerminalLog(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: Some(expected.to_string_lossy().to_string()),
            write_mode: Some(ExternalControlLogWriteMode::Append),
        })
    );
}

#[test]
fn terminal_log_start_preserves_absolute_path_and_overwrite_mode() {
    let file_path = r"C:\logs\session.log";
    assert_eq!(
        build_request(
            parse(&[
                "exaterm-cli",
                "terminal",
                "log",
                "start",
                "--session-id",
                "s1",
                "--file-path",
                file_path,
                "--write-mode",
                "overwrite",
            ]),
            &mut io::empty()
        )
        .unwrap(),
        ExternalControlRequest::StartTerminalLog(StartTerminalLogArgs {
            session_id: "s1".into(),
            file_path: Some(file_path.into()),
            write_mode: Some(ExternalControlLogWriteMode::Overwrite),
        })
    );
}

#[test]
fn terminal_log_start_requires_path_and_mode_together() {
    for args in [
        vec![
            "exaterm-cli",
            "terminal",
            "log",
            "start",
            "--session-id",
            "s1",
            "--file-path",
            "session.log",
        ],
        vec![
            "exaterm-cli",
            "terminal",
            "log",
            "start",
            "--session-id",
            "s1",
            "--write-mode",
            "overwrite",
        ],
    ] {
        let error = build_request(parse(&args), &mut io::empty()).unwrap_err();
        assert!(error.contains("specified together"));
    }
}

#[test]
fn terminal_log_status_pause_and_resume_build_requests() {
    for (command, expected) in [
        (
            "status",
            ExternalControlRequest::GetTerminalLogStatus(TerminalLogSessionArgs {
                session_id: "s1".into(),
            }),
        ),
        (
            "pause",
            ExternalControlRequest::PauseTerminalLog(TerminalLogSessionArgs {
                session_id: "s1".into(),
            }),
        ),
        (
            "resume",
            ExternalControlRequest::ResumeTerminalLog(TerminalLogSessionArgs {
                session_id: "s1".into(),
            }),
        ),
    ] {
        assert_eq!(
            build_request(
                parse(&[
                    "exaterm-cli",
                    "terminal",
                    "log",
                    command,
                    "--session-id",
                    "s1",
                ]),
                &mut io::empty()
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn terminal_log_stop_builds_request() {
    assert_eq!(
        build_request(
            parse(&[
                "exaterm-cli",
                "terminal",
                "log",
                "stop",
                "--session-id",
                "s1",
            ]),
            &mut io::empty()
        )
        .unwrap(),
        ExternalControlRequest::StopTerminalLog(StopTerminalLogArgs {
            session_id: "s1".into(),
        })
    );
}

#[test]
fn not_found_uses_invalid_arguments_exit_code() {
    assert_eq!(
        classify_external_control_error(&ExternalControlError::NotFound("missing".into())),
        ("invalid_arguments", 2)
    );
}

#[test]
fn permission_denied_uses_tool_error_exit_code() {
    assert_eq!(
        classify_external_control_error(&ExternalControlError::PermissionDenied("denied".into())),
        ("tool_error", 1)
    );
}

#[test]
fn print_response_serializes_result_value() {
    let response = ExternalControlResponse::ListTerminalSessions(ListTerminalSessionsResult {
        sessions: Vec::new(),
    });

    let serialized = serde_json::to_string(&response.into_value().unwrap()).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&serialized).unwrap(),
        json!({ "sessions": [] })
    );
}
