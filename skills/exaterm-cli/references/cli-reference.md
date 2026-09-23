# ExaTerm CLI Reference

## Requirements

`exaterm-cli.exe` is distributed with supported Windows builds of ExaTerm and is normally
installed beside `exaterm.exe` and `exaterm-mcp.exe`. Add that directory to the current
PowerShell `PATH` or invoke the executable by its full path.

Enable the shared external-control service and CLI access in ExaTerm Settings, or configure:

```json
{
  "external_control": {
    "enabled": true,
    "cli_enabled": true,
    "mcp_enabled": false,
    "connect_enabled": false,
    "direct_connect_enabled": false
  }
}
```

- `external_control.enabled` is the master permission for CLI and MCP compatibility access.
- `external_control.cli_enabled` permits CLI operations and is the recommended primary path.
- `external_control.connect_enabled` additionally permits saved-profile and serial connections.
- `external_control.direct_connect_enabled` additionally permits direct SSH/Telnet connections to explicitly specified hosts.
- `external_control.mcp_enabled` affects `exaterm-mcp`, not `exaterm-cli`.

Restart ExaTerm after changing these settings. Individual saved profiles must also allow
external-control access before the CLI can list or connect them.

## Commands

```text
exaterm-cli doctor
exaterm-cli sessions list
exaterm-cli sessions disconnect --session-id <id>
exaterm-cli profiles list [--type <ssh|telnet>]
exaterm-cli profiles connect --type <ssh|telnet> --profile-id <id> [--cols <n>] [--rows <n>]
exaterm-cli ssh connect --host <host> --username <user> [options]
exaterm-cli telnet connect --host <host> [options]
exaterm-cli serial ports
exaterm-cli serial connect --port <name> [options]
exaterm-cli terminal output --session-id <id> --mode <recent|delta|wait> [options]
exaterm-cli terminal send --session-id <id> --data <text|->
exaterm-cli terminal run --session-id <id> --command <text|-> [options]
exaterm-cli terminal log start --session-id <id> [--file-path <path> --write-mode <overwrite|append>]
exaterm-cli terminal log stop --session-id <id>
exaterm-cli terminal log status --session-id <id>
exaterm-cli terminal log pause --session-id <id>
exaterm-cli terminal log resume --session-id <id>
```

Use `exaterm-cli <command> --help` for the syntax supported by the installed version.
`--help` and `--version` produce human-readable text, so do not pipe them to
`ConvertFrom-Json` or any other JSON parser.

## Diagnosing CLI Availability

Use `doctor` to diagnose CLI readiness without reading terminal content or changing settings:

```powershell
$diagnosis = exaterm-cli doctor | ConvertFrom-Json
$doctorExitCode = $LASTEXITCODE
```

It checks these stable IDs in order:

- `config`: the configuration can be loaded and validated.
- `external_control`: `external_control.enabled` is enabled.
- `cli_permission`: `external_control.cli_enabled` is enabled.
- `gui_executable`: the GUI executable can be found near the CLI.
- `control_plane`: the existing or newly started local control plane is reachable.
- `protocol`: the nonce handshake and protocol version are compatible.

The report contains `ok`, `version`, `protocol_version`, `gui_started`, and `checks`. Each
check has `id`, `status` (`pass`, `fail`, or `skipped`), `message`, and an optional
`remediation`. Absolute paths, configuration values, sessions, and credentials are omitted.
Inspect fields and check IDs rather than depending on property order or message wording.

If the GUI is not running, `doctor` may start the normal visible GUI and wait up to 30 seconds.
It continues independent GUI and control-plane checks when configuration loading fails. A
protocol mismatch is reported without repeatedly starting the GUI. Do not close or restart a
running GUI merely to make the diagnostic pass because that can interrupt active sessions.

Exit code `0` means every check passed. Exit code `1` means at least one check failed or was
skipped, but the complete report is still written to stdout and should be parsed. Evaluate the
individual checks: a lone `gui_executable` failure with a reachable, compatible control plane
does not invalidate the current GUI, although automatic startup is not ready. Do not apply a
remediation automatically when it changes configuration or restarts the GUI.

## Sessions and Profiles

`sessions list` returns JSON containing a `sessions` array. Use a returned `session_id` for
terminal operations.

`profiles list` returns a `profiles` array containing approved SSH and Telnet profiles.
Secrets and private-key paths are not returned. SSH and Telnet profiles may share an ID, so
always retain both the returned profile ID and connection type. `--type` accepts only `ssh`
or `telnet`.

`profiles connect` requires an exact returned ID and type. `--cols` and `--rows` each accept
values from 1 through 1000. Profile connections require
`external_control.connect_enabled=true`.

SSH passwords and encrypted private-key passphrases are entered through the visible ExaTerm
UI and must never be supplied as CLI arguments.

