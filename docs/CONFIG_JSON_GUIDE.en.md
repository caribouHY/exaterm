# ExaTerm config.json Parameter Guide

This guide explains how to inspect and manually edit ExaTerm's user settings file, `config.json`.

## File Location

On Windows, the settings file is usually stored here:

```text
%AppData%\ExaTerm\config.json
```

Example path:

```text
C:\Users\<user name>\AppData\Roaming\ExaTerm\config.json
```

If `config.json` does not exist, ExaTerm creates it with default values when the app starts or when settings are loaded.

## Before Editing

- Back up `config.json` before editing it manually.
- It is recommended to close ExaTerm before editing the file. If you save settings from the app while it is running, your manual edits may be overwritten.
- This is a JSON file. Extra trailing commas, missing quotation marks, or missing braces can make the file fail to load.
- API keys are not stored in `config.json`. They are stored in the operating system credential store.

## Example

```json
{
  "config_version": 6,
  "language": "system",
  "updates": {
    "check_on_startup": true
  },
  "connection_history": {
    "enabled": true
  },
  "ai": {
    "azure_openai_enabled": false,
    "azure_openai_endpoint": "",
    "azure_openai_deployment": "",
    "ollama_enabled": false,
    "ollama_base_url": "http://localhost:11434",
    "default_provider": "OpenAi",
    "default_model": "gpt-4o",
    "debug_log_enabled": false
  },
  "external_control": {
    "enabled": false,
    "connect_enabled": false,
    "mcp_enabled": false,
    "cli_enabled": false
  },
  "shortcuts": {
    "new_connection": { "key": "n", "ctrl": true, "alt": false, "shift": false },
    "new_window": { "key": "n", "ctrl": true, "alt": false, "shift": true },
    "open_settings": { "key": ",", "ctrl": true, "alt": false, "shift": false },
    "exit": null,
    "terminal_select_all": { "key": "a", "ctrl": true, "alt": false, "shift": true },
    "terminal_copy": { "key": "c", "ctrl": true, "alt": false, "shift": true },
    "terminal_paste": { "key": "v", "ctrl": true, "alt": false, "shift": true },
    "terminal_clear_viewport": null,
    "terminal_clear_buffer": null,
    "terminal_log_start_overwrite": { "key": "F9", "ctrl": true, "alt": false, "shift": true },
    "terminal_log_start_append": null,
    "terminal_log_stop": { "key": "F10", "ctrl": true, "alt": false, "shift": true },
    "terminal_log_pause": null,
    "terminal_log_resume": null
  },
  "terminal": {
    "font_size": 14,
    "font_family": "Consolas, 'Courier New', monospace",
    "cursor_style": "block",
    "scrollback": 10000,
    "auto_session_log": false,
    "log_format": "display",
    "include_log_header": false
  },
  "ssh": {
    "algorithm_mode": "default",
    "algorithms": {
      "kex": [],
      "host_key": [],
      "cipher": [],
      "mac": [],
      "compression": []
    }
  },
  "saved_connections": []
}
```

## Root Fields

| Parameter            | Type   | Default    | Description                                                                                                                                                |
| -------------------- | ------ | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `config_version`     | number | `6`        | The settings file version. Usually, you should not change this. When an older config is loaded, ExaTerm updates it to the current version.                 |
| `language`           | string | `"system"` | Display language. Use `"system"` to follow the OS language, `"en"` for English, or `"ja"` for Japanese. Unsupported system languages fall back to English. |
| `updates`            | object | See below  | Controls automatic checks for published stable ExaTerm updates.                                                                                            |
| `connection_history` | object | See below  | Controls local SSH and Telnet connection history.                                                                                                          |
| `ai`                 | object | See below  | AI assistant settings.                                                                                                                                     |
| `external_control`   | object | See below  | Local external-control settings for the Terminal CLI and MCP compatibility adapter.                                                                        |
| `shortcuts`          | object | See below  | Customizable application keyboard shortcuts.                                                                                                               |
| `terminal`           | object | See below  | Terminal display and logging settings.                                                                                                                     |
| `ssh`                | object | See below  | SSH connection compatibility settings.                                                                                                                     |
| `saved_connections`  | array  | `[]`       | Saved SSH and Telnet connection profiles. Profiles can be created, selected, and deleted from the connection dialog.                                       |

## updates

| Key                | Type    | Default | Description                                                                                                                                     |
| ------------------ | ------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `check_on_startup` | boolean | `true`  | Checks once for the latest published stable release when the main ExaTerm window starts. Downloads and installation still require confirmation. |

