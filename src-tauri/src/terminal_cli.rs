use std::io::{self, Read};
use std::path::{Path, PathBuf};

use clap::{error::ErrorKind, Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::json;

use crate::external_control::service::ExternalControlLogWriteMode;
use crate::{
    config,
    external_control::{
        client::{ExternalControlClient, ExternalControlClientDiagnostic},
        protocol::CONTROL_PROTOCOL_VERSION,
        service::{
            normalize_direct_host, ExternalControlEncoding, ExternalControlSerialFlowControl,
            ExternalControlSerialParity, ExternalControlSshAuthMethod, ExternalControlTerminalMode,
            ListConnectionProfilesArgs, SavedProfileConnectionType,
        },
        ConnectSavedProfileArgs, ConnectSerialConsoleArgs, ConnectSshArgs, ConnectTelnetArgs,
        DisconnectTerminalSessionArgs, ExternalControlError, ExternalControlRequest,
        ExternalControlResponse, ReadTerminalOutputArgs, RunTerminalCommandArgs,
        SendTerminalInputArgs, StartTerminalLogArgs, StopTerminalLogArgs, TerminalLogSessionArgs,
    },
};

mod follow;
use follow::{run_follow, FollowOptions};

#[derive(Debug, Parser)]
#[command(
    name = "exaterm-cli",
    bin_name = "exaterm-cli",
    version,
    about = "Control ExaTerm terminal sessions"
)]
struct Cli {
    #[command(subcommand)]
    command: RootCommand,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    /// Diagnose ExaTerm CLI availability.
    Doctor,
    Sessions(SessionsArgs),
    Profiles(ProfilesArgs),
    Ssh(SshArgs),
    Telnet(TelnetArgs),
    Serial(SerialArgs),
    Terminal(TerminalArgs),
}

#[derive(Debug, Args)]
struct SessionsArgs {
    #[command(subcommand)]
    command: SessionsCommand,
}

#[derive(Debug, Subcommand)]
enum SessionsCommand {
    List,
    /// Select a session tab and bring its window to the foreground.
    Focus(SessionArg),
    Disconnect(SessionArg),
}

#[derive(Debug, Args)]
struct ProfilesArgs {
    #[command(subcommand)]
    command: ProfilesCommand,
}

#[derive(Debug, Subcommand)]
enum ProfilesCommand {
    List(ProfileListArgs),
    Connect(ProfileConnectArgs),
}

#[derive(Debug, Args)]
struct ProfileListArgs {
    #[arg(long = "type", value_enum)]
    connection_type: Option<ProfileType>,
}

