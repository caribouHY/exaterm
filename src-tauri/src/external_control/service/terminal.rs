use std::time::Duration;

use serde_json::{json, Value};
use tokio::time;
#[cfg(not(test))]
use uuid::Uuid;

use crate::logger::{self};
use crate::serial;
use crate::ssh;
use crate::telnet;
use crate::terminal_control::{TerminalControlState, TerminalProtocol, TerminalStatus};

use super::connections::terminal_protocol_log_type;
#[cfg(not(test))]
use super::ExternalControlLogControlRequestPayload;
use super::{
    internal_error, invalid_params, not_found, unavailable, ExternalControlError,
    ExternalControlRuntime, ExternalControlService, ReadTerminalOutputArgs, RunTerminalCommandArgs,
    SendTerminalInputArgs, StartTerminalLogArgs, StopTerminalLogArgs, DEFAULT_READ_CHARS,
    DEFAULT_SETTLE_MS, DEFAULT_WAIT_TIMEOUT_MS, MAX_INPUT_CHARS, MAX_READ_CHARS, MAX_SETTLE_MS,
    MAX_WAIT_TIMEOUT_MS,
};

impl ExternalControlService {
    pub(crate) async fn list_terminal_sessions(&self) -> Result<Value, ExternalControlError> {
        let sessions = self.runtime.terminals.list_sessions().await;
        Ok(json!({
            "sessions": sessions,
        }))
    }

    pub(crate) async fn read_terminal_output(
        &self,
        args: ReadTerminalOutputArgs,
    ) -> Result<Value, ExternalControlError> {
        match args {
            ReadTerminalOutputArgs::Recent {
                session_id,
                max_chars,
            } => {
                let snapshot = self
                    .runtime
                    .terminals
                    .read_output(&session_id, normalize_max_chars(max_chars))
                    .await
                    .map_err(invalid_params)?;

                Ok(json!({
                    "session_id": snapshot.session_id,
                    "mode": "recent",
                    "output": snapshot.output,
                    "truncated": snapshot.truncated,
                    "available_chars": snapshot.available_chars,
                    "start_cursor": snapshot.start_cursor,
                    "cursor": snapshot.cursor,
                }))
            }
            ReadTerminalOutputArgs::Delta {
                session_id,
                cursor,
                max_chars,
            } => {
                let snapshot = self
                    .runtime
                    .terminals
                    .read_output_delta(&session_id, cursor, normalize_max_chars(max_chars))
                    .await
                    .map_err(invalid_params)?;

                Ok(json!({
                    "session_id": snapshot.session_id,
                    "mode": "delta",
                    "output": snapshot.output,
                    "truncated": snapshot.truncated,
                    "available_chars": snapshot.available_chars,
                    "start_cursor": snapshot.start_cursor,
                    "cursor": snapshot.cursor,
                }))
            }
            ReadTerminalOutputArgs::Wait {
                session_id,
                cursor,
                contains,
                timeout_ms,
                max_chars,
            } => {
                let start_cursor = match cursor {
                    Some(cursor) => cursor,
                    None => self
                        .runtime
                        .terminals
                        .cursor(&session_id)
                        .await
                        .map_err(invalid_params)?,
                };
                let contains = contains.filter(|value| !value.is_empty());
                let mut result = wait_for_terminal_output(
                    &self.runtime.terminals,
                    &session_id,
                    start_cursor,
                    contains.as_deref(),
                    normalize_max_chars(max_chars),
                    normalize_timeout_ms(timeout_ms),
                )
                .await?;
                result["mode"] = json!("wait");
                Ok(result)
            }
        }
    }

