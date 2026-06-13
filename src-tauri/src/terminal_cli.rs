use std::io::{self, Read};

use clap::{error::ErrorKind, Args, Parser, Subcommand, ValueEnum};
use rmcp::model::ErrorCode;
use serde_json::{json, Value};

use crate::{config, mcp::client::ControlClient};

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

impl ProfileType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::Telnet => "telnet",
        }
    }
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
}

impl TerminalMode {
    fn as_json_name(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::CiscoIos => "cisco_ios",
        }
    }
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

#[derive(Debug)]
struct CliCall {
    tool_name: &'static str,
    args: Value,
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

    let call = match build_call(cli.command, &mut io::stdin()) {
        Ok(call) => call,
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
    if !app_config.mcp.enabled || !app_config.mcp.cli_enabled {
        print_error(
            "cli_disabled",
            "ExaTerm CLI is disabled. Set mcp.enabled=true and mcp.cli_enabled=true.",
        );
        return 1;
    }

    let client = ControlClient::new();
    if let Err(error) = client.discover_or_start_gui().await {
        print_error("control_unavailable", &error);
        return 1;
    }

    match client.call_tool(call.tool_name, call.args).await {
        Ok(result) => {
            println!(
                "{}",
                serde_json::to_string(&result).unwrap_or_else(|_| "{}".into())
            );
            0
        }
        Err(error) => {
            let (code, exit_code) = classify_tool_error(error.code);
            print_error(code, error.message.as_ref());
            exit_code
        }
    }
}

fn build_call(command: RootCommand, stdin: &mut impl Read) -> Result<CliCall, String> {
    match command {
        RootCommand::Sessions(SessionsArgs {
            command: SessionsCommand::List,
        }) => Ok(call("list_terminal_sessions", json!({}))),
        RootCommand::Profiles(ProfilesArgs {
            command: ProfilesCommand::List(args),
        }) => Ok(call(
            "list_connection_profiles",
            json!({
                "connection_type": args.connection_type.map(ProfileType::as_str),
            }),
        )),
        RootCommand::Profiles(ProfilesArgs {
            command: ProfilesCommand::Connect(args),
        }) => {
            require_non_empty("--profile-id", &args.profile_id)?;
            validate_dimensions(args.cols, args.rows)?;
            Ok(call(
                "connect_saved_profile",
                json!({
                    "profile_id": args.profile_id,
                    "connection_type": args.connection_type.as_str(),
                    "cols": args.cols,
                    "rows": args.rows,
                }),
            ))
        }
        RootCommand::Serial(SerialArgs {
            command: SerialCommand::Ports,
        }) => Ok(call("list_serial_ports", json!({}))),
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
            Ok(call(
                "connect_serial_console",
                json!({
                    "port": args.port,
                    "baud_rate": args.baud_rate,
                    "data_bits": args.data_bits,
                    "parity": args.parity.map(value_name),
                    "stop_bits": args.stop_bits,
                    "flow_control": args.flow_control.map(value_name),
                    "terminal_mode": args.terminal_mode.map(TerminalMode::as_json_name),
                    "cols": args.cols,
                    "rows": args.rows,
                }),
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command: TerminalCommand::Output(args),
        }) => build_output_call(args),
        RootCommand::Terminal(TerminalArgs {
            command: TerminalCommand::Send(args),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            let data = read_value(args.data, stdin)?;
            validate_input_length(&data)?;
            Ok(call(
                "send_terminal_input",
                json!({
                    "session_id": args.session_id,
                    "data": data,
                }),
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
            Ok(call(
                "run_terminal_command",
                json!({
                    "session_id": args.session_id,
                    "command": command,
                    "append_newline": args.append_newline,
                    "wait_contains": args.wait_contains,
                    "timeout_ms": args.timeout_ms,
                    "settle_ms": args.settle_ms,
                    "max_chars": args.max_chars,
                }),
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command:
                TerminalCommand::Log(LogArgs {
                    command: LogCommand::Start(args),
                }),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            Ok(call(
                "start_terminal_log",
                json!({ "session_id": args.session_id }),
            ))
        }
        RootCommand::Terminal(TerminalArgs {
            command:
                TerminalCommand::Log(LogArgs {
                    command: LogCommand::Stop(args),
                }),
        }) => {
            require_non_empty("--session-id", &args.session_id)?;
            Ok(call(
                "stop_terminal_log",
                json!({ "session_id": args.session_id }),
            ))
        }
    }
}

fn build_output_call(args: OutputArgs) -> Result<CliCall, String> {
    require_non_empty("--session-id", &args.session_id)?;
    validate_optional_range("--max-chars", args.max_chars, 1, 20_000)?;
    let value = match args.mode {
        OutputMode::Recent => {
            if args.cursor.is_some() || args.contains.is_some() || args.timeout_ms.is_some() {
                return Err(
                    "recent mode does not accept --cursor, --contains, or --timeout-ms".into(),
                );
            }
            json!({
                "session_id": args.session_id,
                "mode": "recent",
                "max_chars": args.max_chars,
            })
        }
        OutputMode::Delta => {
            let cursor = args
                .cursor
                .ok_or_else(|| "delta mode requires --cursor".to_string())?;
            if args.contains.is_some() || args.timeout_ms.is_some() {
                return Err("delta mode does not accept --contains or --timeout-ms".into());
            }
            json!({
                "session_id": args.session_id,
                "mode": "delta",
                "cursor": cursor,
                "max_chars": args.max_chars,
            })
        }
        OutputMode::Wait => {
            validate_optional_range("--timeout-ms", args.timeout_ms, 1, 60_000)?;
            json!({
                "session_id": args.session_id,
                "mode": "wait",
                "cursor": args.cursor,
                "contains": args.contains,
                "timeout_ms": args.timeout_ms,
                "max_chars": args.max_chars,
            })
        }
    };

    Ok(call("read_terminal_output", value))
}

fn call(tool_name: &'static str, args: Value) -> CliCall {
    CliCall { tool_name, args }
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

fn value_name<T: ValueEnum>(value: T) -> String {
    value
        .to_possible_value()
        .expect("value enum variants have names")
        .get_name()
        .to_string()
}

fn print_error(code: &str, message: &str) {
    eprintln!(
        "{}",
        json!({ "error": { "code": code, "message": message } })
    );
}

fn classify_tool_error(code: ErrorCode) -> (&'static str, i32) {
    if code == ErrorCode::INVALID_PARAMS {
        ("invalid_arguments", 2)
    } else {
        ("tool_error", 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> RootCommand {
        Cli::try_parse_from(args).unwrap().command
    }

    #[test]
    fn profile_connect_includes_connection_type() {
        let call = build_call(
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
        assert_eq!(call.tool_name, "connect_saved_profile");
        assert_eq!(call.args["connection_type"], "telnet");
        assert_eq!(call.args["profile_id"], "router");
    }

    #[test]
    fn profile_list_without_type_requests_all_profiles() {
        let call = build_call(
            parse(&["exaterm-cli", "profiles", "list"]),
            &mut io::empty(),
        )
        .unwrap();
        assert_eq!(call.tool_name, "list_connection_profiles");
        assert_eq!(call.args["connection_type"], Value::Null);
    }

    #[test]
    fn profile_list_includes_connection_type() {
        let ssh_call = build_call(
            parse(&["exaterm-cli", "profiles", "list", "--type", "ssh"]),
            &mut io::empty(),
        )
        .unwrap();
        let telnet_call = build_call(
            parse(&["exaterm-cli", "profiles", "list", "--type", "telnet"]),
            &mut io::empty(),
        )
        .unwrap();

        assert_eq!(ssh_call.tool_name, "list_connection_profiles");
        assert_eq!(ssh_call.args["connection_type"], "ssh");
        assert_eq!(telnet_call.tool_name, "list_connection_profiles");
        assert_eq!(telnet_call.args["connection_type"], "telnet");
    }

    #[test]
    fn profile_list_rejects_unknown_connection_type() {
        let error = Cli::try_parse_from(["exaterm-cli", "profiles", "list", "--type", "serial"])
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn output_delta_requires_cursor() {
        let error = build_call(
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
        let error = build_call(
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
        let call = build_call(
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
        assert_eq!(call.args["data"], "show version\n");
    }

    #[test]
    fn serial_rejects_invalid_data_bits() {
        let error = build_call(
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
        let error = build_call(
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
    fn invalid_tool_parameters_use_argument_exit_code() {
        assert_eq!(
            classify_tool_error(ErrorCode::INVALID_PARAMS),
            ("invalid_arguments", 2)
        );
        assert_eq!(
            classify_tool_error(ErrorCode::INTERNAL_ERROR),
            ("tool_error", 1)
        );
    }
}