Manual update checks remain available from the app menu when this value is `false`.

## connection_history

| Key       | Type    | Default | Description                                                                                                                                              |
| --------- | ------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `enabled` | boolean | `true`  | Saves settings after successful SSH and Telnet connections. Turning this off stops new entries but keeps existing history available until it is cleared. |

Connection history is stored separately from `config.json` at:

```text
%AppData%\ExaTerm\connection_history.json
```

The file stores hosts, ports, SSH usernames, authentication methods, private-key paths, jump-profile IDs, encodings, terminal modes, and connection timestamps as plaintext. Passwords, private-key passphrases, and jump-host credentials are not stored. SSH and Telnet keep up to 10 entries each. History can be removed individually from the connection dialog or cleared from Settings.

## ai

| Parameter                    | Type    | Default                    | Description                                                                                                                                                                                                                            |
| ---------------------------- | ------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ai.azure_openai_enabled`    | boolean | `false`                    | When set to `true`, Azure OpenAI is shown in the AI panel when the endpoint, model deployment name, and API key are configured.                                                                                                        |
| `ai.azure_openai_endpoint`   | string  | `""`                       | The full Azure OpenAI chat completions URL to send requests to, such as `"https://your-resource.openai.azure.com/openai/v1/chat/completions"` or a deployment URL with `api-version`. ExaTerm uses this URL as entered.                |
| `ai.azure_openai_deployment` | string  | `""`                       | The Azure OpenAI model deployment name. With the v1 API, this value is sent as the `model` field.                                                                                                                                      |
| `ai.ollama_enabled`          | boolean | `false`                    | When set to `true`, Ollama models are shown in the AI panel. To use Ollama, a local or configured Ollama server must be running.                                                                                                       |
| `ai.ollama_base_url`         | string  | `"http://localhost:11434"` | The base URL for the Ollama API. For a standard local setup, use `"http://localhost:11434"`. If this is an empty string, the UI treats it as the default URL.                                                                          |
| `ai.default_provider`        | string  | `"OpenAi"`                 | The provider selected by default in the AI panel. Supported values are `"OpenAi"`, `"AzureOpenAi"`, `"Anthropic"`, `"Gemini"`, `"OpenRouter"`, and `"Ollama"`.                                                                         |
| `ai.default_model`           | string  | `"gpt-4o"`                 | The model ID selected by default in the AI panel. This is not currently editable from the Settings screen, so edit it manually if needed. If the saved model is not available, ExaTerm automatically falls back to an available model. |
| `ai.debug_log_enabled`       | boolean | `false`                    | When set to `true`, AI chat requests and responses are saved as JSON Lines debug logs under `%AppData%\ExaTerm\ai-debug`.                                                                                                              |

### AI API Keys

API keys for OpenAI, Azure OpenAI, Anthropic, Google Gemini, and OpenRouter are not stored in `config.json`. Keys registered from the Settings screen are stored in the operating system credential store.

Enter the full Azure OpenAI chat completions URL and model deployment name. ExaTerm uses the endpoint URL exactly as entered and does not append a path or `api-version` value.

Ollama usually does not require an API key. Configure `ai.ollama_enabled` and `ai.ollama_base_url` instead.

### AI Debug Logs

Set `ai.debug_log_enabled` to `true` only when you need to troubleshoot AI chat behavior. Debug logs are saved to `%AppData%\ExaTerm\ai-debug\YYYYMMDD.log` and may include prompts, AI responses, and terminal context text. API keys and HTTP headers are not saved.

### Common Model IDs

| Provider     | Example model IDs                                       |
| ------------ | ------------------------------------------------------- |
| OpenAI       | `gpt-4o`, `gpt-4o-mini`                                 |
| Azure OpenAI | Your Azure model deployment name, such as `my-gpt4o`    |
| Anthropic    | `claude-sonnet-4-20250514`, `claude-3-5-haiku-20241022` |
| Gemini       | `gemini-2.5-pro`, `gemini-2.5-flash`                    |
| OpenRouter   | `openai/gpt-4o`, `anthropic/claude-sonnet-4`            |
| Ollama       | Model names installed in your local Ollama instance     |

## external_control

| Parameter                          | Type    | Default | Description                                                                                                                                                                                                                                                  |
| ---------------------------------- | ------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `external_control.enabled`         | boolean | `false` | Master permission for local external-control access. Setting it to `true` does not enable an interface by itself; enable `external_control.cli_enabled` or `external_control.mcp_enabled` separately.                                                        |
| `external_control.connect_enabled` | boolean | `false` | When set to `true`, trusted external-control clients can list saved SSH/Telnet profiles, open new ExaTerm tabs from those profiles, list Serial ports, and open Serial consoles. SSH credentials are entered in the ExaTerm UI and are not exposed or saved. |
| `external_control.mcp_enabled`     | boolean | `false` | When set to `true` with `external_control.enabled=true`, local MCP clients can use the `exaterm-mcp` compatibility adapter.                                                                                                                                  |
| `external_control.cli_enabled`     | boolean | `false` | When set to `true` with `external_control.enabled=true`, trusted local programs can use `exaterm-cli` to call the shared terminal-control operations and receive JSON results. See the [CLI guide](CLI_GUIDE.en.md).                                         |

Older config files may still contain the legacy `mcp` object and `saved_connections[*].mcp_enabled`. ExaTerm migrates those fields automatically when loading the config. If both legacy and new settings are present, the new `external_control` fields take priority and the legacy values are used only to fill missing values. After ExaTerm saves the config, only the new `external_control` and `external_control_enabled` fields remain.

The HTTP MCP transport has been removed. Existing HTTP-only `mcp.host` and `mcp.port` values in older config files are ignored and will be omitted the next time ExaTerm saves the settings. Users who previously used HTTP MCP must manually enable `external_control.enabled=true` and `external_control.mcp_enabled=true`, then configure their MCP client to launch `exaterm-mcp`.

### MCP Tools

When MCP is enabled, external clients can call these tools:

- `list_terminal_sessions`: lists ExaTerm terminal sessions opened by the user.
- `read_terminal_output`: reads or waits for session output using the required `mode` argument:
  - `recent`: immediately reads the most recent retained output.
  - `delta`: immediately reads output after the required `cursor`.
  - `wait`: waits for new output or for the optional `contains` substring. When `cursor` is omitted, waiting starts at the current output position.
- `send_terminal_input`: sends text to a connected session.
- `run_terminal_command`: sends a command to a connected session, waits for output, and returns the output delta.
- `start_terminal_log`: explicitly starts a plaintext log for a connected session. The log is saved under `%AppData%\ExaTerm\logs`, and the result returns the created file path.
- `stop_terminal_log`: stops the active plaintext log for a session after ExaTerm flushes pending displayed output to the log.

Example output reads:

```json
{ "session_id": "session-id", "mode": "recent", "max_chars": 2000 }
```

```json
{ "session_id": "session-id", "mode": "delta", "cursor": 1200 }
```

```json
{
  "session_id": "session-id",
  "mode": "wait",
  "cursor": 1200,
  "contains": "router#",
  "timeout_ms": 30000
}
```

When `external_control.connect_enabled` is also `true`, external clients can call these additional tools:

- `list_connection_profiles`: lists saved SSH/Telnet profiles that individually allow external control access. Private key paths and credentials are not returned.
- `connect_saved_profile`: opens a new SSH/Telnet session from a saved profile and shows it as a tab in ExaTerm. It requires both `profile_id` and `connection_type` (`"ssh"` or `"telnet"`), so profiles of different types may share an ID. SSH credentials are requested in the ExaTerm UI.
- `list_serial_ports`: lists currently available Serial ports.
- `connect_serial_console`: opens a new Serial console session from explicit port and line settings and shows it as a tab in ExaTerm. The port name must match an available Serial port exactly.

`read_terminal_output` returns the selected `mode`. It and `run_terminal_command` also return `start_cursor`, `cursor`, and `truncated`. Pass the returned `cursor` to `read_terminal_output` with `mode: "delta"` or `mode: "wait"` to continue from the same point. If older output has been trimmed from the internal buffer, `truncated` is `true`. The `wait` result also returns `matched` and `timed_out`.

The `wait` mode and `run_terminal_command` wait for up to 60 seconds. `run_terminal_command` targets only existing connected sessions; use `connect_saved_profile` for the opt-in saved-profile connection flow.

The previous `read_terminal_output_delta` and `wait_terminal_output` tools were removed. Replace them with `read_terminal_output` using `mode: "delta"` and `mode: "wait"`, respectively.

The MCP compatibility adapter and CLI do not read saved credentials, expose API keys, or read log files directly. External clients can explicitly start and stop session logs, but they receive only the log state and file path, not the log contents. New SSH/Telnet connections are limited to saved profiles, Serial connections require an explicit available port name, and all new external connections require `external_control.connect_enabled=true`. Saved profiles can opt out with `saved_connections[*].external_control_enabled=false`. SSH connections still enforce known-host checks and request required credentials in the ExaTerm UI. Terminal output and log files can contain sensitive information, so enable external control only for trusted local clients.

## shortcuts

Each shortcut is either an object with `key`, `ctrl`, `alt`, and `shift` fields or `null` for an unassigned action. Printable keys and `Space` require `ctrl` or `alt`; `F1` through `F12` can be assigned without a modifier. Assignments must be unique, and `Alt+F4` is reserved by Windows.

| Parameter                                | Default          | Action                                                                            |
| ---------------------------------------- | ---------------- | --------------------------------------------------------------------------------- |
| `shortcuts.new_connection`               | `Ctrl+N`         | Opens the new connection dialog.                                                  |
| `shortcuts.new_window`                   | `Ctrl+Shift+N`   | Opens a new ExaTerm window.                                                       |
| `shortcuts.open_settings`                | `Ctrl+,`         | Opens the Shortcuts and other application settings.                               |
| `shortcuts.exit`                         | Unassigned       | Exits ExaTerm, asking for confirmation if terminal sessions are connected.        |
| `shortcuts.terminal_select_all`          | `Ctrl+Shift+A`   | Selects the terminal screen and scrollback buffer.                                |
| `shortcuts.terminal_copy`                | `Ctrl+Shift+C`   | Copies the selected terminal text.                                                |
| `shortcuts.terminal_paste`               | `Ctrl+Shift+V`   | Pastes clipboard text into a connected terminal.                                  |
| `shortcuts.terminal_clear_viewport`      | Unassigned       | Clears the visible terminal display locally without sending input to the session. |
| `shortcuts.terminal_clear_buffer`        | Unassigned       | Clears the local terminal scrollback buffer without ending the session.           |
| `shortcuts.terminal_log_start_overwrite` | `Ctrl+Shift+F9`  | Opens the save dialog and starts a new log or overwrites the selected file.       |
| `shortcuts.terminal_log_start_append`    | Unassigned       | Opens the save dialog and starts a log by appending to the selected file.         |
| `shortcuts.terminal_log_stop`            | `Ctrl+Shift+F10` | Flushes pending displayed output and stops the active log.                        |
| `shortcuts.terminal_log_pause`           | Unassigned       | Pauses active logging for the terminal session.                                   |
| `shortcuts.terminal_log_resume`          | Unassigned       | Resumes paused logging for the terminal session.                                  |

Letter keys are stored in lowercase, `Space` uses the literal string `"Space"`, and function keys use uppercase names such as `"F2"`. Modifier matching is exact. For example, `Ctrl+Shift+N` does not also match `Ctrl+N`.

Terminal shortcuts run only while the terminal has keyboard focus. Assigning `Ctrl+A`, `Ctrl+C`, or `Ctrl+V` to an application or terminal action overrides common remote shortcuts such as beginning-of-line, interrupt, and quoted insert while ExaTerm has focus.

Clearing the terminal display or buffer affects only the local xterm view. It does not send a clear command to the connected device, end the session, delete saved logs, or delete backend-retained output.

## terminal

| Parameter                     | Type    | Default                                | Description                                                                                                                                                  |
| ----------------------------- | ------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `terminal.font_size`          | number  | `14`                                   | Terminal font size. The Settings screen allows values from `8` to `32`.                                                                                      |
| `terminal.font_family`        | string  | `"Consolas, 'Courier New', monospace"` | Terminal font family. Use the same format as CSS `font-family`.                                                                                              |
| `terminal.cursor_style`       | string  | `"block"`                              | Terminal cursor shape. The default is a block cursor. When editing manually, use a value accepted by xterm.js, such as `"block"`, `"underline"`, or `"bar"`. |
| `terminal.scrollback`         | number  | `10000`                                | Number of terminal scrollback lines. Larger values keep more history but may increase memory usage.                                                          |
| `terminal.auto_session_log`   | boolean | `false`                                | When set to `true`, a plaintext log starts automatically for each new SSH, serial, or Telnet connection.                                                     |
| `terminal.log_format`         | string  | `"display"`                            | Session log formatting mode. `"display"` saves text closer to the terminal screen; `"strip_controls"` removes control sequences.                             |
| `terminal.include_log_header` | boolean | `false`                                | When set to `true`, new session log files start with an ExaTerm header containing the connection type, target, log mode, and start time.                     |

### Session Log Notice

When `terminal.auto_session_log` is set to `true`, ExaTerm starts the same controllable log used by the status bar for every new connection. Automatically started logs can be paused, resumed, or stopped. Stopping one does not restart it during the same session; the setting is evaluated again for the next connection. Logs may contain sensitive information such as:

- Commands and command output
- Hostnames, usernames, and prompts
- Server or network device output
- Accidentally entered tokens, passwords, or secrets

Logs are usually stored here:

```text
%AppData%\ExaTerm\logs
```

In sensitive environments, enable session logging only when necessary.

When `terminal.log_format` is `"display"`, common line edits such as Backspace, cursor-left, and erase-to-end-of-line are applied before text is saved. When it is `"strip_controls"`, control sequences are removed, but partially edited text may remain.

When `terminal.include_log_header` is `false`, new session logs start directly with terminal content instead of the ExaTerm header. Existing log files are not changed. The session log history shows `Auto` or `Manual` as the log mode.

## ssh

| Parameter                    | Type   | Default     | Description                                                                                |
| ---------------------------- | ------ | ----------- | ------------------------------------------------------------------------------------------ |
| `ssh.algorithm_mode`         | string | `"default"` | Uses `"default"` for the recommended defaults or `"custom"` for the configured allowlists. |
| `ssh.algorithms.kex`         | array  | `[]`        | Allowed key exchange algorithms in custom mode.                                            |
| `ssh.algorithms.host_key`    | array  | `[]`        | Allowed server host-key algorithms in custom mode.                                         |
| `ssh.algorithms.cipher`      | array  | `[]`        | Allowed symmetric ciphers in custom mode.                                                  |
| `ssh.algorithms.mac`         | array  | `[]`        | Allowed message authentication algorithms in custom mode.                                  |
| `ssh.algorithms.compression` | array  | `[]`        | Allowed compression algorithms in custom mode.                                             |

### Available SSH Algorithms

The Settings screen shows the catalog supported by the bundled SSH library. In custom mode, select at least one algorithm in every category. ExaTerm applies its built-in priority order rather than the order of the JSON arrays. Compatibility algorithms such as SHA-1 key exchange, CBC/3DES ciphers, and `ssh-rsa` are identified in the UI.

Unknown names, duplicate names, and empty custom categories are rejected. Internal SSH extension markers, such as strict key exchange and extension info markers, are managed automatically and must not be added to the configuration.

## saved_connections

`saved_connections` is an array of saved SSH and Telnet connection profiles. Profiles can be managed from the connection dialog. Serial profiles are not currently supported. Passwords, private key contents, key passphrases, and other credentials are not stored in this section. Profile memos are stored as plaintext and may be returned by `list_connection_profiles` when external profile connections are enabled, so do not put secrets in them. Existing profiles treat a missing `external_control_enabled` value as `true`.

| Parameter                  | Type           | Description                                                                                                                                                                                                                     |
| -------------------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `id`                       | string         | Profile name and identifier.                                                                                                                                                                                                    |
| `connection_type`          | string         | Connection type. Supported profile values are `"ssh"` and `"telnet"`.                                                                                                                                                           |
| `host`                     | string or null | SSH or Telnet target host.                                                                                                                                                                                                      |
| `port`                     | number or null | SSH or Telnet target port.                                                                                                                                                                                                      |
| `username`                 | string or null | SSH username. Not used for Telnet profiles.                                                                                                                                                                                     |
| `encoding`                 | string or null | Initial terminal display encoding for the profile. Supported values are `"utf-8"`, `"shift-jis"`, and `"euc-jp"`. Missing values default to `"utf-8"`.                                                                          |
| `terminal_mode`            | string or null | Initial terminal mode for the profile. Supported values are `"general"`, `"cisco_ios"`, `"arista_eos"`, `"vyos"`, `"fujitsu_sir"`, `"allied_telesis_awplus"`, and `"furukawa_fitelnet"`. Missing values default to `"general"`. |
| `auth_method`              | string or null | SSH authentication method. Supported values are `"password"` and `"public_key"`. Missing values default to `"password"`. Not used for Telnet profiles.                                                                          |
| `private_key_path`         | string or null | Private key file path used with SSH `"public_key"` authentication, such as an `id_ed25519` file. The file contents and passphrase are not stored.                                                                               |
| `jump_profile_id`          | string or null | SSH jump host profile ID. The referenced profile must be a saved SSH profile. Only one jump host is supported; nested jump hosts are rejected.                                                                                  |
| `memo`                     | string or null | Optional plaintext profile memo, such as the device model, role, or operational notes. Non-empty memos may be returned by external-control profile listing.                                                                     |
| `external_control_enabled` | boolean        | Whether trusted CLI and MCP clients may list and open this saved profile. Missing values default to `true`.                                                                                                                     |

Example:

```json
{
  "id": "dev-server",
  "connection_type": "ssh",
  "host": "192.168.1.10",
  "port": 22,
  "username": "admin",
  "auth_method": "public_key",
  "private_key_path": "C:\\Users\\user\\.ssh\\id_ed25519",
  "jump_profile_id": "bastion",
  "encoding": "shift-jis",
  "terminal_mode": "cisco_ios",
  "memo": "Cisco ISR branch edge"
}
```

When `jump_profile_id` is set, ExaTerm first connects to the referenced SSH profile and then opens the target connection through that jump host. The jump host profile cannot reference another jump host, and a profile cannot reference itself. SSH passwords and encrypted key passphrases for both the jump host and the target are requested in the ExaTerm UI and are not saved in `config.json`.

Telnet example:

```json
{
  "id": "legacy-router",
  "connection_type": "telnet",
  "host": "192.168.1.20",
  "port": 23,
  "encoding": "euc-jp",
  "terminal_mode": "cisco_ios",
  "memo": "Legacy access switch"
}
```

## Common Edits

### Switch the UI to Japanese

```json
"language": "ja"
```

### Follow the system language

```json
"language": "system"
```

### Enable Ollama

```json
"ai": {
  "ollama_enabled": true,
  "ollama_base_url": "http://localhost:11434",
  "default_provider": "Ollama",
  "default_model": "llama3.1",
  "debug_log_enabled": false
}
```

Set `default_model` to a model name installed in Ollama.

### Enable Azure OpenAI

```json
"ai": {
  "azure_openai_enabled": true,
  "azure_openai_endpoint": "https://your-resource.openai.azure.com/openai/v1/chat/completions",
  "azure_openai_deployment": "my-gpt4o",
  "default_provider": "AzureOpenAi",
  "default_model": "my-gpt4o",
  "debug_log_enabled": false
}
```

Save the Azure OpenAI API key from the Settings screen. ExaTerm sends requests to the configured endpoint URL without modifying it.

### Use OpenRouter

```json
"ai": {
  "default_provider": "OpenRouter",
  "default_model": "openai/gpt-4o",
  "debug_log_enabled": false
}
```

Save the OpenRouter API key from the Settings screen. ExaTerm fetches available models from OpenRouter and falls back to representative model IDs if the model list cannot be loaded.

### Disable Session Logs

```json
"terminal": {
  "auto_session_log": false
}
```

Keep the other fields in the actual `terminal` object. The snippet above only shows the field being changed.

### Select SSH Algorithms

```json
"ssh": {
  "algorithm_mode": "custom",
  "algorithms": {
    "kex": ["curve25519-sha256", "diffie-hellman-group14-sha1"],
    "host_key": ["ssh-ed25519", "ssh-rsa"],
    "cipher": ["aes256-ctr", "aes128-cbc"],
    "mac": ["hmac-sha2-256", "hmac-sha1"],
    "compression": ["none"]
  }
}
```

Prefer the Settings screen when creating a custom selection. Existing configurations that use `allow_legacy_algorithms` are migrated automatically.

## Troubleshooting

| Symptom                              | Action                                                                                                                                                                                                                                                                                   |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ExaTerm cannot load settings         | Check the JSON syntax, especially extra commas, quotation marks, and braces.                                                                                                                                                                                                             |
| Changes are not reflected            | Restart ExaTerm or save the settings again from the Settings screen.                                                                                                                                                                                                                     |
| An AI provider does not appear       | Cloud providers require API keys. For Azure OpenAI, also check `azure_openai_enabled`, `azure_openai_endpoint`, and `azure_openai_deployment`. For OpenRouter, save the OpenRouter API key from Settings. For Ollama, check `ollama_enabled` and make sure the Ollama server is running. |
| An older SSH device will not connect | If the error indicates no common SSH algorithm, choose a custom selection in Settings and enable only the compatibility algorithms required by that device.                                                                                                                              |
| Text is hard to read                 | Adjust `terminal.font_size` or `terminal.font_family`.                                                                                                                                                                                                                                   |
| You do not want logs to be saved     | Set `terminal.auto_session_log` to `false`. If logs were already created, delete them from `%AppData%\ExaTerm\logs` as needed.                                                                                                                                                           |
