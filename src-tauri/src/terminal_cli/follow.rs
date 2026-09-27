use std::future::Future;
use std::io::{self, Write};
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use tokio::time::{self, Instant};

use crate::external_control::client::ExternalControlClient;
use crate::external_control::service::{
    ExternalControlError, ExternalControlRequest, ExternalControlResponse, ReadTerminalOutputArgs,
    ReadTerminalOutputResult, TerminalOutputResult,
};
use crate::terminal_control::TerminalStatus;

use super::{require_non_empty, validate_optional_range, OutputArgs};

const DEFAULT_DURATION_MS: u64 = 30_000;
const MAX_DURATION_MS: u64 = 600_000;
const DEFAULT_TOTAL_CHARS: usize = 20_000;
const MAX_TOTAL_CHARS: usize = 200_000;
const DEFAULT_READ_CHARS: usize = 2_000;
const WAIT_SLICE_MS: u64 = 10_000;

pub(super) struct FollowOptions {
    session_id: String,
    cursor: Option<usize>,
    duration_ms: u64,
    max_total_chars: usize,
    max_chars: usize,
    until: Option<String>,
}

pub(super) fn build_options(args: OutputArgs) -> Result<FollowOptions, String> {
    require_non_empty("--session-id", &args.session_id)?;
    if args.timeout_ms.is_some() || args.contains.is_some() {
        return Err("follow mode does not accept --timeout-ms or --contains".into());
    }
    validate_optional_range("--duration-ms", args.duration_ms, 1, MAX_DURATION_MS)?;
    validate_optional_range(
        "--max-total-chars",
        args.max_total_chars,
        1,
        MAX_TOTAL_CHARS,
    )?;
    validate_optional_range("--max-chars", args.max_chars, 1, 20_000)?;
    if let Some(until) = &args.until {
        require_non_empty("--until", until)?;
        if until.chars().count() > 20_000 {
            return Err("--until must not exceed 20000 characters".into());
        }
    }
    Ok(FollowOptions {
        session_id: args.session_id,
        cursor: args.cursor,
        duration_ms: args.duration_ms.unwrap_or(DEFAULT_DURATION_MS),
        max_total_chars: args.max_total_chars.unwrap_or(DEFAULT_TOTAL_CHARS),
        max_chars: args.max_chars.unwrap_or(DEFAULT_READ_CHARS),
        until: args.until,
    })
}

#[derive(Debug)]
pub(super) enum FollowError {
    Control(ExternalControlError),
    Output(io::Error),
}

#[async_trait]
pub(super) trait FollowControl {
    async fn call(
        &self,
        request: ExternalControlRequest,
    ) -> Result<ExternalControlResponse, ExternalControlError>;
}

