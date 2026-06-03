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
  "config_version": 1,
  "language": "en",
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
  "mcp": {
    "enabled": false,
    "connect_enabled": false,
    "stdio_enabled": false
  },
  "terminal": {
    "font_size": 14,
    "font_family": "Consolas, 'Courier New', monospace",
    "cursor_style": "block",
    "scrollback": 10000,
    "auto_session_log": false,
    "log_format": "display",
    "include_log_header": true
  },
  "ssh": {
    "allow_legacy_algorithms": false
  },
  "saved_connections": []
}
```

## Root Fields

| Parameter           | Type   | Default   | Description                                                                                                                                |
| ------------------- | ------ | --------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `config_version`    | number | `1`       | The settings file version. Usually, you should not change this. When an older config is loaded, ExaTerm updates it to the current version. |
| `language`          | string | `"en"`    | Display language. Use `"en"` for English or `"ja"` for Japanese.                                                                           |
| `ai`                | object | See below | AI assistant settings.                                                                                                                     |
| `mcp`               | object | See below | Local MCP settings for external AI agent terminal control.                                                                                 |
| `terminal`          | object | See below | Terminal display and logging settings.                                                                                                     |
| `ssh`               | object | See below | SSH connection compatibility settings.                                                                                                     |
| `saved_connections` | array  | `[]`      | Saved SSH and Telnet connection profiles. Profiles can be created, selected, and deleted from the connection dialog.                       |

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

## mcp

| Parameter             | Type    | Default | Description                                                                                                                                                                                                                                                                                   |
| --------------------- | ------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mcp.enabled`         | boolean | `false` | Master permission for local MCP access. When this is `false`, all MCP transports are disabled. Setting it to `true` does not enable stdio by itself; enable a transport such as `mcp.stdio_enabled` separately.                                                                               |
| `mcp.connect_enabled` | boolean | `false` | When set to `true`, trusted MCP clients can list saved SSH/Telnet profiles, open new ExaTerm tabs from those profiles, list Serial ports, and open Serial consoles. SSH passwords and encrypted key passphrases are entered in the ExaTerm UI and are not exposed to the MCP client or saved. |
| `mcp.stdio_enabled`   | boolean | `false` | When set to `true` with `mcp.enabled=true`, the `exaterm-mcp` stdio proxy can be used by local MCP clients. The proxy forwards tool calls to the running ExaTerm GUI through a current-user local control plane.                                                                              |

The HTTP MCP transport has been removed. Existing HTTP-only `mcp.host` and `mcp.port` values in older config files are ignored and will be omitted the next time ExaTerm saves the settings. Users who previously used HTTP MCP must manually enable `mcp.enabled=true` and `mcp.stdio_enabled=true`, then configure their MCP client to launch `exaterm-mcp`.

### MCP Tools

When MCP is enabled, external clients can call these tools:

- `list_terminal_sessions`: lists ExaTerm terminal sessions opened by the user.
- `read_terminal_output`: reads recent output from a session. The result includes a `cursor` that can be used for the next delta read.
- `read_terminal_output_delta`: reads only the output written after the specified `cursor`.
- `wait_terminal_output`: waits until new output is written or a specified substring appears.
- `send_terminal_input`: sends text to a connected session.
- `run_terminal_command`: sends a command to a connected session, waits for output, and returns the output delta.
- `start_terminal_log`: starts a manual plaintext log for a connected session. The log is saved under `%AppData%\ExaTerm\logs`, and the result returns the created file path.
- `stop_terminal_log`: stops a manual plaintext log for a session after ExaTerm flushes pending displayed output to the log.

When `mcp.connect_enabled` is also `true`, external clients can call these additional tools:

- `list_connection_profiles`: lists saved SSH/Telnet profiles. Private key paths and credentials are not returned.
- `connect_saved_profile`: opens a new SSH/Telnet session from a saved profile and shows it as a tab in ExaTerm. SSH passwords and encrypted key passphrases are requested in the ExaTerm UI, not accepted from the MCP tool arguments.
- `list_serial_ports`: lists currently available Serial ports.
- `connect_serial_console`: opens a new Serial console session from explicit port and line settings and shows it as a tab in ExaTerm. The port name must match an available Serial port exactly.

