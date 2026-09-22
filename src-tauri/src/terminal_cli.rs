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
        ExternalControlError, ExternalControlRequest, ExternalControlResponse,
        ReadTerminalOutputArgs, RunTerminalCommandArgs, SendTerminalInputArgs,
        StartTerminalLogArgs, StopTerminalLogArgs, TerminalLogSessionArgs,
    },
};

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
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputMode {
    Recent,
    Delta,
    Wait,
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

    let request = match build_request(cli.command, &mut io::stdin()) {
        Ok(request) => request,
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
mod tests {
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
        let error = Cli::try_parse_from(["exaterm-cli", "profiles", "list", "--type", "serial"])
            .unwrap_err();
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
            classify_external_control_error(&ExternalControlError::PermissionDenied(
                "denied".into()
            )),
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
}