#[async_trait]
impl FollowControl for ExternalControlClient {
    async fn call(
        &self,
        request: ExternalControlRequest,
    ) -> Result<ExternalControlResponse, ExternalControlError> {
        ExternalControlClient::call(self, request).await
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum EndReason {
    Matched,
    DurationLimit,
    OutputLimit,
    Disconnected,
    Interrupted,
}

enum CallOutcome {
    Response(Result<ExternalControlResponse, ExternalControlError>),
    Stop(EndReason),
}

async fn guarded_call<C: FollowControl, F: Future<Output = ()>>(
    client: &C,
    request: ExternalControlRequest,
    deadline: Instant,
    interrupt: &mut Pin<Box<F>>,
) -> CallOutcome {
    tokio::select! {
        biased;
        _ = interrupt.as_mut() => CallOutcome::Stop(EndReason::Interrupted),
        _ = time::sleep_until(deadline) => CallOutcome::Stop(EndReason::DurationLimit),
        result = client.call(request) => CallOutcome::Response(result),
    }
}

struct FollowState {
    cursor: Option<usize>,
    emitted_chars: usize,
    match_tail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum OutputPhase {
    Initial,
    Live,
}

fn write_event(writer: &mut impl Write, event: &impl Serialize) -> Result<(), FollowError> {
    serde_json::to_writer(&mut *writer, event)
        .map_err(|error| FollowError::Output(io::Error::other(error)))?;
    writer.write_all(b"\n").map_err(FollowError::Output)?;
    writer.flush().map_err(FollowError::Output)
}

fn write_end(
    writer: &mut impl Write,
    state: &FollowState,
    reason: EndReason,
) -> Result<(), FollowError> {
    write_event(
        writer,
        &serde_json::json!({
            "type": "end",
            "cursor": state.cursor,
            "reason": reason,
        }),
    )
}

fn tail_chars(text: &str, count: usize) -> String {
    text.chars()
        .rev()
        .take(count)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn process_output(
    writer: &mut impl Write,
    options: &FollowOptions,
    state: &mut FollowState,
    snapshot: TerminalOutputResult,
    phase: OutputPhase,
) -> Result<Option<EndReason>, FollowError> {
    if let Some(requested_cursor) = state.cursor {
        if snapshot.start_cursor > requested_cursor {
            write_event(
                writer,
                &serde_json::json!({
                    "type": "gap",
                    "session_id": options.session_id,
                    "requested_cursor": requested_cursor,
                    "resumed_cursor": snapshot.start_cursor,
                }),
            )?;
            state.match_tail.clear();
        }
    }

    let marker_end = options.until.as_deref().and_then(|until| {
        let combined = format!("{}{}", state.match_tail, snapshot.output);
        combined.find(until).map(|start| {
            combined[..start + until.len()]
                .chars()
                .count()
                .saturating_sub(state.match_tail.chars().count())
        })
    });
    let remaining = options.max_total_chars - state.emitted_chars;
    let output_chars = snapshot.output.chars().count();
    let emitted_now = output_chars
        .min(remaining)
        .min(marker_end.unwrap_or(usize::MAX));
    let emitted: String = snapshot.output.chars().take(emitted_now).collect();
    state.cursor = Some(snapshot.start_cursor + emitted_now);

    if !emitted.is_empty() {
        write_event(
            writer,
            &serde_json::json!({
                "type": "output",
                "phase": phase,
                "session_id": options.session_id,
                "output": emitted,
                "start_cursor": snapshot.start_cursor,
                "cursor": state.cursor,
            }),
        )?;
        state.emitted_chars += emitted_now;
        if let Some(until) = &options.until {
            state.match_tail = tail_chars(
                &format!("{}{}", state.match_tail, emitted),
                until.chars().count().saturating_sub(1),
            );
        }
    }

    if marker_end.is_some_and(|end| end <= emitted_now) {
        return Ok(Some(EndReason::Matched));
    }
    if state.emitted_chars == options.max_total_chars {
        return Ok(Some(EndReason::OutputLimit));
    }
    Ok(None)
}

fn output_from_response(
    response: ExternalControlResponse,
) -> Result<TerminalOutputResult, FollowError> {
    match response {
        ExternalControlResponse::ReadTerminalOutput(
            ReadTerminalOutputResult::Recent(result) | ReadTerminalOutputResult::Delta(result),
        ) => Ok(result),
        ExternalControlResponse::ReadTerminalOutput(ReadTerminalOutputResult::Wait(result)) => {
            Ok(TerminalOutputResult {
                session_id: result.session_id,
                output: result.output,
                truncated: result.truncated,
                available_chars: result.available_chars,
                start_cursor: result.start_cursor,
                cursor: result.cursor,
            })
        }
        _ => Err(FollowError::Control(ExternalControlError::Internal(
            "Unexpected response to follow output request".into(),
        ))),
    }
}

fn session_disconnected(
    response: ExternalControlResponse,
    session_id: &str,
) -> Result<bool, FollowError> {
    match response {
        ExternalControlResponse::ListTerminalSessions(result) => {
            Ok(result.sessions.iter().any(|session| {
                session.session_id == session_id && session.status == TerminalStatus::Disconnected
            }))
        }
        _ => Err(FollowError::Control(ExternalControlError::Internal(
            "Unexpected response to follow session status request".into(),
        ))),
    }
}

pub(super) async fn run_follow<C: FollowControl, W: Write, F: Future<Output = ()>>(
    client: &C,
    writer: &mut W,
    options: FollowOptions,
    interrupt: F,
) -> Result<(), FollowError> {
    let deadline = Instant::now() + Duration::from_millis(options.duration_ms);
    let mut interrupt = Box::pin(interrupt);
    let mut state = FollowState {
        cursor: options.cursor,
        emitted_chars: 0,
        match_tail: String::new(),
    };
    let mut request = match options.cursor {
        Some(cursor) => ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Delta {
            session_id: options.session_id.clone(),
            cursor,
            max_chars: Some(options.max_chars),
        }),
        None => ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Recent {
            session_id: options.session_id.clone(),
            max_chars: Some(options.max_chars),
        }),
    };
    let mut initial = true;
    let mut finish_after_chunk = false;

    loop {
        let was_wait = matches!(
            request,
            ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Wait { .. })
        );
        let response = match guarded_call(client, request, deadline, &mut interrupt).await {
            CallOutcome::Stop(reason) => return write_end(writer, &state, reason),
            CallOutcome::Response(Ok(response)) => response,
            CallOutcome::Response(Err(error)) if was_wait => {
                match guarded_call(
                    client,
                    ExternalControlRequest::ListTerminalSessions,
                    deadline,
                    &mut interrupt,
                )
                .await
                {
                    CallOutcome::Stop(EndReason::Interrupted) => {
                        return write_end(writer, &state, EndReason::Interrupted);
                    }
                    CallOutcome::Stop(EndReason::DurationLimit) => {
                        return Err(FollowError::Control(error));
                    }
                    CallOutcome::Stop(_) => {
                        unreachable!("guarded calls only stop on time or interrupt")
                    }
                    CallOutcome::Response(Ok(response)) => {
                        if session_disconnected(response, &options.session_id)? {
                            request = delta_request(
                                &options,
                                state.cursor.expect("initial output established cursor"),
                            );
                            finish_after_chunk = true;
                            continue;
                        }
                        return Err(FollowError::Control(error));
                    }
                    _ => return Err(FollowError::Control(error)),
                }
            }
            CallOutcome::Response(Err(error)) => return Err(FollowError::Control(error)),
        };
        let snapshot = output_from_response(response)?;
        if let Some(reason) = process_output(
            writer,
            &options,
            &mut state,
            snapshot,
            if initial {
                OutputPhase::Initial
            } else {
                OutputPhase::Live
            },
        )? {
            return write_end(writer, &state, reason);
        }
        initial = false;
        if finish_after_chunk {
            return write_end(writer, &state, EndReason::Disconnected);
        }

        let disconnected = match guarded_call(
            client,
            ExternalControlRequest::ListTerminalSessions,
            deadline,
            &mut interrupt,
        )
        .await
        {
            CallOutcome::Stop(reason) => return write_end(writer, &state, reason),
            CallOutcome::Response(Ok(response)) => {
                session_disconnected(response, &options.session_id)?
            }
            CallOutcome::Response(Err(error)) => return Err(FollowError::Control(error)),
        };
        if disconnected {
            request = delta_request(
                &options,
                state.cursor.expect("initial output established cursor"),
            );
            finish_after_chunk = true;
        } else {
            let remaining_ms = deadline
                .saturating_duration_since(Instant::now())
                .as_millis();
            if remaining_ms == 0 {
                return write_end(writer, &state, EndReason::DurationLimit);
            }
            request = ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Wait {
                session_id: options.session_id.clone(),
                cursor: state.cursor,
                contains: None,
                timeout_ms: Some(remaining_ms.min(u128::from(WAIT_SLICE_MS)) as u64),
                max_chars: Some(options.max_chars),
            });
        }
    }
}

