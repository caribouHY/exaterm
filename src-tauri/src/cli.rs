use serde::Serialize;
use std::ffi::OsString;
use std::io::{self, Write};

const DEFAULT_SSH_PORT: u16 = 22;
const USAGE: &str = "\
Usage:
  exaterm.exe help
  exaterm.exe ssh [-p <port>] <target>
  exaterm.exe telnet [-p <port>] <target>

Targets:
  ssh user@hostname    Connect to an SSH host
  ssh profile-name     Connect using a saved SSH profile
  telnet hostname      Connect to a Telnet host
  telnet profile-name  Connect using a saved Telnet profile
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    RunApp(Option<StartupCliRequest>),
    PrintHelp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StartupCliRequest {
    Ssh(StartupSshRequest),
    Telnet(StartupTelnetRequest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartupSshRequest {
    pub target_kind: StartupSshTargetKind,
    pub host: Option<String>,
    pub username: Option<String>,
    pub profile_name: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartupTelnetRequest {
    pub target: String,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupSshTargetKind {
    Direct,
    Profile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn parse_env_args() -> Result<CliAction, CliError> {
    parse_args(std::env::args_os().skip(1))
}

fn parse_args<I>(args: I) -> Result<CliAction, CliError>
where
    I: IntoIterator<Item = OsString>,
{
    let args = args
        .into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|_| CliError::new("Arguments must be valid Unicode."))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if args.is_empty() {
        return Ok(CliAction::RunApp(None));
    }

    match args[0].as_str() {
        "help" | "--help" | "-h" => {
            if args.len() == 1 {
                Ok(CliAction::PrintHelp)
            } else {
                Err(CliError::new(
                    "The help command does not accept extra arguments.",
                ))
            }
        }
        "ssh" => parse_ssh_args(&args[1..]),
        "telnet" => parse_telnet_args(&args[1..]),
        command => Err(CliError::new(format!("Unknown command: {command}"))),
    }
}

fn parse_ssh_args(args: &[String]) -> Result<CliAction, CliError> {
    let (target, port) = parse_target_args(args, "SSH")?;

    let request = if let Some((username, host)) = target.split_once('@') {
        let username = username.trim();
        let host = host.trim();
        if username.is_empty() || host.is_empty() {
            return Err(CliError::new(
                "SSH direct targets must use the user@hostname format.",
            ));
        }

        StartupSshRequest {
            target_kind: StartupSshTargetKind::Direct,
            host: Some(host.to_string()),
            username: Some(username.to_string()),
            profile_name: None,
            port: Some(port.unwrap_or(DEFAULT_SSH_PORT)),
        }
    } else {
        StartupSshRequest {
            target_kind: StartupSshTargetKind::Profile,
            host: None,
            username: None,
            profile_name: Some(target.to_string()),
            port,
        }
    };

    Ok(CliAction::RunApp(Some(StartupCliRequest::Ssh(request))))
}

fn parse_telnet_args(args: &[String]) -> Result<CliAction, CliError> {
    let (target, port) = parse_target_args(args, "Telnet")?;

    Ok(CliAction::RunApp(Some(StartupCliRequest::Telnet(
        StartupTelnetRequest {
            target: target.to_string(),
            port,
        },
    ))))
}

fn parse_target_args<'a>(
    args: &'a [String],
    command_label: &str,
) -> Result<(&'a str, Option<u16>), CliError> {
    if args.is_empty() {
        return Err(CliError::new(format!("Missing {command_label} target.")));
    }

    let mut target: Option<&str> = None;
    let mut port: Option<u16> = None;
    let mut index = 0;

    while index < args.len() {
        let arg = args[index].as_str();
        if arg == "-p" {
            if port.is_some() {
                return Err(CliError::new(format!(
                    "{command_label} port was specified more than once."
                )));
            }
            let raw_port = args
                .get(index + 1)
                .ok_or_else(|| CliError::new(format!("Missing {command_label} port after -p.")))?;
            port = Some(parse_port(raw_port, command_label)?);
            index += 2;
            continue;
        }

        if target.is_some() {
            return Err(CliError::new(format!("Invalid {command_label} arguments.")));
        }
        target = Some(arg);
        index += 1;
    }

    let target = target
        .ok_or_else(|| CliError::new(format!("Missing {command_label} target.")))?
        .trim();
    if target.is_empty() {
        return Err(CliError::new(format!("Missing {command_label} target.")));
    }

    Ok((target, port))
}

fn parse_port(raw: &str, command_label: &str) -> Result<u16, CliError> {
    let port = raw.parse::<u16>().map_err(|_| {
        CliError::new(format!(
            "{command_label} port must be a number from 1 to 65535."
        ))
    })?;
    if port == 0 {
        return Err(CliError::new(format!(
            "{command_label} port must be a number from 1 to 65535."
        )));
    }
    Ok(port)
}

pub fn print_help() {
    write_to_console(USAGE, false);
}

pub fn print_error(error: &CliError) {
    write_to_console(&format!("Error: {}\n\n{USAGE}", error.message()), true);
}

#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() -> bool {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }

    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) != 0 }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn attach_parent_console() -> bool {
    false
}

#[cfg(all(windows, not(debug_assertions)))]
fn detach_console(attached: bool) {
    if !attached {
        return;
    }

    extern "system" {
        fn FreeConsole() -> i32;
    }

    unsafe {
        FreeConsole();
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn detach_console(_attached: bool) {}

fn write_to_console(message: &str, stderr: bool) {
    let attached = attach_parent_console();

    #[cfg(all(windows, not(debug_assertions)))]
    {
        let device = if stderr { "CONERR$" } else { "CONOUT$" };
        if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(device) {
            let _ = file.write_all(message.as_bytes());
            let _ = file.flush();
            detach_console(attached);
            return;
        }
    }

    if stderr {
        let mut stderr = io::stderr();
        let _ = stderr.write_all(message.as_bytes());
        let _ = stderr.flush();
    } else {
        let mut stdout = io::stdout();
        let _ = stdout.write_all(message.as_bytes());
        let _ = stdout.flush();
    }

    detach_console(attached);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<CliAction, CliError> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn help_is_help_action() {
        assert_eq!(parse(&["help"]).unwrap(), CliAction::PrintHelp);
    }

    #[test]
    fn direct_ssh_defaults_to_port_22() {
        assert_eq!(
            parse(&["ssh", "user@example.com"]).unwrap(),
            CliAction::RunApp(Some(StartupCliRequest::Ssh(StartupSshRequest {
                target_kind: StartupSshTargetKind::Direct,
                host: Some("example.com".into()),
                username: Some("user".into()),
                profile_name: None,
                port: Some(22),
            })))
        );
    }

    #[test]
    fn direct_ssh_accepts_port() {
        assert_eq!(
            parse(&["ssh", "user@example.com", "-p", "2222"]).unwrap(),
            CliAction::RunApp(Some(StartupCliRequest::Ssh(StartupSshRequest {
                target_kind: StartupSshTargetKind::Direct,
                host: Some("example.com".into()),
                username: Some("user".into()),
                profile_name: None,
                port: Some(2222),
            })))
        );
    }

    #[test]
    fn direct_ssh_accepts_port_before_target() {
        assert_eq!(
            parse(&["ssh", "-p", "2222", "user@example.com"]).unwrap(),
            CliAction::RunApp(Some(StartupCliRequest::Ssh(StartupSshRequest {
                target_kind: StartupSshTargetKind::Direct,
                host: Some("example.com".into()),
                username: Some("user".into()),
                profile_name: None,
                port: Some(2222),
            })))
        );
    }

    #[test]
    fn profile_ssh_uses_profile_target() {
        assert_eq!(
            parse(&["ssh", "office-router"]).unwrap(),
            CliAction::RunApp(Some(StartupCliRequest::Ssh(StartupSshRequest {
                target_kind: StartupSshTargetKind::Profile,
                host: None,
                username: None,
                profile_name: Some("office-router".into()),
                port: None,
            })))
        );
    }

    #[test]
    fn missing_port_is_error() {
        assert!(parse(&["ssh", "user@example.com", "-p"]).is_err());
    }

    #[test]
    fn invalid_port_is_error() {
        assert!(parse(&["ssh", "user@example.com", "-p", "abc"]).is_err());
    }

    #[test]
    fn out_of_range_port_is_error() {
        assert!(parse(&["ssh", "user@example.com", "-p", "70000"]).is_err());
    }

    #[test]
    fn zero_port_is_error() {
        assert!(parse(&["ssh", "user@example.com", "-p", "0"]).is_err());
    }

    #[test]
    fn unknown_command_is_error() {
        assert!(parse(&["ftp", "example.com"]).is_err());
    }

    #[test]
    fn extra_arguments_are_error() {
        assert!(parse(&["ssh", "user@example.com", "-p", "22", "extra"]).is_err());
    }

    #[test]
    fn duplicate_port_is_error() {
        assert!(parse(&["ssh", "-p", "22", "user@example.com", "-p", "2222"]).is_err());
    }

    #[test]
    fn telnet_accepts_target_without_port() {
        assert_eq!(
            parse(&["telnet", "example.com"]).unwrap(),
            CliAction::RunApp(Some(StartupCliRequest::Telnet(StartupTelnetRequest {
                target: "example.com".into(),
                port: None,
            })))
        );
    }

    #[test]
    fn telnet_accepts_port_before_target() {
        assert_eq!(
            parse(&["telnet", "-p", "2323", "example.com"]).unwrap(),
            CliAction::RunApp(Some(StartupCliRequest::Telnet(StartupTelnetRequest {
                target: "example.com".into(),
                port: Some(2323),
            })))
        );
    }

    #[test]
    fn telnet_accepts_port_after_target() {
        assert_eq!(
            parse(&["telnet", "example.com", "-p", "2323"]).unwrap(),
            CliAction::RunApp(Some(StartupCliRequest::Telnet(StartupTelnetRequest {
                target: "example.com".into(),
                port: Some(2323),
            })))
        );
    }

    #[test]
    fn telnet_missing_port_is_error() {
        assert!(parse(&["telnet", "example.com", "-p"]).is_err());
    }

    #[test]
    fn telnet_invalid_port_is_error() {
        assert!(parse(&["telnet", "-p", "abc", "example.com"]).is_err());
    }

    #[test]
    fn telnet_out_of_range_port_is_error() {
        assert!(parse(&["telnet", "example.com", "-p", "70000"]).is_err());
    }

    #[test]
    fn telnet_duplicate_port_is_error() {
        assert!(parse(&["telnet", "-p", "23", "example.com", "-p", "2323"]).is_err());
    }

    #[test]
    fn telnet_extra_target_is_error() {
        assert!(parse(&["telnet", "example.com", "extra"]).is_err());
    }
}
