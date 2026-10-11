# Lifecycle Review Cases

Use only sections affected by the change. Paths are repository-relative starting points; locate current callers and tests if code moves. Confirm current behavior in the selected architecture sections and source before treating a suspected race as a defect. These cases guide investigation and regression selection, not a requirement to run every scenario for every change.

## Connection Attempts and Finalization

Start with `src/components/Connection/connectionAttemptController.ts`, `src/components/Connection/connectionAttemptState.ts`, `src/components/Connection/connectionAttemptServices.ts`, and `src-tauri/src/connect_attempt.rs`; follow the affected protocol adapter.

- Trace request and credential-prompt identities through preparation, submission, cancellation, and callbacks. Check duplicate starts/submissions and results from an older attempt after a new attempt begins.
- Compare both orderings of backend completion and cancellation. Inspect `ConnectAttempt::begin_completion` and frontend finalization together; a cancellation response arriving last must not undo an accepted success.
- Dispose during preparation, connect, logging, and pending workspace registration where affected. Check who owns a late successful session, including registration that succeeds after the dialog closes. Release an unregistered resource without disconnecting one already transferred to the workspace.
- Fail logging, registration, or history independently. Verify that recoverable registration uses the same session, logging is not started twice, and auxiliary failures do not discard a successful connection.
- Keep pre-connection credential prompts distinct from SSH handshake/host-key prompts. Inspect cleanup without exposing secret values in snapshots, fixtures, or reports.

## Tab/Window Ownership and Teardown

Start with `src-tauri/src/workspace/`, `src/features/workspace-tabs/`, and the affected protocol owner (`src-tauri/src/ssh/`, `src-tauri/src/serial.rs`, or `src-tauri/src/telnet.rs`).

- Follow the backend session ID, workspace tab ID, and owner window separately through move, detach, close, and disconnect. Check the authoritative workspace revision against delayed events from the former owner.
- Distinguish closing a window with another destination available from closing the last window. Compare the requested behavior and existing close policy; do not impose session preservation on intentional application shutdown.
- For disconnect changes, check already-disconnected and unknown-session cases, pending I/O completion, logger shutdown, and retained tab/output behavior for that entry point. Do not equate disconnect with explicit tab removal.
- For Serial, trace worker/handle release before reporting port availability. Static inspection or mocked teardown does not prove that a real device can reconnect.

## Output Restoration and Disposal

Start with `src/components/Terminal/terminalOutputSyncController.ts`, `src/components/Terminal/TerminalView.tsx`, and `src-tauri/src/terminal_control.rs`.

- Interleave snapshot/delta responses and live events, especially events arriving during the final recovery response. Include overlapping cursor ranges and equal text at different cursors; compare Unicode code points rather than JavaScript code units when slicing retained output.
- Separate backend retention loss from local scheduling limits. A bounded drain must yield without silently dropping pending events; a retention gap cannot be reported as a complete transcript.
- Check failed or non-advancing recovery, single in-flight recovery, continuation scheduling, and a later event triggering retry. Obtain current limits from source rather than hardcoding them in this procedure.
- Dispose during subscription, recovery, or a scheduled write, then remount in another window. Old callbacks must not affect the new view or release its backend session; late subscription handles and pending completion promises still need cleanup.
- Separate visibility changes from teardown. Switching to a utility view must not reset the session or erase its buffer.

## Logging State and Flush Ordering

Start with `src/features/terminal-logging/manualLogController.ts`, `src/features/terminal-logging/manualLogBufferWriter.ts`, `src/features/terminal-logging/externalLogControl.ts`, and `src-tauri/src/logger.rs`.

- Treat backend logger state as authoritative when metadata is stale, especially after a metadata update fails or a tab moves. Use session identity for logger calls and tab identity for workspace updates.
- Interleave output arrival with a flush, including the interval after the drain loop exits but before its promise settles. Verify flush completion covers queued writes and that failed writes retain ordering for retry.
- Check flush-before-pause/stop, repeated operations, same-session overlap, and independent sessions. A metadata retry must not repeat a completed backend start/stop or overwrite a log again.
- Revalidate a save dialog's fixed session/tab target when it resolves; switching active tabs must not redirect logging. Follow target ownership and the relevant acknowledgement if movement occurs during an external operation.
- When log deletion changes, inspect active file identity and associated history rows before allowing deletion. Use synthetic fixtures; do not read real logs to establish lifecycle behavior.

## Evidence and Findings

For a suspected race, describe the initial state, ordered events or suspension points, resulting state, and broken invariant. Tie the sequence to current code and, where available, a deterministic test. Distinguish a supported defect from a hypothesis needing runtime evidence. Missing GUI/device validation is a reported limitation, not by itself a code defect.
