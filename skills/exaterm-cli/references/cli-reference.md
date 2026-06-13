# ExaTerm CLI Reference

## Requirements

`exaterm-cli.exe` is distributed with supported Windows builds of ExaTerm and is normally
installed beside `exaterm.exe`. Add that directory to the current PowerShell `PATH` or invoke
the executable by its full path.

Enable the shared external-control service and CLI access in ExaTerm Settings, or configure:

```json
{
  "mcp": {
    "enabled": true,
    "cli_enabled": true,
    "stdio_enabled": false,
    "connect_enabled": false
  }
}
```

- `mcp.enabled` is the master permission for MCP and CLI access.
- `mcp.cli_enabled` permits CLI operations.
- `mcp.connect_enabled` additionally permits saved-profile and serial connections.
- `mcp.stdio_enabled` affects `exaterm-mcp`, not `exaterm-cli`.

Restart ExaTerm after changing these settings. Individual saved profiles must also allow MCP
access before the CLI can list or connect them.

## Commands

```text
exaterm-cli sessions list
exaterm-cli profiles list [--type <ssh|telnet>]
exaterm-cli profiles connect --type <ssh|telnet> --profile-id <id> [--cols <n>] [--rows <n>]
exaterm-cli serial ports
exaterm-cli serial connect --port <name> [options]
exaterm-cli terminal output --session-id <id> --mode <recent|delta|wait> [options]
exaterm-cli terminal send --session-id <id> --data <text|->
exaterm-cli terminal run --session-id <id> --command <text|-> [options]
exaterm-cli terminal log start --session-id <id>
exaterm-cli terminal log stop --session-id <id>
```

Use `exaterm-cli <command> --help` for the syntax supported by the installed version.

## Sessions and Profiles

`sessions list` returns JSON containing a `sessions` array. Use a returned `session_id` for
terminal operations.

`profiles list` returns a `profiles` array containing approved SSH and Telnet profiles.
Secrets and private-key paths are not returned. SSH and Telnet profiles may share an ID, so
always retain both the returned profile ID and connection type. `--type` accepts only `ssh`
or `telnet`.

`profiles connect` requires an exact returned ID and type. `--cols` and `--rows` each accept
values from 1 through 1000. Profile connections require `mcp.connect_enabled=true`.

SSH passwords and encrypted private-key passphrases are entered through the visible ExaTerm
UI and must never be supplied as CLI arguments.

## Connection Readiness

A successful `profiles connect` or `serial connect` result means that ExaTerm created a
session. It does not guarantee that the initial banner, login exchange, or normal prompt has
finished rendering.

After connecting:

1. Retain the returned `session_id`.
2. Run `sessions list` and confirm that the matching session has status `connected`.
3. Read recent output and retain its cursor:

   ```powershell
   $initial = exaterm-cli terminal output --session-id $sessionId `
     --mode recent --max-chars 4000 | ConvertFrom-Json
   $cursor = $initial.cursor
   ```

4. Inspect whether the output shows a normal prompt, a login prompt, an incomplete banner,
   or no output.
5. If readiness is unclear, wait from the retained cursor:

   ```powershell
   $ready = exaterm-cli terminal output --session-id $sessionId `
     --mode wait --cursor $cursor --timeout-ms 30000 `
     --max-chars 4000 | ConvertFrom-Json
   ```

Use `--contains` only when a reliable expected prompt is known. Some serial consoles remain
silent until input is sent. Sending an empty line changes the remote interaction, so follow
the host agent's normal approval policy before doing so.

## Serial Connections

`serial ports` returns a `ports` array. Pass an exact returned port name to `serial connect`.
Serial connections require `mcp.connect_enabled=true`.

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
  --mode wait --cursor $cursor --contains "router#" `
  --timeout-ms 30000 | ConvertFrom-Json
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

```powershell
"show version`n" |
  exaterm-cli terminal send --session-id $sessionId --data -
```

`terminal run` sends a command and returns captured output:

```powershell
$result = exaterm-cli terminal run --session-id $sessionId `
  --command "show version" --wait-contains "#" `
  --timeout-ms 30000 --max-chars 4000 | ConvertFrom-Json
```

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
$result = exaterm-cli terminal run --session-id $sessionId `
  --command "long-running-command" --wait-contains "router#" `
  --timeout-ms 60000 --max-chars 20000 | ConvertFrom-Json

$cursor = $result.cursor
$deadline = (Get-Date).AddMinutes(10)

while ($result.timed_out -and (Get-Date) -lt $deadline) {
  $result = exaterm-cli terminal output --session-id $sessionId `
    --mode wait --cursor $cursor --contains "router#" `
    --timeout-ms 60000 --max-chars 20000 | ConvertFrom-Json
  $cursor = $result.cursor
}
```

- Do not resend the command after a wait timeout.
- Update the cursor after every result, including timeouts.
- Treat the expected prompt or another command-specific completion marker as completion.
- If the overall deadline expires, report that the command remains unconfirmed rather than
  reporting failure or success.
- If `truncated=true`, report that the returned output is incomplete. Continuing from the
  latest cursor observes future output but does not recover already truncated content.
- Re-run `sessions list` if waiting fails because the session may have disconnected.

## Manual Logging

```powershell
exaterm-cli terminal log start --session-id $sessionId
exaterm-cli terminal log stop --session-id $sessionId
```

Manual logs are plaintext and can contain commands, prompts, output, hostnames, usernames,
and accidental secrets. Start them only when the user explicitly requests logging.

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
  --mode wait --cursor $cursor --contains "router#" `
  --timeout-ms 30000 --max-chars 2000 | ConvertFrom-Json
```

Only run the requested command after the fresh prompt is observed. Capture the cursor before
sending the empty line so that a quickly redrawn prompt is not missed. Replace `router#`
with a verified prompt; omit `--contains` when it is unknown and inspect the returned output.
If the terminal does not redraw its prompt after an empty line, report that the initial
prompt may be absent from the log.

## JSON and Exit Codes

Successful commands write one JSON value to stdout. Errors write JSON to stderr:

```json
{ "error": { "code": "cli_disabled", "message": "..." } }
```

`--help` and `--version` are the only human-readable outputs.

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

- `cli_disabled`: Enable `mcp.enabled` and `mcp.cli_enabled`, then restart ExaTerm.
- Connection rejected: Enable `mcp.connect_enabled` and verify that the selected saved
  profile allows MCP access.
- Session not found: Run `sessions list` again and select a current returned session ID.
- Profile not found or ambiguous: Run `profiles list`, retain both ID and type, and retry
  only with an exact match.
- Serial port rejected: Run `serial ports` again and use an exact current port name.
- Wait timed out: Inspect partial output and continue from the returned cursor.
- GUI unavailable: Confirm `exaterm.exe` is installed near `exaterm-cli.exe` and can launch
  normally.
- Invalid arguments: Run the relevant `--help`; do not repeatedly retry the same arguments.

## Security

Terminal output, commands, prompts, profile memos, hostnames, usernames, device output, and
log paths can be sensitive. The CLI does not expose saved credentials, API keys, private-key
contents, or log-file contents. Enable it only for trusted local agents and programs.