## Disconnecting a Session

Disconnect only an exact `session_id` returned by `sessions list`:

```powershell
$disconnect = exaterm-cli sessions disconnect --session-id $sessionId |
  ConvertFrom-Json
```

The result contains `session_id`, `disconnected`, and `already_disconnected`. A first successful
disconnect normally returns `disconnected=true` and `already_disconnected=false`. Repeating the
operation for the same known session is safe and returns `disconnected=false` with
`already_disconnected=true`. An unknown session returns a not-found error; re-list sessions
instead of guessing another ID.

Disconnect preserves the GUI tab and retained scrollback. ExaTerm flushes and stops an active
session log before ending SSH, Telnet, or Serial I/O. For Serial, a successful response is sent
only after the read, write, and receive-FIFO workers have stopped and the local port handle has
been released, so the same port can be opened again.

Disconnecting is a material connectivity change. Use it only when the user requested it or an
authorized workflow explicitly requires cleanup of a temporary session created for that task.
Do not use it as routine recovery for timeouts, ambiguous prompts, or transient errors.

For an authorized temporary automation session, preserve its exact returned ID and clean it up
without selecting another session:

```powershell
$connection = exaterm-cli serial connect --port $port | ConvertFrom-Json
try {
  # Perform only the authorized terminal operations.
} finally {
  exaterm-cli sessions disconnect --session-id $connection.session_id
}
```

## Direct SSH and Telnet Connections

Direct connections require `external_control.connect_enabled=true` and
`external_control.direct_connect_enabled=true`. Use only a host name or IP address explicitly
provided by the user. Never infer a host, SSH user name, port, authentication method,
private-key path, or jump profile.

```powershell
exaterm-cli ssh connect --host $host --username $username
exaterm-cli telnet connect --host $host
```

SSH supports `--port` (default `22`), `--auth-method`, `--private-key-path`,
`--jump-profile-id`, `--encoding`, `--terminal-mode`, `--cols`, and `--rows`. Telnet supports
`--port` (default `23`), `--encoding`, `--terminal-mode`, `--cols`, and `--rows`. Pass the host,
port, and SSH user name separately; do not use URI, `user@host`, embedded-port, path, bracketed
IPv6, or whitespace-containing syntax. A jump profile must be an externally enabled saved SSH
profile and cannot use another jump profile.

Unknown SSH host keys are confirmed in the visible ExaTerm UI. Host-key mismatches are
rejected. Passwords and passphrases stay in the GUI and must not be passed through CLI
arguments, environment variables, terminal input, or chat.

## Connection Readiness

A successful `profiles connect`, direct `ssh connect`/`telnet connect`, or `serial connect`
result means that ExaTerm created a session. It does not guarantee that the initial banner,
login exchange, or normal prompt has finished rendering.

After connecting:

1. Retain the returned `session_id`.
2. Run `sessions list` and confirm that the matching session has status `connected`.
3. Read recent output and retain its cursor:

   ```powershell
   $initial = exaterm-cli terminal output --session-id $sessionId `
     --mode recent --max-chars 2000 | ConvertFrom-Json
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
   $ready = exaterm-cli terminal output --session-id $sessionId `
     --mode wait --cursor $cursor --timeout-ms 30000 `
     --max-chars 2000 | ConvertFrom-Json
   ```

MUST NOT send the requested command until a normal prompt or another explicit readiness
marker has been observed. A successful connection result alone does not establish readiness.

Use `--contains $verifiedPrompt` only after the exact prompt has been observed in the current
session. If it remains unknown, omit `--contains` and inspect the output returned by each
wait. Some serial consoles remain silent until input is sent. Sending an empty line changes
the remote interaction, so follow the host agent's normal approval policy before doing so.

## Serial Connections

`serial ports` returns a `ports` array. Pass an exact returned port name to `serial connect`.
Serial connections require `external_control.connect_enabled=true`.

| Option             | Default     | Allowed values                 |
| ------------------ | ----------- | ------------------------------ |
| `--baud-rate`      | `9600`      | Positive integer               |
| `--data-bits`      | `8`         | `5`, `6`, `7`, `8`             |
| `--parity`         | `none`      | `none`, `odd`, `even`          |
| `--stop-bits`      | `1`         | `1`, `2`                       |
| `--flow-control`   | `none`      | `none`, `software`, `hardware` |
| `--terminal-mode`  | `general`   | `general`, `cisco-ios`         |
| `--cols`, `--rows` | `120`, `30` | 1 through 1000                 |

## Reading Output

The default and maximum output lengths are 2,000 and 20,000 characters.

### Output Size Guidance

- Use `--max-chars 2000` for ordinary inspection.
- Increase the limit incrementally, such as to 4,000, only when relevant output is missing.
- Use 20,000 only when the task requires a long result and the host agent has enough
  remaining context to process it.
- Prefer `delta` or `wait` with a retained cursor over repeatedly returning a large recent
  buffer.
- When `truncated=true`, narrow the requested output or continue from an appropriate cursor.
- Summarize large terminal results instead of copying them wholesale into the response.

Read the most recent retained output:

```powershell
$result = exaterm-cli terminal output --session-id $sessionId `
  --mode recent --max-chars 2000 | ConvertFrom-Json
```

