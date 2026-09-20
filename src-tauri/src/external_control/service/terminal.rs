use std::time::Duration;

use tokio::time;
use uuid::Uuid;

use crate::terminal_control::{TerminalControlState, TerminalStatus};

use super::connections::terminal_protocol_log_type;
use super::{
    internal_error, invalid_params, not_found, unavailable, ExternalControlError,
    ExternalControlLogControlRequestPayload, ExternalControlRuntime, ExternalControlService,
    ListTerminalSessionsResult, ReadTerminalOutputArgs, ReadTerminalOutputResult,
    RunTerminalCommandArgs, RunTerminalCommandResult, SendTerminalInputArgs,
    SendTerminalInputResult, StartTerminalLogArgs, StartTerminalLogResult, StopTerminalLogArgs,
    StopTerminalLogResult, TerminalOutputResult, WaitTerminalOutputResult, DEFAULT_READ_CHARS,
    DEFAULT_SETTLE_MS, DEFAULT_WAIT_TIMEOUT_MS, MAX_INPUT_CHARS, MAX_READ_CHARS, MAX_SETTLE_MS,
    MAX_WAIT_TIMEOUT_MS,
};

impl ExternalControlService {
    pub(crate) async fn list_terminal_sessions(
        &self,
    ) -> Result<ListTerminalSessionsResult, ExternalControlError> {
        let sessions = self.runtime.terminals.list_sessions().await;
        Ok(ListTerminalSessionsResult { sessions })
    }

    pub(crate) async fn read_terminal_output(
        &self,
        args: ReadTerminalOutputArgs,
    ) -> Result<ReadTerminalOutputResult, ExternalControlError> {
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

                Ok(ReadTerminalOutputResult::Recent(
                    TerminalOutputResult::from(snapshot),
                ))
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

                Ok(ReadTerminalOutputResult::Delta(TerminalOutputResult::from(
                    snapshot,
                )))
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
                let result = wait_for_terminal_output(
                    &self.runtime.terminals,
                    &session_id,
                    start_cursor,
                    contains.as_deref(),
                    normalize_max_chars(max_chars),
                    normalize_timeout_ms(timeout_ms),
                )
                .await?;
                Ok(ReadTerminalOutputResult::Wait(result))
            }
        }
    }

    pub(crate) async fn send_terminal_input(
        &self,
        args: SendTerminalInputArgs,
    ) -> Result<SendTerminalInputResult, ExternalControlError> {
        send_terminal_input_to_runtime(&self.runtime, &args.session_id, args.data).await?;

        Ok(SendTerminalInputResult {
            session_id: args.session_id,
            sent: true,
        })
    }

    pub(crate) async fn start_terminal_log(
        &self,
        args: StartTerminalLogArgs,
    ) -> Result<StartTerminalLogResult, ExternalControlError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| not_found("Session not found"))?;

        if info.status != TerminalStatus::Connected {
            return Err(unavailable("The session is already disconnected"));
        }

        if !self.runtime.io.logger.is_available() {
            return Err(internal_error(
                "Logger state required to start external control logging is unavailable",
            ));
        }
        let already_active = self
            .runtime
            .io
            .logger
            .active_log_file(&args.session_id)
            .await
            .is_some();

        let file_path = request_manual_log_start(&self.runtime, &info)
            .await
            .map_err(internal_error)?;

        Ok(StartTerminalLogResult {
            session_id: args.session_id,
            started: !already_active,
            already_active,
            file_path,
            log_mode: "manual".into(),
        })
    }

    pub(crate) async fn stop_terminal_log(
        &self,
        args: StopTerminalLogArgs,
    ) -> Result<StopTerminalLogResult, ExternalControlError> {
        let info = self
            .runtime
            .terminals
            .session_info(&args.session_id)
            .await
            .ok_or_else(|| not_found("Session not found"))?;

        if !self.runtime.io.logger.is_available() {
            return Err(internal_error(
                "Logger state required to stop external control logging is unavailable",
            ));
        }
        let already_inactive = self
            .runtime
            .io
            .logger
            .active_log_file(&args.session_id)
            .await
            .is_none();

        request_manual_log_stop(&self.runtime, &info)
            .await
            .map_err(internal_error)?;

        Ok(StopTerminalLogResult {
            session_id: args.session_id,
            stopped: !already_inactive,
            already_inactive,
        })
    }

    pub(crate) async fn run_terminal_command(
        &self,
        args: RunTerminalCommandArgs,
    ) -> Result<RunTerminalCommandResult, ExternalControlError> {
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
        if !wait_result.timed_out && settle_ms > 0 {
            time::sleep(Duration::from_millis(settle_ms)).await;
        }

        let snapshot = self
            .runtime
            .terminals
            .read_output_delta(&args.session_id, start_cursor, max_chars)
            .await
            .map_err(invalid_params)?;

        Ok(RunTerminalCommandResult {
            session_id: args.session_id,
            sent: true,
            matched: wait_result.matched,
            timed_out: wait_result.timed_out,
            output: snapshot.output,
            truncated: snapshot.truncated,
            available_chars: snapshot.available_chars,
            start_cursor: snapshot.start_cursor,
            cursor: snapshot.cursor,
        })
    }
}

async fn request_manual_log_start(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<String, String> {
    let owner_window_id = runtime
        .workspace
        .owner_window_id_for_session(&info.session_id)
        .await
        .ok_or_else(|| "The session does not have an owner window".to_string())?;
    let ack = runtime
        .io
        .ui
        .request_log_control(
            &owner_window_id,
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

async fn request_manual_log_stop(
    runtime: &ExternalControlRuntime,
    info: &crate::terminal_control::TerminalSessionInfo,
) -> Result<(), String> {
    let owner_window_id = runtime
        .workspace
        .owner_window_id_for_session(&info.session_id)
        .await
        .ok_or_else(|| "The session does not have an owner window".to_string())?;
    runtime
        .io
        .ui
        .request_log_control(
            &owner_window_id,
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

    runtime
        .io
        .protocol
        .write_terminal(info.protocol, session_id, data)
        .await
        .map_err(internal_error)
}

async fn wait_for_terminal_output(
    terminals: &TerminalControlState,
    session_id: &str,
    start_cursor: usize,
    contains: Option<&str>,
    max_chars: usize,
    timeout_ms: u64,
) -> Result<WaitTerminalOutputResult, ExternalControlError> {
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
            return Ok(WaitTerminalOutputResult {
                session_id: snapshot.session_id,
                matched: true,
                timed_out: false,
                output: snapshot.output,
                truncated: snapshot.truncated,
                available_chars: snapshot.available_chars,
                start_cursor: snapshot.start_cursor,
                cursor: snapshot.cursor,
            });
        }

        let now = time::Instant::now();
        if now >= deadline {
            return Ok(WaitTerminalOutputResult {
                session_id: snapshot.session_id,
                matched: false,
                timed_out: true,
                output: snapshot.output,
                truncated: snapshot.truncated,
                available_chars: snapshot.available_chars,
                start_cursor: snapshot.start_cursor,
                cursor: snapshot.cursor,
            });
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