Output-reading tools return `start_cursor`, `cursor`, and `truncated`. Pass `cursor` to `read_terminal_output_delta` or `wait_terminal_output` to continue reading from the same point. If older output has been trimmed from the internal buffer, `truncated` is `true`.

`wait_terminal_output` and `run_terminal_command` wait for up to 60 seconds. `run_terminal_command` targets only existing connected sessions; use `connect_saved_profile` for the opt-in saved-profile connection flow.

The MCP server does not read saved credentials, expose API keys, or read log files directly. MCP clients can explicitly start and stop session logs, but they receive only the log state and file path, not the log contents. New MCP-created SSH/Telnet connections are limited to saved profiles, Serial connections require an explicit available port name, and all new MCP-created connections require `mcp.connect_enabled=true`. SSH connections still enforce the existing known-host checks. If an SSH profile uses `jump_profile_id`, MCP-created connections use the same one-hop jump host flow and request any required jump-host credential in the ExaTerm UI. Terminal output and log files can contain sensitive information, so enable MCP and MCP-triggered logging only for trusted local clients.

## terminal

| Parameter                     | Type    | Default                                | Description                                                                                                                                                  |
| ----------------------------- | ------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `terminal.font_size`          | number  | `14`                                   | Terminal font size. The Settings screen allows values from `8` to `32`.                                                                                      |
| `terminal.font_family`        | string  | `"Consolas, 'Courier New', monospace"` | Terminal font family. Use the same format as CSS `font-family`.                                                                                              |
| `terminal.cursor_style`       | string  | `"block"`                              | Terminal cursor shape. The default is a block cursor. When editing manually, use a value accepted by xterm.js, such as `"block"`, `"underline"`, or `"bar"`. |
| `terminal.scrollback`         | number  | `10000`                                | Number of terminal scrollback lines. Larger values keep more history but may increase memory usage.                                                          |
| `terminal.auto_session_log`   | boolean | `false`                                | When set to `true`, SSH, serial, and Telnet terminal input/output is saved as plaintext logs.                                                                |
| `terminal.log_format`         | string  | `"display"`                            | Session log formatting mode. `"display"` saves text closer to the terminal screen; `"strip_controls"` removes control sequences.                             |
| `terminal.include_log_header` | boolean | `true`                                 | When set to `true`, new session log files start with an ExaTerm header containing the connection type, target, log mode, and start time.                     |

### Session Log Notice

When `terminal.auto_session_log` is set to `true`, text displayed in the terminal and typed into the terminal is saved to log files. Logs may contain sensitive information such as:

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

When `terminal.include_log_header` is `false`, new auto and manual session logs start directly with terminal content instead of the ExaTerm header. Existing log files are not changed.

## ssh

| Parameter                     | Type    | Default | Description                                                                                                              |
| ----------------------------- | ------- | ------- | ------------------------------------------------------------------------------------------------------------------------ |
| `ssh.allow_legacy_algorithms` | boolean | `false` | When set to `true`, ExaTerm also offers legacy SSH algorithms for older devices that do not support modern SSH defaults. |

### Legacy SSH Algorithm Notice

Keep `ssh.allow_legacy_algorithms` set to `false` unless you need to connect to an older SSH server or network device. Enabling it allows weaker compatibility algorithms such as SHA-1 based key exchange, CBC/3DES ciphers, and `ssh-rsa` host keys.

### Available SSH Algorithms

ExaTerm offers the following SSH algorithms. Legacy add-ons are offered only when `ssh.allow_legacy_algorithms` is set to `true`.