Continue from a cursor returned by an earlier output or run result:

```powershell
$result = exaterm-cli terminal output --session-id $sessionId `
  --mode delta --cursor $cursor | ConvertFrom-Json
```

Wait for new output or a substring:

```powershell
$result = exaterm-cli terminal output --session-id $sessionId `
  --mode wait --cursor $cursor --contains $verifiedPrompt `
  --timeout-ms 30000 | ConvertFrom-Json
```

If no exact prompt or command-specific marker has been verified, omit `--contains`:

```powershell
$result = exaterm-cli terminal output --session-id $sessionId `
  --mode wait --cursor $cursor --timeout-ms 30000 | ConvertFrom-Json
```

- `recent` rejects `--cursor`, `--contains`, and `--timeout-ms`.
- `delta` requires `--cursor` and rejects `--contains` and `--timeout-ms`.
- `wait` starts at the current output position when `--cursor` is omitted.
- `--timeout-ms` accepts 1 through 60,000 milliseconds.
- A wait result includes a `timed_out` flag. Retain its returned cursor even on timeout.

Output results include the session ID, captured output, and cursor information. Additional
metadata can vary with operation and ExaTerm version; parse fields by name rather than
depending on property order.

## Sending Input and Running Commands

`terminal send` writes input without waiting for a result. Use it for interactive input such
as confirmation responses or control sequences that cannot be expressed as a normal command.
Before using it, verify the target session and the expected interaction state shown in recent
output. The valid state may be a normal prompt, login prompt, or confirmation question.

```powershell
"show version`n" |
  exaterm-cli terminal send --session-id $sessionId --data -
```

`terminal run` sends a command and returns captured output:

```powershell
$result = exaterm-cli terminal run --session-id $sessionId `
  --command "show version" --wait-contains $verifiedPrompt `
  --timeout-ms 30000 --max-chars 2000 | ConvertFrom-Json
```

Use `--wait-contains` only when `$verifiedPrompt` or a command-specific completion marker has
been established from terminal evidence. Otherwise omit it and inspect the returned output.

- Input is limited to 20,000 characters.
- Passing `-` to `--data` or `--command` reads the value from stdin.
- `terminal run` appends a newline by default.
- Use `--append-newline false` when the input must not end in a newline.
- `--timeout-ms` accepts 1 through 60,000 milliseconds.
- `--settle-ms` accepts 0 through 5,000 milliseconds.
- `--max-chars` accepts 1 through 20,000 characters.
- A timeout may still return useful partial output and a cursor.

## Long-running Commands

Each `terminal run` or `terminal output --mode wait` call can wait for at most 60 seconds.
For a command that may take longer, send it once and continue observing from the returned
cursor.

```powershell
$completionMarker = $verifiedPrompt

$result = exaterm-cli terminal run --session-id $sessionId `
  --command "long-running-command" --wait-contains $completionMarker `
  --timeout-ms 60000 --max-chars 20000 | ConvertFrom-Json

$cursor = $result.cursor
$deadline = (Get-Date).AddMinutes(10)

while ($result.timed_out -and (Get-Date) -lt $deadline) {
  $result = exaterm-cli terminal output --session-id $sessionId `
    --mode wait --cursor $cursor --contains $completionMarker `
    --timeout-ms 60000 --max-chars 20000 | ConvertFrom-Json
  $cursor = $result.cursor
}
```

- Set `$completionMarker` only to the exact verified prompt or a command-specific completion
  marker. If neither is known, omit the contains arguments and inspect each result.
- MUST NOT resend the command after a wait timeout. Continue from the returned cursor.
- Update the cursor after every result, including timeouts.
- Treat the expected prompt or another command-specific completion marker as completion.
- If the overall deadline expires, report that the command remains unconfirmed rather than
  reporting failure or success.
- If `truncated=true`, report that the returned output is incomplete. Continuing from the
  latest cursor observes future output but does not recover already truncated content.
- Re-run `sessions list` if waiting fails because the session may have disconnected.

## Session Logging

```powershell
$status = exaterm-cli terminal log status --session-id $sessionId | ConvertFrom-Json
exaterm-cli terminal log start --session-id $sessionId
exaterm-cli terminal log pause --session-id $sessionId
exaterm-cli terminal log resume --session-id $sessionId
exaterm-cli terminal log stop --session-id $sessionId
```

Logs are plaintext and can contain commands, prompts, output, hostnames, usernames, and
accidental secrets. Start them only when the user explicitly requests logging. Status reports
`inactive`, `active`, or `paused`, plus the active file path and `auto` or `manual` mode; inactive
path and mode fields are `null`. Pause and resume also apply to automatically started logs and
are idempotent through the `changed` result.

Omitting destination options creates a unique log file in ExaTerm's log directory using
overwrite mode. To choose a file, supply both options. Relative paths are resolved from the CLI
process's current directory. A different destination is rejected while a log is already active.

```powershell
exaterm-cli terminal log start --session-id $sessionId `
  --file-path .\logs\session.log --write-mode append
```