    pub(crate) async fn send_terminal_input(
        &self,
        args: SendTerminalInputArgs,
    ) -> Result<Value, ExternalControlError> {
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, args.data).await?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
        }))
    }

    pub(crate) async fn start_terminal_log(
        &self,
        args: StartTerminalLogArgs,
    ) -> Result<Value, ExternalControlError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| not_found("Session not found"))?;

        if info.status != TerminalStatus::Connected {
            return Err(unavailable("The session is already disconnected"));
        }

        let logger_state = self.runtime.logger.as_ref().ok_or_else(|| {
            internal_error("Logger state required to start external control logging is unavailable")
        })?;
        if let Some(session) = logger::manual_log_session(logger_state, &args.session_id).await {
            return Ok(json!({
                "session_id": args.session_id,
                "started": false,
                "already_active": true,
                "file_path": session.file_path,
                "log_mode": "manual",
            }));
        }

        let file_path = request_manual_log_start(&self.runtime, &info)
            .await
            .map_err(internal_error)?;

        Ok(json!({
            "session_id": args.session_id,
            "started": true,
            "already_active": false,
            "file_path": file_path,
            "log_mode": "manual",
        }))
    }

    pub(crate) async fn stop_terminal_log(
        &self,
        args: StopTerminalLogArgs,
    ) -> Result<Value, ExternalControlError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| not_found("Session not found"))?;

        let logger_state = self.runtime.logger.as_ref().ok_or_else(|| {
            internal_error("Logger state required to stop external control logging is unavailable")
        })?;
        if logger::manual_log_session(logger_state, &args.session_id)
            .await
            .is_none()
        {
            return Ok(json!({
                "session_id": args.session_id,
                "stopped": false,
                "already_inactive": true,
            }));
        }

        request_manual_log_stop(&self.runtime, &info)
            .await
            .map_err(internal_error)?;

        Ok(json!({
            "session_id": args.session_id,
            "stopped": true,
            "already_inactive": false,
        }))
    }

    pub(crate) async fn run_terminal_command(
        &self,
        args: RunTerminalCommandArgs,
    ) -> Result<Value, ExternalControlError> {
        if args.command.trim().is_empty() {
            return Err(invalid_params("The command to send must not be empty"));
        }
        if args.command.chars().count() > MAX_INPUT_CHARS {
            return Err(invalid_params(format!(
                "Commands must be no longer than {} characters",
                MAX_INPUT_CHARS
            )));
        }

        let start_cursor = self
            .runtime
            .terminals
            .cursor(&args.session_id)
            .await
            .map_err(invalid_params)?;
        let data = if args.append_newline.unwrap_or(true) {
            format!("{}\n", args.command)
        } else {
            args.command.clone()
        };
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, data).await?;

        let max_chars = normalize_max_chars(args.max_chars);
        let wait_result = wait_for_terminal_output(
            &self.runtime.terminals,
            &args.session_id,
            start_cursor,
            args.wait_contains
                .as_deref()
                .filter(|value| !value.is_empty()),
            max_chars,
            normalize_timeout_ms(args.timeout_ms),
        )
        .await?;

        let settle_ms = args
            .settle_ms
            .unwrap_or(DEFAULT_SETTLE_MS)
            .clamp(0, MAX_SETTLE_MS);
        if wait_result["timed_out"] == false && settle_ms > 0 {
            time::sleep(Duration::from_millis(settle_ms)).await;
        }

        let snapshot = self
            .runtime
            .terminals
            .read_output_delta(&args.session_id, start_cursor, max_chars)
            .await
            .map_err(invalid_params)?;

        Ok(json!({
            "session_id": args.session_id,
            "sent": true,
            "matched": wait_result["matched"],
            "timed_out": wait_result["timed_out"],
            "output": snapshot.output,
            "truncated": snapshot.truncated,
            "available_chars": snapshot.available_chars,
            "start_cursor": snapshot.start_cursor,
            "cursor": snapshot.cursor,
        }))
    }
}

