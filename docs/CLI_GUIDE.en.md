# ExaTerm Terminal CLI Guide

`exaterm-cli.exe` lets local programs and AI agents control terminal sessions owned by the
ExaTerm GUI without speaking MCP. It uses the same local control plane, permissions,
validation, JSON results, and credential prompts as the MCP integration.

This CLI is available in builds after ExaTerm v0.6.1 that include `exaterm-cli.exe`.

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
  "mcp": {
    "enabled": true,
    "cli_enabled": true,
    "stdio_enabled": false,
    "connect_enabled": false
  }
}
```

- `mcp.enabled` is the master permission for MCP and CLI access.
- `mcp.cli_enabled` permits `exaterm-cli`.
- `mcp.connect_enabled` additionally permits profile and Serial connection commands.
- `mcp.stdio_enabled` controls only `exaterm-mcp`.

See the [config guide](CONFIG_JSON_GUIDE.en.md#mcp) for the authoritative setting details.
Restart ExaTerm after changing these settings.

## Commands

```text
exaterm-cli sessions list
exaterm-cli profiles list
exaterm-cli profiles connect --type <ssh|telnet> --profile-id <id> [--cols <n>] [--rows <n>]
exaterm-cli serial ports
exaterm-cli serial connect --port <name> [options]
exaterm-cli terminal output --session-id <id> --mode <recent|delta|wait> [options]
exaterm-cli terminal send --session-id <id> --data <text|->
exaterm-cli terminal run --session-id <id> --command <text|-> [options]
exaterm-cli terminal log start --session-id <id>
exaterm-cli terminal log stop --session-id <id>
```

Use `exaterm-cli <command> --help` for command-specific syntax.

### Saved Profiles

SSH and Telnet profiles may have the same ID. Always specify the connection type:

```powershell
exaterm-cli profiles connect --type ssh --profile-id router
exaterm-cli profiles connect --type telnet --profile-id router
```

The ID and type must match one profile. Profile connections require
`mcp.connect_enabled=true`, and the selected profile must allow MCP access. SSH passwords
and encrypted key passphrases are requested in the ExaTerm UI, never as CLI arguments.

### Serial Connections

`serial connect` supports:

| Option             | Default     | Allowed values                 |
| ------------------ | ----------- | ------------------------------ |
| `--baud-rate`      | `9600`      | Positive integer               |
| `--data-bits`      | `8`         | `5`, `6`, `7`, `8`             |
| `--parity`         | `none`      | `none`, `odd`, `even`          |
| `--stop-bits`      | `1`         | `1`, `2`                       |
| `--flow-control`   | `none`      | `none`, `software`, `hardware` |
| `--terminal-mode`  | `general`   | `general`, `cisco-ios`         |
| `--cols`, `--rows` | `120`, `30` | `1` through `1000`             |

The port must exactly match a value returned by `serial ports`.

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

## Output and Exit Codes

Successful commands write the same JSON result as the corresponding MCP tool to stdout.
Errors write JSON to stderr:

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
seconds for its local control plane. Sessions remain owned by that GUI. Profile connections
appear as normal ExaTerm tabs, and required SSH credentials are entered in the GUI.

## Security

Terminal output, commands, prompts, profile memos, hostnames, usernames, and log paths can
be sensitive. Enable CLI access only for trusted local programs. The CLI does not expose
saved credentials, API keys, private key contents, or log file contents. Manual logs are
plaintext files and remain opt-in.

## Troubleshooting

- `cli_disabled`: enable `mcp.enabled` and `mcp.cli_enabled`, then restart ExaTerm.
- Profile or Serial connection rejected: enable `mcp.connect_enabled`.
- Session not found: run `sessions list` and use the returned session ID.
- Wait timed out: inspect `timed_out` and the returned output, then continue from `cursor`.
- GUI unavailable: confirm `exaterm.exe` is installed beside `exaterm-cli.exe` and can start.

## AI Agent Example

```powershell
$sessions = exaterm-cli sessions list | ConvertFrom-Json
$session = $sessions.sessions[0].session_id
$result = exaterm-cli terminal run --session-id $session `
  --command "show version" --wait-contains "#" --timeout-ms 30000 | ConvertFrom-Json
$result.output
```

Do not let an agent select a destructive command without an application-level approval
policy.
