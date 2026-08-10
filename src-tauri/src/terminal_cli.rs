use std::io::{self, Read};

use clap::{error::ErrorKind, Args, Parser, Subcommand, ValueEnum};
use serde_json::json;

use crate::{
    config,
    external_control::{
        client::ExternalControlClient,
        service::{
            ExternalControlSerialFlowControl, ExternalControlSerialParity,
            ExternalControlTerminalMode, ListConnectionProfilesArgs, SavedProfileConnectionType,
        },
        ConnectSavedProfileArgs, ConnectSerialConsoleArgs, ExternalControlError,
        ExternalControlRequest, ExternalControlResponse, ReadTerminalOutputArgs,
        RunTerminalCommandArgs, SendTerminalInputArgs, StartTerminalLogArgs, StopTerminalLogArgs,
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
    Sessions(SessionsArgs),
    Profiles(ProfilesArgs),
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
    Vyos,
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
    Start(SessionArg),
    Stop(SessionArg),
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

fn build_request(
    command: RootCommand,
    stdin: &mut impl Read,
) -> Result<ExternalControlRequest, String> {
    match command {
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
            Ok(ExternalControlRequest::StartTerminalLog(
                StartTerminalLogArgs {
                    session_id: args.session_id,
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
    }
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
    println!(
        "{}",
        serde_json::to_string(&response.into_value()).unwrap_or_else(|_| "{}".into())
    );
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
            Self::Vyos => ExternalControlTerminalMode::Vyos,
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
            })
        );
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
        let response =
            ExternalControlResponse::ListTerminalSessions(ListTerminalSessionsResult(json!({
                "sessions": [{"session_id": "s1"}]
            })));

        let serialized = serde_json::to_string(&response.into_value()).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&serialized).unwrap(),
            json!({
                "sessions": [{"session_id": "s1"}]
            })
        );
    }
}