| Category     | Default algorithms                                                                                                                           | Legacy add-ons                                                                                     |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Key exchange | `curve25519-sha256`, `curve25519-sha256@libssh.org`, `diffie-hellman-group16-sha512`, `diffie-hellman-group14-sha256`                        | `diffie-hellman-group1-sha1`, `diffie-hellman-group14-sha1`                                        |
| Cipher       | `chacha20-poly1305@openssh.com`, `aes256-gcm@openssh.com`, `aes256-ctr`, `aes192-ctr`, `aes128-ctr`                                          | `aes128-cbc`, `aes192-cbc`, `aes256-cbc`, `3des-cbc`                                               |
| MAC          | `hmac-sha2-512-etm@openssh.com`, `hmac-sha2-256-etm@openssh.com`, `hmac-sha2-512`, `hmac-sha2-256`, `hmac-sha1-etm@openssh.com`, `hmac-sha1` | No additional MAC algorithms are added; the current default already includes `hmac-sha1` variants. |
| Host key     | `ssh-ed25519`, `ecdsa-sha2-nistp256`, `ecdsa-sha2-nistp521`, `rsa-sha2-256`, `rsa-sha2-512`                                                  | `ssh-rsa`                                                                                          |

Internal SSH extension markers, such as strict key exchange and extension info markers, are intentionally omitted from this list because users do not configure them directly.

## saved_connections

`saved_connections` is an array of saved SSH and Telnet connection profiles. Profiles can be managed from the connection dialog. Serial profiles are not currently supported. Passwords, private key contents, key passphrases, and other credentials are not stored in this section. Profile memos are stored as plaintext and may be returned by `list_connection_profiles` when MCP profile connections are enabled, so do not put secrets in them.

| Parameter          | Type           | Description                                                                                                                                            |
| ------------------ | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`               | string         | Profile name and identifier.                                                                                                                           |
| `connection_type`  | string         | Connection type. Supported profile values are `"ssh"` and `"telnet"`.                                                                                  |
| `host`             | string or null | SSH or Telnet target host.                                                                                                                             |
| `port`             | number or null | SSH or Telnet target port.                                                                                                                             |
| `username`         | string or null | SSH username. Not used for Telnet profiles.                                                                                                            |
| `encoding`         | string or null | Initial terminal display encoding for the profile. Supported values are `"utf-8"`, `"shift-jis"`, and `"euc-jp"`. Missing values default to `"utf-8"`. |
| `terminal_mode`    | string or null | Initial terminal mode for the profile. Supported values are `"general"` and `"cisco_ios"`. Missing values default to `"general"`.                      |
| `auth_method`      | string or null | SSH authentication method. Supported values are `"password"` and `"public_key"`. Missing values default to `"password"`. Not used for Telnet profiles. |
| `private_key_path` | string or null | Private key file path used with SSH `"public_key"` authentication, such as an `id_ed25519` file. The file contents and passphrase are not stored.      |
| `jump_profile_id`  | string or null | SSH jump host profile ID. The referenced profile must be a saved SSH profile. Only one jump host is supported; nested jump hosts are rejected.         |
| `memo`             | string or null | Optional plaintext profile memo, such as the device model, role, or operational notes. Non-empty memos may be returned by MCP profile listing.         |

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

### Allow Legacy SSH Algorithms

```json
"ssh": {
  "allow_legacy_algorithms": true
}
```

Use this only for older devices that cannot negotiate with the default SSH algorithms.

## Troubleshooting

| Symptom                              | Action                                                                                                                                                                                                                                                                                   |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ExaTerm cannot load settings         | Check the JSON syntax, especially extra commas, quotation marks, and braces.                                                                                                                                                                                                             |
| Changes are not reflected            | Restart ExaTerm or save the settings again from the Settings screen.                                                                                                                                                                                                                     |
| An AI provider does not appear       | Cloud providers require API keys. For Azure OpenAI, also check `azure_openai_enabled`, `azure_openai_endpoint`, and `azure_openai_deployment`. For OpenRouter, save the OpenRouter API key from Settings. For Ollama, check `ollama_enabled` and make sure the Ollama server is running. |
| An older SSH device will not connect | If the error indicates no common SSH algorithm, set `ssh.allow_legacy_algorithms` to `true`, then try connecting again. Disable it again when it is no longer needed.                                                                                                                    |
| Text is hard to read                 | Adjust `terminal.font_size` or `terminal.font_family`.                                                                                                                                                                                                                                   |
| You do not want logs to be saved     | Set `terminal.auto_session_log` to `false`. If logs were already created, delete them from `%AppData%\ExaTerm\logs` as needed.                                                                                                                                                           |
