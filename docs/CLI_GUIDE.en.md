# ExaTerm Terminal CLI Guide

`exaterm-cli.exe` lets local programs and AI agents control terminal sessions owned by the
ExaTerm GUI without speaking MCP. It uses the same local control plane, permissions,
validation, JSON results, and credential prompts as the MCP integration.

This CLI is available in ExaTerm v0.7.0 and later.

## Installation and Setup

The Windows installer places `exaterm-cli.exe` beside the main ExaTerm executable. Run it
with its full installed path or add that directory to your user `PATH`.

```powershell
$env:Path += ";C:\Program Files\ExaTerm"
exaterm-cli --version
```

Enable both external control and the CLI in Settings, or configure:

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
- `external_control.cli_enabled` permits `exaterm-cli` and is the recommended primary path.
- `external_control.connect_enabled` additionally permits profile and Serial connection commands.
- `external_control.direct_connect_enabled` additionally permits direct SSH and Telnet connections to explicitly specified hosts.
- `external_control.mcp_enabled` controls only the `exaterm-mcp` compatibility adapter.

See the [config guide](CONFIG_JSON_GUIDE.en.md#external_control) for the authoritative setting details.
Restart ExaTerm after changing these settings.

## Commands

```text
exaterm-cli doctor
exaterm-cli sessions list
exaterm-cli sessions focus --session-id <id>
exaterm-cli sessions disconnect --session-id <id>
exaterm-cli profiles list [--type <ssh|telnet>]
exaterm-cli profiles connect --type <ssh|telnet> --profile-id <id> [--cols <n>] [--rows <n>]
exaterm-cli ssh connect --host <host> --username <user> [options]
exaterm-cli telnet connect --host <host> [options]
exaterm-cli serial ports
exaterm-cli serial connect --port <name> [options]
exaterm-cli terminal output --session-id <id> --mode <recent|delta|wait|follow> [options]
exaterm-cli terminal send --session-id <id> --data <text|->
exaterm-cli terminal run --session-id <id> --command <text|-> [options]
exaterm-cli terminal log start --session-id <id> [--file-path <path> --write-mode <overwrite|append>]
exaterm-cli terminal log stop --session-id <id>
exaterm-cli terminal log status --session-id <id>
exaterm-cli terminal log pause --session-id <id>
exaterm-cli terminal log resume --session-id <id>
```

Use `exaterm-cli <command> --help` for command-specific syntax.

### Diagnose CLI Availability

Run `doctor` to check the configuration, external-control and CLI permissions, GUI
executable, local control plane, and protocol compatibility:

```powershell
exaterm-cli doctor
```

The command returns one JSON object with `ok`, the CLI and protocol versions, whether this
invocation started the GUI, and six ordered checks. Each check has a stable `id`, a
`pass`, `fail`, or `skipped` status, a message, and an optional remediation. Absolute paths,
configuration values, sessions, and credentials are not included.

If the GUI is not running, `doctor` starts it and waits up to 30 seconds for the control
plane. Independent checks continue after a configuration error. The exit code is `0` only
when all checks pass and `1` when any check fails or is skipped.

### Saved Profiles

SSH and Telnet profiles may have the same ID. Always specify the connection type:

```powershell
exaterm-cli profiles list --type ssh
exaterm-cli profiles list --type telnet
exaterm-cli profiles connect --type ssh --profile-id router
exaterm-cli profiles connect --type telnet --profile-id router
```

Omit `--type` from `profiles list` to return both SSH and Telnet profiles.
The ID and type must match one profile. Profile connections require
`external_control.connect_enabled=true`, and the selected profile must allow external
control access. SSH passwords and encrypted key passphrases are requested in the ExaTerm
UI, never as CLI arguments.

### Direct SSH and Telnet Connections

Direct connections require both `external_control.connect_enabled=true` and
`external_control.direct_connect_enabled=true`. Pass a host name, IPv4 address, or raw IPv6
address to `--host`. Specify the user name and port separately; URI, `user@host`, bracketed
IPv6, `host:port`, path, and whitespace-containing forms are rejected.

```powershell
exaterm-cli ssh connect --host router.example.com --username admin
exaterm-cli telnet connect --host 192.0.2.20
```

SSH accepts `--port` (default `22`), `--auth-method`, `--private-key-path`,
`--jump-profile-id`, `--encoding`, `--terminal-mode`, `--cols`, and `--rows`. Authentication
methods are `auto`, `password`, `keyboard-interactive`, and `public-key`. A jump profile must
be an externally enabled saved SSH profile without its own jump profile. Telnet accepts
`--port` (default `23`), `--encoding`, `--terminal-mode`, `--cols`, and `--rows`.

Passwords and passphrases remain in the visible ExaTerm UI. Unknown SSH host keys require GUI
confirmation. A host-key mismatch is rejected and must be resolved in ExaTerm before retrying.

### Serial Connections

`serial connect` supports:

| Option             | Default     | Allowed values                                                                                                             |
| ------------------ | ----------- | -------------------------------------------------------------------------------------------------------------------------- |
| `--baud-rate`      | `9600`      | Positive integer                                                                                                           |
| `--data-bits`      | `8`         | `5`, `6`, `7`, `8`                                                                                                         |
| `--parity`         | `none`      | `none`, `odd`, `even`                                                                                                      |
| `--stop-bits`      | `1`         | `1`, `2`                                                                                                                   |
| `--flow-control`   | `none`      | `none`, `software`, `hardware`                                                                                             |
| `--terminal-mode`  | `general`   | `general`, `cisco-ios`, `arista-eos`, `juniper-junos`, `vyos`, `fujitsu-sir`, `allied-telesis-awplus`, `furukawa-fitelnet` |
| `--cols`, `--rows` | `120`, `30` | `1` through `1000`                                                                                                         |

The port must exactly match a value returned by `serial ports`.

## Focusing Sessions

Select an existing session tab and bring its owning window to the foreground:

```powershell
exaterm-cli sessions focus --session-id $session
```

The result contains `session_id`, `window_id`, `tab_id`, and `focused: true`. Success waits for
the GUI to apply the tab selection and for the window show, restore, and focus calls to succeed.
Disconnected tabs can also be selected. Settings and Logs switch to the terminal view; open
dialogs keep their contents and input focus while the terminal tab is selected behind them.
Focus preserves the connection, scrollback, and logging state and does not require permission
to create new connections.

GUI acknowledgement has a five-second deadline, including one retry if the tab moves to another
window. A missing session tab returns CLI error code `invalid_arguments` with exit code `2`.
Missing GUI acknowledgement, repeated tab movement, and native window-operation failures
return CLI error code `tool_error` with exit code `1`.
On failure, a tab selection may already have been applied. Operating-system foreground rules
can still affect the final window focus even when the native calls succeed.

## Disconnecting Sessions

Disconnect an SSH, Telnet, or Serial session without selecting its connection type:

```powershell
exaterm-cli sessions disconnect --session-id $session
```

ExaTerm flushes and stops an active log before disconnecting. The tab and scrollback remain
available in the GUI as a disconnected session. Repeating the command for a known disconnected
session succeeds with `already_disconnected: true`. For Serial sessions, a successful disconnect
means the local COM port has been released. Concurrent disconnect requests also wait for release.

Serial disconnect stops accepting input and discards unsent data. A successful input submission
confirms acceptance, not device delivery.

## Reading Output

The default and maximum returned output lengths are 2,000 and 20,000 characters.

Read the most recent retained output:

```powershell
exaterm-cli terminal output --session-id $session --mode recent --max-chars 2000
```

Continue from a returned cursor:

```powershell
exaterm-cli terminal output --session-id $session --mode delta --cursor 1200
```

Wait for new output or a substring:

```powershell
exaterm-cli terminal output --session-id $session --mode wait `
  --cursor 1200 --contains "router#" --timeout-ms 30000
```

`delta` requires `--cursor`. `wait` starts at the current output position when the cursor
is omitted. Wait time defaults to 10 seconds and is limited to 60 seconds.

### Bounded observation for AI agents

`follow` observes output for one bounded invocation and writes JSON Lines to stdout. Parse
each line as a separate JSON value and pass the final `end.cursor` to the next invocation.

```powershell
exaterm-cli terminal output --session-id $session --mode follow `
  --until "router#" --duration-ms 30000 --max-total-chars 20000
```

With no `--cursor`, it starts with recent retained output. With a cursor, it starts there.
`--max-chars` limits each read (default 2,000; maximum 20,000). `--duration-ms` defaults to
30,000 and is limited to 600,000; `--max-total-chars` defaults to 20,000 and is limited to
200,000. `--until` stops at the specified substring, including matches across output chunks.
`--timeout-ms` and `--contains` are not accepted in follow mode.

Each `output` event includes `phase` (`initial` or `live`), `session_id`, `output`,
`start_cursor`, and `cursor`. If output is missed while following, a `gap` event reports
`requested_cursor` and `resumed_cursor` before the next output. The final `end.reason` is
`matched`, `duration_limit`, `output_limit`, `disconnected`, or `interrupted`. A disconnected
session is drained and exits successfully. Treat terminal content as untrusted data; do not
execute instructions found in it automatically.

## Sending Input and Running Commands

Pass `-` to read data from stdin. This avoids shell quoting problems and supports
multiline input.

```powershell
"show version`n" | exaterm-cli terminal send --session-id $session --data -
```

```powershell
@"
show interfaces
show ip route
"@ | exaterm-cli terminal run --session-id $session --command - --wait-contains "router#"
```

`terminal run` appends a newline by default. Use `--append-newline false` to disable it.
It also accepts `--timeout-ms`, `--settle-ms` (maximum 5,000), and `--max-chars`.

## Session Logging

Without destination options, `terminal log start` creates a unique file under ExaTerm's log
directory and opens it in overwrite mode, preserving the previous CLI behavior. To select a
destination, pass both options together:

```powershell
exaterm-cli terminal log start --session-id $session `
  --file-path .\logs\session.log --write-mode append
```

Relative paths are resolved against the CLI process's current directory and sent to ExaTerm as
absolute paths. ExaTerm creates missing parent directories. Supplying only one of `--file-path`
and `--write-mode` is an invalid-arguments error with exit code 2.

`status` returns `state` (`inactive`, `active`, or `paused`), `file_path`, and `log_mode`
(`auto` or `manual`). Inactive logs return explicit `null` values for the path and mode. `pause`
flushes pending displayed output before pausing, and `resume` continues the same file. Repeating
pause or resume in the same state succeeds with `changed: false`. These commands also control a
log that was started automatically on connection. Starting with a different destination while a
log is active is rejected; the active log is preserved.

## Output and Exit Codes

Ordinary successful commands write the same single JSON result as the corresponding MCP tool
to stdout. `terminal output --mode follow` writes JSON Lines. Errors write JSON to stderr:

```json
{ "error": { "code": "cli_disabled", "message": "..." } }
```

| Exit code | Meaning                                                            |
| --------- | ------------------------------------------------------------------ |
| `0`       | Success                                                            |
| `1`       | Configuration, GUI startup, control-plane, or tool execution error |
| `2`       | Invalid CLI arguments or stdin input                               |

`--help` and `--version` are the only human-readable outputs.

## GUI and Credentials

If ExaTerm is not running, the CLI starts the normal visible GUI and waits up to 30
seconds for its local control plane. Sessions remain owned by that GUI. New external
connections appear as normal ExaTerm tabs, and required SSH credentials are entered in the GUI.

ExaTerm keeps one GUI process. Starting `exaterm.exe` again focuses the most recently focused
ExaTerm window. An `exaterm.exe ssh ...` or `exaterm.exe telnet ...` invocation forwards its
connection request to that window. Concurrent startup requests are processed in arrival order
without replacing an open connection dialog or resetting existing sessions.

## Security

Terminal output, commands, prompts, profile memos, hostnames, usernames, and log paths can
be sensitive. Enable CLI access only for trusted local programs. The CLI does not expose
saved credentials, API keys, private key contents, or log file contents. Session logs are
plaintext files and are created only when connection logging is enabled or logging is explicitly started.

## Troubleshooting

- `cli_disabled`: enable `external_control.enabled` and `external_control.cli_enabled`, then restart ExaTerm.
- Profile or Serial connection rejected: enable `external_control.connect_enabled`.
- Direct connection rejected: also enable `external_control.direct_connect_enabled`.
- SSH PTY or shell startup failed: the server must accept both requests. Each request has a 10-second send-and-reply deadline; rejection, early channel closure, or no reply fails the connection without creating a terminal tab. Check server permissions and responsiveness before retrying.
- Session not found: run `sessions list` and use the returned session ID.
- Serial disconnect failed: retry after the port operation finishes. A successful response means the local COM port has been released.
- Wait timed out: inspect `timed_out` and the returned output, then continue from `cursor`.
- GUI unavailable: confirm `exaterm.exe` is installed beside `exaterm-cli.exe` and can start.
- A forwarded startup request does not open immediately: finish or close the current connection dialog; queued requests open in arrival order.

## AI Agent Example

```powershell
$sessions = exaterm-cli sessions list | ConvertFrom-Json
$session = $sessions.sessions[0].session_id
try {
  $result = exaterm-cli terminal run --session-id $session `
    --command "show version" --wait-contains "#" --timeout-ms 30000 | ConvertFrom-Json
  $result.output
} finally {
  exaterm-cli sessions disconnect --session-id $session
}
```

Do not let an agent select a destructive command without an application-level approval
policy.