#[cfg(not(test))]
async fn request_manual_log_start(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<String, String> {
    let app = runtime.app.as_ref().ok_or_else(|| {
        "App handle required to start external control logging is unavailable".to_string()
    })?;
    let log_control = runtime.log_control.as_ref().ok_or_else(|| {
        "Log control state required to start external control logging is unavailable".to_string()
    })?;
    let ack = log_control
        .request(
            app,
            "external-control://log-start-request",
            ExternalControlLogControlRequestPayload {
                request_id: Uuid::new_v4().to_string(),
                session_id: info.session_id.clone(),
                connection_type: terminal_protocol_log_type(info.protocol).into(),
                target: info.target.clone(),
            },
        )
        .await?;
    ack.file_path.ok_or_else(|| {
        "The external control log start response did not include a log file path".to_string()
    })
}

#[cfg(test)]
async fn request_manual_log_start(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<String, String> {
    let logger_state = runtime.logger.as_ref().ok_or_else(|| {
        "Logger state required to start external control logging is unavailable".to_string()
    })?;
    logger::start_manual_log(
        logger_state,
        info.session_id.clone(),
        terminal_protocol_log_type(info.protocol).into(),
        info.target.clone(),
        None,
        None,
    )
    .await
}

#[cfg(not(test))]
async fn request_manual_log_stop(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<(), String> {
    let app = runtime.app.as_ref().ok_or_else(|| {
        "App handle required to stop external control logging is unavailable".to_string()
    })?;
    let log_control = runtime.log_control.as_ref().ok_or_else(|| {
        "Log control state required to stop external control logging is unavailable".to_string()
    })?;
    log_control
        .request(
            app,
            "external-control://log-stop-request",
            ExternalControlLogControlRequestPayload {
                request_id: Uuid::new_v4().to_string(),
                session_id: info.session_id.clone(),
                connection_type: terminal_protocol_log_type(info.protocol).into(),
                target: info.target.clone(),
            },
        )
        .await
        .map(|_| ())
}

#[cfg(test)]
async fn request_manual_log_stop(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<(), String> {
    let logger_state = runtime.logger.as_ref().ok_or_else(|| {
        "Logger state required to stop external control logging is unavailable".to_string()
    })?;
    logger::stop_manual_log(logger_state, &info.session_id).await
}

async fn send_terminal_input_to_runtime(
    runtime: &ExternalControlRuntime,
    session_id: &str,
    data: String,
) -> Result<(), ExternalControlError> {
    if data.chars().count() > MAX_INPUT_CHARS {
        return Err(invalid_params(format!(
            "Input must be no longer than {} characters",
            MAX_INPUT_CHARS
        )));
    }

    let info = runtime
        .terminals
        .session_info(session_id)
        .await
        .ok_or_else(|| not_found("Session not found"))?;

    if info.status != TerminalStatus::Connected {
        return Err(unavailable("The session is already disconnected"));
    }

    match info.protocol {
        TerminalProtocol::Ssh => ssh::write_data(&runtime.ssh, session_id, data).await,
        TerminalProtocol::Serial => serial::write_data(&runtime.serial, session_id, data).await,
        TerminalProtocol::Telnet => telnet::write_data(&runtime.telnet, session_id, data).await,
    }
    .map_err(internal_error)
}

async fn wait_for_terminal_output(
    terminals: &TerminalControlState,
    session_id: &str,
    start_cursor: usize,
    contains: Option<&str>,
    max_chars: usize,
    timeout_ms: u64,
) -> Result<Value, ExternalControlError> {
    let deadline = time::Instant::now() + Duration::from_millis(timeout_ms);

    loop {
        let output_changed = terminals.output_change_notified();
        tokio::pin!(output_changed);

        let snapshot = terminals
            .read_output_delta(session_id, start_cursor, max_chars)
            .await
            .map_err(invalid_params)?;
        let matched = match contains {
            Some(needle) => snapshot.output.contains(needle),
            None => !snapshot.output.is_empty(),
        };

        if matched {
            return Ok(json!({
                "session_id": snapshot.session_id,
                "matched": true,
                "timed_out": false,
                "output": snapshot.output,
                "truncated": snapshot.truncated,
                "available_chars": snapshot.available_chars,
                "start_cursor": snapshot.start_cursor,
                "cursor": snapshot.cursor,
            }));
        }

        let now = time::Instant::now();
        if now >= deadline {
            return Ok(json!({
                "session_id": snapshot.session_id,
                "matched": false,
                "timed_out": true,
                "output": snapshot.output,
                "truncated": snapshot.truncated,
                "available_chars": snapshot.available_chars,
                "start_cursor": snapshot.start_cursor,
                "cursor": snapshot.cursor,
            }));
        }

        let remaining = deadline - now;
        let _ = time::timeout(remaining, &mut output_changed).await;
    }
}

pub(crate) fn normalize_max_chars(max_chars: Option<usize>) -> usize {
    max_chars
        .unwrap_or(DEFAULT_READ_CHARS)
        .clamp(1, MAX_READ_CHARS)
}

pub(crate) fn normalize_timeout_ms(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(DEFAULT_WAIT_TIMEOUT_MS)
        .clamp(1, MAX_WAIT_TIMEOUT_MS)
}