fn delta_request(options: &FollowOptions, cursor: usize) -> ExternalControlRequest {
    ExternalControlRequest::ReadTerminalOutput(ReadTerminalOutputArgs::Delta {
        session_id: options.session_id.clone(),
        cursor,
        max_chars: Some(options.max_chars),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use clap::Parser;
    use serde_json::Value;

    use super::*;
    use crate::external_control::service::ListTerminalSessionsResult;
    use crate::terminal_control::{TerminalProtocol, TerminalSessionInfo};

    enum Reply {
        Result(Result<ExternalControlResponse, ExternalControlError>),
        Pending,
    }

    struct FakeControl {
        replies: Mutex<VecDeque<Reply>>,
        requests: Mutex<Vec<ExternalControlRequest>>,
    }

    impl FakeControl {
        fn new(replies: Vec<Reply>) -> Self {
            Self {
                replies: Mutex::new(replies.into()),
                requests: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl FollowControl for FakeControl {
        async fn call(
            &self,
            request: ExternalControlRequest,
        ) -> Result<ExternalControlResponse, ExternalControlError> {
            self.requests.lock().unwrap().push(request);
            let reply = self.replies.lock().unwrap().pop_front().expect("reply");
            match reply {
                Reply::Result(result) => result,
                Reply::Pending => std::future::pending().await,
            }
        }
    }

    fn options() -> FollowOptions {
        FollowOptions {
            session_id: "s1".into(),
            cursor: None,
            duration_ms: 30_000,
            max_total_chars: 20_000,
            max_chars: 2_000,
            until: None,
        }
    }

    fn output(text: &str, start_cursor: usize) -> TerminalOutputResult {
        TerminalOutputResult {
            session_id: "s1".into(),
            output: text.into(),
            truncated: false,
            available_chars: text.chars().count(),
            start_cursor,
            cursor: start_cursor + text.chars().count(),
        }
    }

    fn output_reply(text: &str, start_cursor: usize, initial: bool) -> Reply {
        let result = if initial {
            ReadTerminalOutputResult::Recent(output(text, start_cursor))
        } else {
            ReadTerminalOutputResult::Delta(output(text, start_cursor))
        };
        Reply::Result(Ok(ExternalControlResponse::ReadTerminalOutput(result)))
    }

    fn sessions_reply(status: TerminalStatus) -> Reply {
        Reply::Result(Ok(ExternalControlResponse::ListTerminalSessions(
            ListTerminalSessionsResult {
                sessions: vec![TerminalSessionInfo {
                    session_id: "s1".into(),
                    protocol: TerminalProtocol::Ssh,
                    target: "host".into(),
                    encoding: "utf-8".into(),
                    status,
                }],
            },
        )))
    }

    fn events(output: &[u8]) -> Vec<Value> {
        std::str::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn follow_options_validate_bounds_and_incompatible_flags() {
        let args = |extra: &[&str]| {
            let mut values = vec![
                "exaterm-cli",
                "terminal",
                "output",
                "--session-id",
                "s1",
                "--mode",
                "follow",
            ];
            values.extend_from_slice(extra);
            let cli = crate::terminal_cli::Cli::try_parse_from(values).unwrap();
            let crate::terminal_cli::RootCommand::Terminal(crate::terminal_cli::TerminalArgs {
                command: crate::terminal_cli::TerminalCommand::Output(args),
            }) = cli.command
            else {
                panic!("output command")
            };
            build_options(args)
        };
        let defaults = args(&[]).unwrap();
        assert_eq!(defaults.duration_ms, 30_000);
        assert_eq!(defaults.max_total_chars, 20_000);
        assert_eq!(defaults.max_chars, 2_000);
        for extra in [
            vec!["--duration-ms", "0"],
            vec!["--duration-ms", "600001"],
            vec!["--max-total-chars", "0"],
            vec!["--max-total-chars", "200001"],
            vec!["--timeout-ms", "1000"],
            vec!["--contains", "prompt"],
            vec!["--until", ""],
        ] {
            assert!(args(&extra).is_err(), "{extra:?}");
        }
    }

    #[test]
    fn output_limit_keeps_resume_cursor_at_last_emitted_character() {
        let mut options = options();
        options.max_total_chars = 3;
        let mut state = FollowState {
            cursor: Some(5),
            emitted_chars: 0,
            match_tail: String::new(),
        };
        let mut writer = Vec::new();
        let reason = process_output(
            &mut writer,
            &options,
            &mut state,
            output("abcd", 5),
            OutputPhase::Live,
        )
        .unwrap();
        assert!(matches!(reason, Some(EndReason::OutputLimit)));
        assert_eq!(state.cursor, Some(8));
        let events = events(&writer);
        assert_eq!(events[0]["output"], "abc");
        assert_eq!(events[0]["cursor"], 8);
    }

    #[test]
    fn marker_matches_across_chunks_without_consuming_later_output() {
        let mut options = options();
        options.until = Some("router#".into());
        let mut state = FollowState {
            cursor: Some(0),
            emitted_chars: 0,
            match_tail: String::new(),
        };
        let mut writer = Vec::new();
        assert!(process_output(
            &mut writer,
            &options,
            &mut state,
            output("rou", 0),
            OutputPhase::Initial
        )
        .unwrap()
        .is_none());
        let reason = process_output(
            &mut writer,
            &options,
            &mut state,
            output("ter#more", 3),
            OutputPhase::Live,
        )
        .unwrap();
        assert!(matches!(reason, Some(EndReason::Matched)));
        assert_eq!(state.cursor, Some(7));
        assert_eq!(events(&writer)[1]["output"], "ter#");
    }

    #[test]
    fn gap_event_precedes_output_and_reports_skipped_cursor_range() {
        let options = options();
        let mut state = FollowState {
            cursor: Some(2),
            emitted_chars: 0,
            match_tail: String::new(),
        };
        let mut writer = Vec::new();
        process_output(
            &mut writer,
            &options,
            &mut state,
            output("new", 8),
            OutputPhase::Live,
        )
        .unwrap();
        let events = events(&writer);
        assert_eq!(events[0]["type"], "gap");
        assert_eq!(events[0]["requested_cursor"], 2);
        assert_eq!(events[0]["resumed_cursor"], 8);
        assert_eq!(events[1]["cursor"], 11);
    }

    #[test]
    fn gap_does_not_complete_marker_across_missing_output() {
        let mut options = options();
        options.until = Some("router#".into());
        let mut state = FollowState {
            cursor: Some(3),
            emitted_chars: 3,
            match_tail: "rou".into(),
        };
        let mut writer = Vec::new();
        let reason = process_output(
            &mut writer,
            &options,
            &mut state,
            output("ter#", 8),
            OutputPhase::Live,
        )
        .unwrap();
        assert!(reason.is_none());
        assert_eq!(events(&writer)[0]["type"], "gap");
    }

    #[tokio::test]
    async fn wait_timeout_continues_until_later_output_matches() {
        let fake = FakeControl::new(vec![
            output_reply("", 0, true),
            sessions_reply(TerminalStatus::Connected),
            Reply::Result(Ok(ExternalControlResponse::ReadTerminalOutput(
                ReadTerminalOutputResult::Wait(
                    crate::external_control::service::WaitTerminalOutputResult {
                        session_id: "s1".into(),
                        matched: false,
                        timed_out: true,
                        output: String::new(),
                        truncated: false,
                        available_chars: 0,
                        start_cursor: 0,
                        cursor: 0,
                    },
                ),
            ))),
            sessions_reply(TerminalStatus::Connected),
            Reply::Result(Ok(ExternalControlResponse::ReadTerminalOutput(
                ReadTerminalOutputResult::Wait(
                    crate::external_control::service::WaitTerminalOutputResult {
                        session_id: "s1".into(),
                        matched: true,
                        timed_out: false,
                        output: "done".into(),
                        truncated: false,
                        available_chars: 4,
                        start_cursor: 0,
                        cursor: 4,
                    },
                ),
            ))),
        ]);
        let mut options = options();
        options.until = Some("done".into());
        let mut writer = Vec::new();
        run_follow(&fake, &mut writer, options, std::future::pending())
            .await
            .unwrap();
        let events = events(&writer);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["output"], "done");
        assert_eq!(events[1]["reason"], "matched");
    }

    #[tokio::test]
    async fn disconnected_session_drains_final_delta_and_ends() {
        let fake = FakeControl::new(vec![
            output_reply("old", 0, true),
            sessions_reply(TerminalStatus::Disconnected),
            output_reply("last", 3, false),
        ]);
        let mut writer = Vec::new();
        run_follow(&fake, &mut writer, options(), std::future::pending())
            .await
            .unwrap();
        let events = events(&writer);
        assert_eq!(events[0]["phase"], "initial");
        assert_eq!(events[1]["phase"], "live");
        assert_eq!(events[2]["reason"], "disconnected");
        assert_eq!(events[2]["cursor"], 7);
    }

    #[tokio::test]
    async fn interrupt_ends_with_last_cursor() {
        let fake = FakeControl::new(vec![
            output_reply("old", 0, true),
            sessions_reply(TerminalStatus::Connected),
            Reply::Pending,
        ]);
        let mut writer = Vec::new();
        run_follow(
            &fake,
            &mut writer,
            options(),
            time::sleep(Duration::from_millis(5)),
        )
        .await
        .unwrap();
        let events = events(&writer);
        assert_eq!(events.last().unwrap()["reason"], "interrupted");
        assert_eq!(events.last().unwrap()["cursor"], 3);
    }

    #[tokio::test]
    async fn duration_limit_ends_even_with_no_new_output() {
        let fake = FakeControl::new(vec![
            output_reply("", 0, true),
            sessions_reply(TerminalStatus::Connected),
            Reply::Pending,
        ]);
        let mut options = options();
        options.duration_ms = 5;
        let mut writer = Vec::new();
        run_follow(&fake, &mut writer, options, std::future::pending())
            .await
            .unwrap();
        let events = events(&writer);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["reason"], "duration_limit");
        assert_eq!(events[0]["cursor"], 0);
    }

    #[tokio::test]
    async fn control_failure_preserves_last_output_cursor_without_end_event() {
        let fake = FakeControl::new(vec![
            output_reply("old", 0, true),
            sessions_reply(TerminalStatus::Connected),
            Reply::Result(Err(ExternalControlError::Internal("offline".into()))),
            sessions_reply(TerminalStatus::Connected),
        ]);
        let mut writer = Vec::new();
        let result = run_follow(&fake, &mut writer, options(), std::future::pending()).await;
        assert!(matches!(result, Err(FollowError::Control(_))));
        let events = events(&writer);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["cursor"], 3);
    }

    #[tokio::test]
    async fn control_failure_is_not_reported_as_success_when_status_check_times_out() {
        let fake = FakeControl::new(vec![
            output_reply("old", 0, true),
            sessions_reply(TerminalStatus::Connected),
            Reply::Result(Err(ExternalControlError::Internal("offline".into()))),
            Reply::Pending,
        ]);
        let mut options = options();
        options.duration_ms = 5;
        let mut writer = Vec::new();
        let result = run_follow(&fake, &mut writer, options, std::future::pending()).await;
        assert!(matches!(
            result,
            Err(FollowError::Control(ExternalControlError::Internal(_)))
        ));
        assert_eq!(events(&writer).len(), 1);
    }

    #[tokio::test]
    async fn writer_failure_stops_follow() {
        struct BrokenWriter;
        impl Write for BrokenWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let fake = FakeControl::new(vec![output_reply("data", 0, true)]);
        let result = run_follow(&fake, &mut BrokenWriter, options(), std::future::pending()).await;
        assert!(matches!(result, Err(FollowError::Output(_))));
    }
}
