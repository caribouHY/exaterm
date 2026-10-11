# ExaTerm CLI Workflows

Use only the section needed for the task. JSON examples use `Invoke-ExaTermJson` from
[checked PowerShell calls](cli-reference.md#checked-powershell-json-calls).

## Connection Readiness

A successful `profiles connect`, direct `ssh connect`/`telnet connect`, or `serial connect`
result means that ExaTerm created a session. It does not guarantee that the initial banner,
login exchange, or normal prompt has finished rendering.

SSH success also requires server acceptance of both PTY allocation and shell startup.
Each request has a 10-second deadline covering send and reply. Rejection, channel closure,
or no reply fails the connection without creating a terminal tab. Inspect the connection
error and server policy before retrying; output alone does not prove startup acceptance.

After connecting:

1. Retain the returned `session_id`.
2. Run `sessions list` and confirm that the matching session has status `connected`.
3. Read recent output and retain its cursor:

   ```powershell
   $initial = Invoke-ExaTermJson terminal output --session-id $sessionId `
     --mode recent --max-chars 2000
   $cursor = $initial.cursor
   ```

4. Inspect whether the output shows a normal prompt, a login prompt, an incomplete banner,
   or no output.
5. If an exact normal prompt is observed, retain it for later operations:

   ```powershell
   $verifiedPrompt = "the exact prompt observed in this session"
   ```

   Do not assign this variable from generic characters such as `#`, `$`, or `>`.

6. If readiness is unclear, wait from the retained cursor without guessing a prompt:

   ```powershell
   $ready = Invoke-ExaTermJson terminal output --session-id $sessionId `
     --mode wait --cursor $cursor --timeout-ms 30000 `
     --max-chars 2000
   ```

MUST NOT send the requested command until a normal prompt or another explicit readiness
marker has been observed. A successful connection result alone does not establish readiness.

Use `--contains $verifiedPrompt` only after the exact prompt has been observed in the current
session. If it remains unknown, omit `--contains` and inspect the output returned by each
wait. Some serial consoles remain silent until input is sent. Sending an empty line changes
the remote interaction, so follow the host agent's normal approval policy before doing so.

Update the cursor after each wait, including timeouts, and inspect every result. Use a finite
overall readiness deadline appropriate to the task. If a silent console needs an authorized
prompt probe, send one newline with ``terminal send --data "`n"``; do not send the substantive
command to discover whether the session is ready. If readiness remains unknown, report it.

## Command Completion and Continuation

`terminal run` captures output after sending input. Without `--wait-contains`, its wait
succeeds on the first nonempty output, which may be command echo; the default additional
250 ms is a fixed delay, not a quiet-period detector. `timed_out=false` and `matched=true`
therefore do not by themselves establish completion. This applies to short commands too.

Use an exact prompt observed in this session or a command-specific completion marker when
available. Choose evidence that cannot be confused with echoed input or unrelated output.
A returned prompt establishes that interaction finished, but inspect output for errors
before claiming command success. If no marker is known, omit the contains option, inspect
each chunk, and continue observation until terminal evidence demonstrates completion.

For a verified marker, this example sends once and continues until the marker is confirmed,
regardless of whether an individual interval timed out. Set the overall budget to the task's deadline;
the example uses two minutes and keeps each call at most 30 seconds.

```powershell
$completionMarker = $verifiedPrompt
$overallTimeoutMs = 120000
$deadline = [DateTime]::UtcNow.AddMilliseconds($overallTimeoutMs)

$result = Invoke-ExaTermJson terminal run --session-id $sessionId `
  --command "long-running-command" --wait-contains $completionMarker `
  --timeout-ms ([Math]::Min(30000, $overallTimeoutMs)) --max-chars 2000
$cursor = $result.cursor
$incomplete = $result.truncated
$completed = $result.matched -or $result.output.Contains($completionMarker)

while (-not $completed) {
  $remainingMs = [Math]::Ceiling(($deadline - [DateTime]::UtcNow).TotalMilliseconds)
  if ($remainingMs -le 0) { break }
  $result = Invoke-ExaTermJson terminal output --session-id $sessionId `
    --mode wait --cursor $cursor --contains $completionMarker `
    --timeout-ms ([Math]::Min(30000, $remainingMs)) --max-chars 2000
  $cursor = $result.cursor
  $incomplete = $incomplete -or $result.truncated
  $completed = $result.matched -or $result.output.Contains($completionMarker)
}
```

- Inspect each result's output before replacing it when the task needs command results;
  the example tracks completion and does not collect a transcript.
- Use this loop only with a nonempty verified marker. Without one, inspect each result and
  stop on explicit completion evidence; do not use `matched` as a completion predicate.
- Check returned output too: `run` takes a final snapshot after waiting, so a completion
  marker can arrive after its wait timed out. Waiting only on `matched` can miss that marker.
- Always update the cursor, including on timeout. Never resend the command just because
  an interval timed out. If a send/run call fails after input may have reached the device,
  inspect the session instead of assuming the command was not accepted.
- Stop on disconnect or a CLI error. Re-list the session before deciding recovery.
- If the deadline expires without completion evidence, report completion as unconfirmed.
  The remote command may still be running; observation timeout does not cancel it.
- `truncated=true` means text was omitted. A later successful match does not make the
  transcript complete. See [output size guidance](cli-reference.md#output-size-guidance)
  for retained-buffer recovery limits. Use bounded `follow` when incremental capture and
  cross-chunk marker matching are needed.

### Multiline and Stdin Input

The CLI reads stdin verbatim and `terminal run` adds another newline by default. PowerShell
native pipelines supply CRLF on Windows, which some terminals interpret as two submissions.
Use the checked helper's `-InputText` to send UTF-8 with explicit LF line endings, and
`--append-newline false` when the input already supplies its final newline:

```powershell
$commands = @'
show interfaces
show ip route
'@
$commands = $commands.Replace("`r`n", "`n") + "`n"
$result = Invoke-ExaTermJson -InputText $commands `
  terminal run --session-id $sessionId --command - --append-newline false
```

This uses a literal here-string so `$`, backticks, and other command text are not expanded
by PowerShell. Supply only authorized commands suitable for sequential input on the target;
inspect output for pagers or interactive questions and apply the same completion rules.
For multiple commands, the first returned prompt does not confirm the entire sequence.
Verify every requested command's result and final prompt, or run commands separately when
their completion cannot be distinguished reliably.
For `terminal send`, supply the intended LF explicitly through `-InputText`; it does not add
a newline. The helper checks exit codes and captures raw stderr before parsing JSON.

## Session Logging

Start logging only when explicitly requested. Logs are plaintext and may contain commands,
prompts, output, hostnames, usernames, and accidental secrets. Check status first:

```powershell
$status = Invoke-ExaTermJson terminal log status --session-id $sessionId
```

Status returns `state` (`inactive`, `active`, or `paused`), `file_path`, and `log_mode`
(`auto` or `manual`). Inactive path and mode are `null`. Select the action before calling it:

| Current state         | Action within the requested logging scope                                                                                      |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `inactive`            | Start a manual log when requested.                                                                                             |
| `active`, `manual`    | Preserve it; do not call start again as routine setup.                                                                         |
| `active`, `auto`      | Preserve it if existing recording satisfies the request. Switching to manual mode requires an authorized lifecycle change.     |
| `paused`, either mode | Preserve the pause unless the request authorizes recording to resume; then use `terminal log resume` and verify active status. |

Pause, resume, and stop operate on the current log, including automatic logs. Pause and
resume are idempotent through `changed`. Do not infer authorization to replace a destination
or switch modes from a request merely to run a command.

When starting a new manual log, omit destination options unless the user requested a path.
The default creates a unique file in ExaTerm's log directory using overwrite mode. To choose
a destination, pass `--file-path` and `--write-mode overwrite|append` together. Relative paths
resolve against the CLI process's working directory. A different destination is rejected
while a log exists; do not stop that log just to retry.

For the inactive branch, after connection readiness is established:

```powershell
$log = Invoke-ExaTermJson terminal log start --session-id $sessionId
$status = Invoke-ExaTermJson terminal log status --session-id $sessionId
if ($status.state -ne 'active' -or $status.log_mode -ne 'manual') {
  throw 'The requested manual log is not active'
}

$beforePrompt = Invoke-ExaTermJson terminal output --session-id $sessionId `
  --mode recent --max-chars 2000
$cursor = $beforePrompt.cursor
Invoke-ExaTermJson terminal send --session-id $sessionId --data "`n" | Out-Null
$prompt = Invoke-ExaTermJson terminal output --session-id $sessionId `
  --mode wait --cursor $cursor --contains $verifiedPrompt `
  --timeout-ms 30000 --max-chars 2000
```

Manual logging does not copy output already in the terminal buffer. Capture the cursor
before sending one newline so a quickly redrawn prompt is not missed. Use `--contains` only
with the exact verified prompt; otherwise omit it and inspect the returned output.
MUST NOT run the substantive command until a fresh normal prompt is observed. Continue
bounded observation if needed; a marker-free successful wait may contain only newline echo.
If the device does not redraw a prompt within the readiness budget, report that the log's
initial prompt and command readiness remain unconfirmed, and do not proceed with the command.