#[derive(Debug, Args)]
struct ProfileConnectArgs {
    #[arg(long = "type", value_enum)]
    connection_type: ProfileType,
    #[arg(long)]
    profile_id: String,
    #[arg(long)]
    cols: Option<u32>,
    #[arg(long)]
    rows: Option<u32>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ProfileType {
    Ssh,
    Telnet,
}

#[derive(Debug, Args)]
struct SshArgs {
    #[command(subcommand)]
    command: SshCommand,
}

#[derive(Debug, Subcommand)]
enum SshCommand {
    Connect(SshConnectArgs),
}

#[derive(Debug, Args)]
struct SshConnectArgs {
    #[arg(long)]
    host: String,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    username: String,
    #[arg(long, value_enum)]
    auth_method: Option<SshAuthMethod>,
    #[arg(long)]
    private_key_path: Option<String>,
    #[arg(long)]
    jump_profile_id: Option<String>,
    #[arg(long, value_enum)]
    encoding: Option<Encoding>,
    #[arg(long, value_enum)]
    terminal_mode: Option<TerminalMode>,
    #[arg(long)]
    cols: Option<u32>,
    #[arg(long)]
    rows: Option<u32>,
}

#[derive(Debug, Args)]
struct TelnetArgs {
    #[command(subcommand)]
    command: TelnetCommand,
}

#[derive(Debug, Subcommand)]
enum TelnetCommand {
    Connect(TelnetConnectArgs),
}

#[derive(Debug, Args)]
struct TelnetConnectArgs {
    #[arg(long)]
    host: String,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long, value_enum)]
    encoding: Option<Encoding>,
    #[arg(long, value_enum)]
    terminal_mode: Option<TerminalMode>,
    #[arg(long)]
    cols: Option<u32>,
    #[arg(long)]
    rows: Option<u32>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SshAuthMethod {
    Auto,
    Password,
    KeyboardInteractive,
    PublicKey,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Encoding {
    Utf8,
    ShiftJis,
    EucJp,
}

#[derive(Debug, Args)]
struct SerialArgs {
    #[command(subcommand)]
    command: SerialCommand,
}

#[derive(Debug, Subcommand)]
enum SerialCommand {
    Ports,
    Connect(SerialConnectArgs),
}

#[derive(Debug, Args)]
struct SerialConnectArgs {
    #[arg(long)]
    port: String,
    #[arg(long)]
    baud_rate: Option<u32>,
    #[arg(long)]
    data_bits: Option<u8>,
    #[arg(long, value_enum)]
    parity: Option<Parity>,
    #[arg(long)]
    stop_bits: Option<u8>,
    #[arg(long, value_enum)]
    flow_control: Option<FlowControl>,
    #[arg(long, value_enum)]
    terminal_mode: Option<TerminalMode>,
    #[arg(long)]
    cols: Option<u32>,
    #[arg(long)]
    rows: Option<u32>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Parity {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FlowControl {
    None,
    Software,
    Hardware,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TerminalMode {
    General,
    CiscoIos,
    AristaEos,
    JuniperJunos,
    Vyos,
    FujitsuSir,
    AlliedTelesisAwplus,
    FurukawaFitelnet,
}

#[derive(Debug, Args)]
struct TerminalArgs {
    #[command(subcommand)]
    command: TerminalCommand,
}

#[derive(Debug, Subcommand)]
enum TerminalCommand {
    Output(OutputArgs),
    Send(SendArgs),
    Run(RunArgs),
    Log(LogArgs),
}

#[derive(Debug, Args)]
struct OutputArgs {
    #[arg(long)]
    session_id: String,
    #[arg(long, value_enum)]
    mode: OutputMode,
    #[arg(long)]
    cursor: Option<usize>,
    #[arg(long)]
    contains: Option<String>,
    #[arg(long)]
    timeout_ms: Option<u64>,
    #[arg(long)]
    max_chars: Option<usize>,
    #[arg(long)]
    duration_ms: Option<u64>,
    #[arg(long)]
    max_total_chars: Option<usize>,
    #[arg(long)]
    until: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputMode {
    Recent,
    Delta,
    Wait,
    Follow,
}

#[derive(Debug, Args)]
struct SendArgs {
    #[arg(long)]
    session_id: String,
    #[arg(long)]
    data: String,
}

#[derive(Debug, Args)]
struct RunArgs {
    #[arg(long)]
    session_id: String,
    #[arg(long)]
    command: String,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    append_newline: bool,
    #[arg(long)]
    wait_contains: Option<String>,
    #[arg(long)]
    timeout_ms: Option<u64>,
    #[arg(long)]
    settle_ms: Option<u64>,
    #[arg(long)]
    max_chars: Option<usize>,
}

#[derive(Debug, Args)]
struct LogArgs {
    #[command(subcommand)]
    command: LogCommand,
}

#[derive(Debug, Subcommand)]
enum LogCommand {
    Start(StartLogArgs),
    Stop(SessionArg),
    Status(SessionArg),
    Pause(SessionArg),
    Resume(SessionArg),
}

#[derive(Debug, Args)]
struct StartLogArgs {
    #[arg(long)]
    session_id: String,
    #[arg(long)]
    file_path: Option<String>,
    #[arg(long, value_enum)]
    write_mode: Option<LogWriteMode>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogWriteMode {
    Overwrite,
    Append,
}

#[derive(Debug, Args)]
struct SessionArg {
    #[arg(long)]
    session_id: String,
}

pub async fn run_terminal_cli() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            let _ = error.print();
            return 0;
        }
        Err(error) => {
            print_error("invalid_arguments", &error.to_string());
            return 2;
        }
    };

    if matches!(&cli.command, RootCommand::Doctor) {
        return run_doctor().await;
    }

    let command = match cli.command {
        RootCommand::Terminal(TerminalArgs {
            command: TerminalCommand::Output(args),
        }) if matches!(args.mode, OutputMode::Follow) => {
            follow::build_options(args).map(CliExecution::Follow)
        }
        command => build_request(command, &mut io::stdin()).map(CliExecution::Once),
    };
    let command = match command {
        Ok(command) => command,
        Err(error) => {
            print_error("invalid_arguments", &error);
            return 2;
        }
    };

    let app_config = match config::config_read() {
        Ok(config) => config,
        Err(error) => {
            print_error("config_error", &error);
            return 1;
        }
    };
    if !app_config.external_control.enabled || !app_config.external_control.cli_enabled {
        print_error(
            "cli_disabled",
            "ExaTerm CLI is disabled. Set external_control.enabled=true and external_control.cli_enabled=true.",
        );
        return 1;
    }

    let client = ExternalControlClient::new();
    if let Err(error) = client.discover_or_start_gui().await {
        print_error("control_unavailable", &error);
        return 1;
    }

    if let CliExecution::Follow(options) = command {
        return match run_follow(&client, &mut io::stdout(), options, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        {
            Ok(()) => 0,
            Err(follow::FollowError::Control(error)) => {
                let (code, exit_code) = classify_external_control_error(&error);
                print_error(code, error.message());
                exit_code
            }
            Err(follow::FollowError::Output(error)) => {
                print_error(
                    "tool_error",
                    &format!("Failed to write follow output: {error}"),
                );
                1
            }
        };
    }
    let CliExecution::Once(request) = command else {
        unreachable!();
    };
    match client.call(request).await {
        Ok(result) => {
            print_response(result);
            0
        }
        Err(error) => {
            let (code, exit_code) = classify_external_control_error(&error);
            print_error(code, error.message());
            exit_code
        }
    }
}

enum CliExecution {
    Once(ExternalControlRequest),
    Follow(FollowOptions),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum DoctorCheckStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct DoctorCheck {
    id: &'static str,
    status: DoctorCheckStatus,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    remediation: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct DoctorReport {
    ok: bool,
    version: &'static str,
    protocol_version: u32,
    gui_started: bool,
    checks: Vec<DoctorCheck>,
}

async fn run_doctor() -> i32 {
    let config = config::config_read().map(|config| {
        (
            config.external_control.enabled,
            config.external_control.cli_enabled,
        )
    });
    let client = ExternalControlClient::new();
    let gui_executable_available = client.gui_executable_available();
    let client_diagnostic = client.diagnose_or_start_gui().await;
    let report = build_doctor_report(
        config.map_err(|_| ()),
        gui_executable_available,
        client_diagnostic,
    );
    let exit_code = doctor_exit_code(&report);
    let output = serde_json::to_string(&report).unwrap_or_else(|_| "{}".into());
    println!("{output}");
    exit_code
}

fn doctor_exit_code(report: &DoctorReport) -> i32 {
    if report.ok {
        0
    } else {
        1
    }
}

fn build_doctor_report(
    config_permissions: Result<(bool, bool), ()>,
    gui_executable_available: bool,
    client: ExternalControlClientDiagnostic,
) -> DoctorReport {
    let mut checks = Vec::with_capacity(6);

    match config_permissions {
        Ok((external_control_enabled, cli_enabled)) => {
            checks.push(passed_check(
                "config",
                "ExaTerm configuration loaded successfully.",
            ));
            checks.push(if external_control_enabled {
                passed_check("external_control", "ExaTerm external control is enabled.")
            } else {
                failed_check(
                    "external_control",
                    "ExaTerm external control is disabled.",
                    "Set external_control.enabled=true and restart ExaTerm.",
                )
            });
            checks.push(if cli_enabled {
                passed_check("cli_permission", "ExaTerm CLI access is enabled.")
            } else {
                failed_check(
                    "cli_permission",
                    "ExaTerm CLI access is disabled.",
                    "Set external_control.cli_enabled=true and restart ExaTerm.",
                )
            });
        }
        Err(()) => {
            checks.push(failed_check(
                "config",
                "ExaTerm configuration could not be loaded.",
                "Repair or replace the ExaTerm configuration file.",
            ));
            checks.push(skipped_check(
                "external_control",
                "External control setting was not checked because configuration loading failed.",
                "Repair the ExaTerm configuration file, then run doctor again.",
            ));
            checks.push(skipped_check(
                "cli_permission",
                "CLI permission was not checked because configuration loading failed.",
                "Repair the ExaTerm configuration file, then run doctor again.",
            ));
        }
    }

    checks.push(if gui_executable_available {
        passed_check("gui_executable", "ExaTerm GUI executable was found.")
    } else {
        failed_check(
            "gui_executable",
            "ExaTerm GUI executable was not found near the CLI.",
            "Install exaterm-cli beside the ExaTerm GUI executable.",
        )
    });

    checks.push(if client.control_plane_reachable {
        passed_check("control_plane", "ExaTerm control plane is reachable.")
    } else {
        failed_check(
            "control_plane",
            "ExaTerm control plane is unavailable.",
            "Confirm that ExaTerm can start, then run doctor again.",
        )
    });

    checks.push(match client.protocol_compatible {
        Some(true) => passed_check(
            "protocol",
            "ExaTerm external control protocol is compatible.",
        ),
        Some(false) => failed_check(
            "protocol",
            "ExaTerm external control protocol handshake failed.",
            "Use matching ExaTerm GUI and CLI versions, restart ExaTerm, then run doctor again.",
        ),
        None => skipped_check(
            "protocol",
            "Protocol compatibility was not checked because the control plane is unavailable.",
            "Restore the ExaTerm control plane, then run doctor again.",
        ),
    });

    let ok = checks
        .iter()
        .all(|check| check.status == DoctorCheckStatus::Pass);
    DoctorReport {
        ok,
        version: env!("CARGO_PKG_VERSION"),
        protocol_version: CONTROL_PROTOCOL_VERSION,
        gui_started: client.gui_started,
        checks,
    }
}

fn passed_check(id: &'static str, message: &'static str) -> DoctorCheck {
    DoctorCheck {
        id,
        status: DoctorCheckStatus::Pass,
        message,
        remediation: None,
    }
}

fn failed_check(id: &'static str, message: &'static str, remediation: &'static str) -> DoctorCheck {
    DoctorCheck {
        id,
        status: DoctorCheckStatus::Fail,
        message,
        remediation: Some(remediation),
    }
}

fn skipped_check(
    id: &'static str,
    message: &'static str,
    remediation: &'static str,
) -> DoctorCheck {
    DoctorCheck {
        id,
        status: DoctorCheckStatus::Skipped,
        message,
        remediation: Some(remediation),
    }
}

fn build_request(
    command: RootCommand,
    stdin: &mut impl Read,
) -> Result<ExternalControlRequest, String> {
    match command {
        RootCommand::Doctor => Err("doctor does not create an external control request".into()),
        RootCommand::Sessions(SessionsArgs {
            command: SessionsCommand::List,
        }) => Ok(ExternalControlRequest::ListTerminalSessions),
        RootCommand::Sessions(SessionsArgs {
            command: SessionsCommand::Focus(args),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            Ok(ExternalControlRequest::FocusTerminalSession(
                crate::external_control::FocusTerminalSessionArgs {
                    session_id: args.session_id,
                },
            ))
        }
        RootCommand::Sessions(SessionsArgs {
            command: SessionsCommand::Disconnect(args),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            Ok(ExternalControlRequest::DisconnectTerminalSession(
                DisconnectTerminalSessionArgs {
                    session_id: args.session_id,
                },
            ))
        }
        RootCommand::Profiles(ProfilesArgs {
            command: ProfilesCommand::List(args),
        }) => Ok(ExternalControlRequest::ListConnectionProfiles(
            ListConnectionProfilesArgs {
                connection_type: args.connection_type.map(ProfileType::into_request_type),
            },
        )),
        RootCommand::Profiles(ProfilesArgs {
            command: ProfilesCommand::Connect(args),
        }) => {
            require_non_empty("--profile-id", &args.profile_id)?;
            validate_dimensions(args.cols, args.rows)?;
            Ok(ExternalControlRequest::ConnectSavedProfile(
                ConnectSavedProfileArgs {
                    profile_id: args.profile_id,
                    connection_type: args.connection_type.into_request_type(),
                    cols: args.cols,
                    rows: args.rows,
                },
            ))
        }
        RootCommand::Ssh(SshArgs {
            command: SshCommand::Connect(args),
        }) => {
            let host = normalize_direct_host(&args.host)?;
            require_non_empty("--username", &args.username)?;
            validate_optional_range("--port", args.port, 1, u16::MAX)?;
            validate_dimensions(args.cols, args.rows)?;
            Ok(ExternalControlRequest::ConnectSsh(ConnectSshArgs {
                host,
                port: args.port,
                username: args.username,
                auth_method: args
                    .auth_method
                    .map(SshAuthMethod::into_request_auth_method),
                private_key_path: args.private_key_path,
                jump_profile_id: args.jump_profile_id,
                encoding: args.encoding.map(Encoding::into_request_encoding),
                terminal_mode: args
                    .terminal_mode
                    .map(TerminalMode::into_request_terminal_mode),
                cols: args.cols,
                rows: args.rows,
            }))
        }
        RootCommand::Telnet(TelnetArgs {
            command: TelnetCommand::Connect(args),
        }) => {
            let host = normalize_direct_host(&args.host)?;
            validate_optional_range("--port", args.port, 1, u16::MAX)?;
            validate_dimensions(args.cols, args.rows)?;
            Ok(ExternalControlRequest::ConnectTelnet(ConnectTelnetArgs {
                host,
                port: args.port,
                encoding: args.encoding.map(Encoding::into_request_encoding),
                terminal_mode: args
                    .terminal_mode
                    .map(TerminalMode::into_request_terminal_mode),
                cols: args.cols,
                rows: args.rows,
            }))
        }
        RootCommand::Serial(SerialArgs {
            command: SerialCommand::Ports,
        }) => Ok(ExternalControlRequest::ListSerialPorts),
        RootCommand::Serial(SerialArgs {
            command: SerialCommand::Connect(args),
        }) => {
            require_non_empty("--port", &args.port)?;
            validate_optional_range("--baud-rate", args.baud_rate, 1, u32::MAX)?;
            if let Some(data_bits) = args.data_bits {
                if !matches!(data_bits, 5..=8) {
                    return Err("--data-bits must be 5, 6, 7, or 8".into());
                }
            }
            if let Some(stop_bits) = args.stop_bits {
                if !matches!(stop_bits, 1 | 2) {
                    return Err("--stop-bits must be 1 or 2".into());
                }
            }
            validate_dimensions(args.cols, args.rows)?;
            Ok(ExternalControlRequest::ConnectSerialConsole(
                ConnectSerialConsoleArgs {
                    port: args.port,
                    baud_rate: args.baud_rate,
                    data_bits: args.data_bits,
                    parity: args.parity.map(Parity::into_request_parity),
                    stop_bits: args.stop_bits,
                    flow_control: args
                        .flow_control
                        .map(FlowControl::into_request_flow_control),
                    terminal_mode: args
                        .terminal_mode
                        .map(TerminalMode::into_request_terminal_mode),
                    cols: args.cols,
                    rows: args.rows,
                },
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command: TerminalCommand::Output(args),
        }) => build_output_request(args),
        RootCommand::Terminal(TerminalArgs {
            command: TerminalCommand::Send(args),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            let data = read_value(args.data, stdin)?;
            validate_input_length(&data)?;
            Ok(ExternalControlRequest::SendTerminalInput(
                SendTerminalInputArgs {
                    session_id: args.session_id,
                    data,
                },
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command: TerminalCommand::Run(args),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            let command = read_value(args.command, stdin)?;
            require_non_empty("--command", &command)?;
            validate_input_length(&command)?;
            validate_wait_options(args.timeout_ms, args.max_chars)?;
            validate_optional_range("--settle-ms", args.settle_ms, 0, 5_000)?;
            Ok(ExternalControlRequest::RunTerminalCommand(
                RunTerminalCommandArgs {
                    session_id: args.session_id,
                    command,
                    append_newline: Some(args.append_newline),
                    wait_contains: args.wait_contains,
                    timeout_ms: args.timeout_ms,
                    settle_ms: args.settle_ms,
                    max_chars: args.max_chars,
                },
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command:
                TerminalCommand::Log(LogArgs {
                    command: LogCommand::Start(args),
                }),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            let (file_path, write_mode) = match (args.file_path, args.write_mode) {
                (None, None) => (None, None),
                (Some(file_path), Some(write_mode)) => {
                    require_non_empty("--file-path", &file_path)?;
                    let base_dir = std::env::current_dir().map_err(|error| {
                        format!("Failed to resolve the current directory: {error}")
                    })?;
                    (
                        Some(resolve_log_file_path(&file_path, &base_dir)),
                        Some(write_mode.into_request_write_mode()),
                    )
                }
                _ => return Err("--file-path and --write-mode must be specified together".into()),
            };
            Ok(ExternalControlRequest::StartTerminalLog(
                StartTerminalLogArgs {
                    session_id: args.session_id,
                    file_path,
                    write_mode,
                },
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command:
                TerminalCommand::Log(LogArgs {
                    command: LogCommand::Stop(args),
                }),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            Ok(ExternalControlRequest::StopTerminalLog(
                StopTerminalLogArgs {
                    session_id: args.session_id,
                },
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command:
                TerminalCommand::Log(LogArgs {
                    command: LogCommand::Status(args),
                }),
        }) => build_log_session_request(args, ExternalControlRequest::GetTerminalLogStatus),
        RootCommand::Terminal(TerminalArgs {
            command:
                TerminalCommand::Log(LogArgs {
                    command: LogCommand::Pause(args),
                }),
        }) => build_log_session_request(args, ExternalControlRequest::PauseTerminalLog),
        RootCommand::Terminal(TerminalArgs {
            command:
                TerminalCommand::Log(LogArgs {
                    command: LogCommand::Resume(args),
                }),
        }) => build_log_session_request(args, ExternalControlRequest::ResumeTerminalLog),
    }
}

fn build_log_session_request(
    args: SessionArg,
    build: impl FnOnce(TerminalLogSessionArgs) -> ExternalControlRequest,
) -> Result<ExternalControlRequest, String> {
    require_non_empty("--session-id", &args.session_id)?;
    Ok(build(TerminalLogSessionArgs {
        session_id: args.session_id,
    }))
}

fn resolve_log_file_path(file_path: &str, base_dir: &Path) -> String {
    let path = PathBuf::from(file_path);
    let absolute = if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    };
    absolute.to_string_lossy().to_string()
}

fn build_output_request(args: OutputArgs) -> Result<ExternalControlRequest, String> {
    require_non_empty("--session-id", &args.session_id)?;
    validate_optional_range("--max-chars", args.max_chars, 1, 20_000)?;
    if args.duration_ms.is_some() || args.max_total_chars.is_some() || args.until.is_some() {
        return Err("--duration-ms, --max-total-chars, and --until require follow mode".into());
    }
    let request = match args.mode {
        OutputMode::Recent => {
            if args.cursor.is_some() || args.contains.is_some() || args.timeout_ms.is_some() {
                return Err(
                    "recent mode does not accept --cursor, --contains, or --timeout-ms".into(),
                );
            }
            ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Recent {
                session_id: args.session_id,
                max_chars: args.max_chars,
            })
        }
        OutputMode::Delta => {
            let cursor = args
                .cursor
                .ok_or_else(|| "delta mode requires --cursor".to_string())?;
            if args.contains.is_some() || args.timeout_ms.is_some() {
                return Err("delta mode does not accept --contains or --timeout-ms".into());
            }
            ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Delta {
                session_id: args.session_id,
                cursor,
                max_chars: args.max_chars,
            })
        }
        OutputMode::Wait => {
            validate_optional_range("--timeout-ms", args.timeout_ms, 1, 60_000)?;
            ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Wait {
                session_id: args.session_id,
                cursor: args.cursor,
                contains: args.contains,
                timeout_ms: args.timeout_ms,
                max_chars: args.max_chars,
            })
        }
        OutputMode::Follow => unreachable!("follow is handled by the CLI runner"),
    };

    Ok(request)
}

fn read_value(value: String, stdin: &mut impl Read) -> Result<String, String> {
    if value != "-" {
        return Ok(value);
    }
    let mut input = String::new();
    stdin
        .read_to_string(&mut input)
        .map_err(|error| format!("Failed to read stdin: {error}"))?;
    Ok(input)
}

fn validate_dimensions(cols: Option<u32>, rows: Option<u32>) -> Result<(), String> {
    validate_optional_range("--cols", cols, 1, 1_000)?;
    validate_optional_range("--rows", rows, 1, 1_000)
}

fn validate_wait_options(timeout_ms: Option<u64>, max_chars: Option<usize>) -> Result<(), String> {
    validate_optional_range("--timeout-ms", timeout_ms, 1, 60_000)?;
    validate_optional_range("--max-chars", max_chars, 1, 20_000)
}

fn validate_optional_range<T>(name: &str, value: Option<T>, min: T, max: T) -> Result<(), String>
where
    T: PartialOrd + std::fmt::Display + Copy,
{
    if let Some(value) = value {
        if value < min || value > max {
            return Err(format!("{name} must be between {min} and {max}"));
        }
    }
    Ok(())
}

fn require_non_empty(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{name} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_input_length(value: &str) -> Result<(), String> {
    if value.chars().count() > 20_000 {
        Err("terminal input must not exceed 20000 characters".into())
    } else {
        Ok(())
    }
}

fn print_response(response: ExternalControlResponse) {
    let output = response
        .into_value()
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .unwrap_or_else(|| "{}".into());
    println!("{output}");
}

fn print_error(code: &str, message: &str) {
    eprintln!(
        "{}",
        json!({ "error": { "code": code, "message": message } })
    );
}

fn classify_external_control_error(error: &ExternalControlError) -> (&'static str, i32) {
    match error {
        ExternalControlError::InvalidArguments(_) | ExternalControlError::NotFound(_) => {
            ("invalid_arguments", 2)
        }
        ExternalControlError::PermissionDenied(_)
        | ExternalControlError::Unavailable(_)
        | ExternalControlError::Internal(_) => ("tool_error", 1),
    }
}

impl ProfileType {
    fn into_request_type(self) -> SavedProfileConnectionType {
        match self {
            Self::Ssh => SavedProfileConnectionType::Ssh,
            Self::Telnet => SavedProfileConnectionType::Telnet,
        }
    }
}

impl SshAuthMethod {
    fn into_request_auth_method(self) -> ExternalControlSshAuthMethod {
        match self {
            Self::Auto => ExternalControlSshAuthMethod::Auto,
            Self::Password => ExternalControlSshAuthMethod::Password,
            Self::KeyboardInteractive => ExternalControlSshAuthMethod::KeyboardInteractive,
            Self::PublicKey => ExternalControlSshAuthMethod::PublicKey,
        }
    }
}

impl LogWriteMode {
    fn into_request_write_mode(self) -> ExternalControlLogWriteMode {
        match self {
            Self::Overwrite => ExternalControlLogWriteMode::Overwrite,
            Self::Append => ExternalControlLogWriteMode::Append,
        }
    }
}

impl Encoding {
    fn into_request_encoding(self) -> ExternalControlEncoding {
        match self {
            Self::Utf8 => ExternalControlEncoding::Utf8,
            Self::ShiftJis => ExternalControlEncoding::ShiftJis,
            Self::EucJp => ExternalControlEncoding::EucJp,
        }
    }
}

impl Parity {
    fn into_request_parity(self) -> ExternalControlSerialParity {
        match self {
            Self::None => ExternalControlSerialParity::None,
            Self::Odd => ExternalControlSerialParity::Odd,
            Self::Even => ExternalControlSerialParity::Even,
        }
    }
}

impl FlowControl {
    fn into_request_flow_control(self) -> ExternalControlSerialFlowControl {
        match self {
            Self::None => ExternalControlSerialFlowControl::None,
            Self::Software => ExternalControlSerialFlowControl::Software,
            Self::Hardware => ExternalControlSerialFlowControl::Hardware,
        }
    }
}

impl TerminalMode {
    fn into_request_terminal_mode(self) -> ExternalControlTerminalMode {
        match self {
            Self::General => ExternalControlTerminalMode::General,
            Self::CiscoIos => ExternalControlTerminalMode::CiscoIos,
            Self::AristaEos => ExternalControlTerminalMode::AristaEos,
            Self::JuniperJunos => ExternalControlTerminalMode::JuniperJunos,
            Self::Vyos => ExternalControlTerminalMode::Vyos,
            Self::FujitsuSir => ExternalControlTerminalMode::FujitsuSir,
            Self::AlliedTelesisAwplus => ExternalControlTerminalMode::AlliedTelesisAwplus,
            Self::FurukawaFitelnet => ExternalControlTerminalMode::FurukawaFitelnet,
        }
    }
}

#[cfg(test)]
mod tests;