Manual logging does not copy output already retained in the terminal buffer. To include a
prompt at the beginning of a newly started log, request a fresh prompt before executing the
substantive command:

```powershell
$log = exaterm-cli terminal log start --session-id $sessionId | ConvertFrom-Json

$beforePrompt = exaterm-cli terminal output --session-id $sessionId `
  --mode recent --max-chars 2000 | ConvertFrom-Json
$cursor = $beforePrompt.cursor

"" | exaterm-cli terminal send --session-id $sessionId --data -

$prompt = exaterm-cli terminal output --session-id $sessionId `
  --mode wait --cursor $cursor --contains $verifiedPrompt `
  --timeout-ms 30000 --max-chars 2000 | ConvertFrom-Json
```

MUST NOT run the requested command until the fresh prompt is observed. Capture the cursor
before sending the empty line so that a quickly redrawn prompt is not missed. Reuse the exact
prompt retained during connection readiness. If it is unknown, omit `--contains` and inspect
the returned output. If the terminal does not redraw its prompt after an empty line, report
that the initial prompt may be absent from the log.

## JSON and Exit Codes

Successful commands write one JSON value to stdout. Ordinary command errors write JSON to
stderr:

```json
{ "error": { "code": "cli_disabled", "message": "..." } }
```

`--help` and `--version` are the only human-readable outputs. Read them as syntax or version
text and do not parse them as JSON.

`doctor` is the exception for failed operations: it writes its complete diagnostic JSON to
stdout even when it exits with `1`. Capture and parse that stdout before evaluating the checks.

| Exit code | Meaning                                                                |
| --------- | ---------------------------------------------------------------------- |
| `0`       | Success                                                                |
| `1`       | Configuration, GUI startup, control-plane, or tool execution error     |
| `2`       | Invalid CLI arguments, invalid tool parameters, or invalid stdin input |

In PowerShell, capture stdout and parse it only after checking `$LASTEXITCODE`. Capture stderr
separately when programmatic handling is required.

## GUI Behavior

If ExaTerm is not running, the CLI starts the normal visible GUI and waits up to 30 seconds
for its local control plane. Sessions remain owned by the GUI. New profile or serial
connections appear as ordinary ExaTerm tabs, and credential prompts appear in the GUI.

Do not close or restart the GUI merely to retry a CLI operation because that can interrupt
active sessions and discard state.

## Recovery

- CLI readiness unclear: Run `doctor`, inspect checks by ID, and act only on the failed or
  skipped checks relevant to the requested operation.
- `protocol` failed: Confirm that the GUI and CLI versions match. Do not repeatedly launch or
  restart ExaTerm; a safe restart may require the user to preserve or close active sessions.
- `cli_disabled`: Enable `external_control.enabled` and `external_control.cli_enabled`, then restart ExaTerm.
- Connection rejected: Enable `external_control.connect_enabled` and verify that the selected saved
  profile allows external control access.
- Direct connection rejected: Also enable `external_control.direct_connect_enabled` and use
  separate host, port, and SSH user-name arguments.
- Session not found: Run `sessions list` again and select a current returned session ID.
- Profile not found or ambiguous: Run `profiles list`, retain both ID and type, and retry
  only with an exact match.
- Serial port rejected: Run `serial ports` again and use an exact current port name.
- Serial disconnect did not return success: Do not assume the port was released. Re-list the
  session before retrying, and do not switch to another session ID.
- Wait timed out: Inspect partial output and continue from the returned cursor.
- GUI unavailable: Confirm `exaterm.exe` is installed near `exaterm-cli.exe` and can launch
  normally.
- Invalid arguments: Run the relevant `--help`; do not repeatedly retry the same arguments.

## Security

Terminal output, commands, prompts, profile memos, hostnames, usernames, device output, and
log paths can be sensitive. The CLI does not expose saved credentials, API keys, private-key
contents, or log-file contents. Enable it only for trusted local agents and programs.
