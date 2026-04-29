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
    "default_model": "gpt-4o"
  },
  "terminal": {
    "font_size": 14,
    "font_family": "Consolas, 'Courier New', monospace",
    "cursor_style": "block",
    "scrollback": 10000,
    "auto_session_log": false
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
| `terminal`          | object | See below | Terminal display and logging settings.                                                                                                     |
| `saved_connections` | array  | `[]`      | Saved connection information. In the current UI, this is mostly treated as internal data.                                                  |

## ai

| Parameter                    | Type    | Default                    | Description                                                                                                                                                                                                                            |
| ---------------------------- | ------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ai.azure_openai_enabled`    | boolean | `false`                    | When set to `true`, Azure OpenAI is shown in the AI panel when the endpoint, model deployment name, and API key are configured.                                                                                                        |
| `ai.azure_openai_endpoint`   | string  | `""`                       | The full Azure OpenAI chat completions URL to send requests to, such as `"https://your-resource.openai.azure.com/openai/v1/chat/completions"` or a deployment URL with `api-version`. ExaTerm uses this URL as entered.                  |
| `ai.azure_openai_deployment` | string  | `""`                       | The Azure OpenAI model deployment name. With the v1 API, this value is sent as the `model` field.                                                                                                                                      |
| `ai.ollama_enabled`          | boolean | `false`                    | When set to `true`, Ollama models are shown in the AI panel. To use Ollama, a local or configured Ollama server must be running.                                                                                                       |
| `ai.ollama_base_url`         | string  | `"http://localhost:11434"` | The base URL for the Ollama API. For a standard local setup, use `"http://localhost:11434"`. If this is an empty string, the UI treats it as the default URL.                                                                          |
| `ai.default_provider`        | string  | `"OpenAi"`                 | The provider selected by default in the AI panel. Supported values are `"OpenAi"`, `"AzureOpenAi"`, `"Anthropic"`, `"Gemini"`, and `"Ollama"`.                                                                                         |
| `ai.default_model`           | string  | `"gpt-4o"`                 | The model ID selected by default in the AI panel. This is not currently editable from the Settings screen, so edit it manually if needed. If the saved model is not available, ExaTerm automatically falls back to an available model. |

### AI API Keys

API keys for OpenAI, Azure OpenAI, Anthropic, and Google Gemini are not stored in `config.json`. Keys registered from the Settings screen are stored in the operating system credential store.

Enter the full Azure OpenAI chat completions URL and model deployment name. ExaTerm uses the endpoint URL exactly as entered and does not append a path or `api-version` value.

Ollama usually does not require an API key. Configure `ai.ollama_enabled` and `ai.ollama_base_url` instead.

### Common Model IDs

| Provider     | Example model IDs                                       |
| ------------ | ------------------------------------------------------- |
| OpenAI       | `gpt-4o`, `gpt-4o-mini`                                 |
| Azure OpenAI | Your Azure model deployment name, such as `my-gpt4o`    |
| Anthropic    | `claude-sonnet-4-20250514`, `claude-3-5-haiku-20241022` |
| Gemini       | `gemini-2.5-pro`, `gemini-2.5-flash`                    |
| Ollama       | Model names installed in your local Ollama instance     |

## terminal

| Parameter                   | Type    | Default                                | Description                                                                                                                                                  |
| --------------------------- | ------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `terminal.font_size`        | number  | `14`                                   | Terminal font size. The Settings screen allows values from `8` to `32`.                                                                                      |
| `terminal.font_family`      | string  | `"Consolas, 'Courier New', monospace"` | Terminal font family. Use the same format as CSS `font-family`.                                                                                              |
| `terminal.cursor_style`     | string  | `"block"`                              | Terminal cursor shape. The default is a block cursor. When editing manually, use a value accepted by xterm.js, such as `"block"`, `"underline"`, or `"bar"`. |
| `terminal.scrollback`       | number  | `10000`                                | Number of terminal scrollback lines. Larger values keep more history but may increase memory usage.                                                          |
| `terminal.auto_session_log` | boolean | `false`                                | When set to `true`, SSH and serial terminal input/output is saved as plaintext logs.                                                                         |

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

## saved_connections

`saved_connections` is an array of saved connection entries. Each entry can contain the following fields.

| Parameter         | Type           | Description                                     |
| ----------------- | -------------- | ----------------------------------------------- |
| `id`              | string         | Identifier for the saved connection.            |
| `name`            | string         | Display name for the connection.                |
| `connection_type` | string         | Connection type. Usually `"ssh"` or `"serial"`. |
| `host`            | string or null | SSH target host.                                |
| `port`            | number or null | SSH target port.                                |
| `username`        | string or null | SSH username.                                   |
| `serial_port`     | string or null | Serial port name.                               |
| `baud_rate`       | number or null | Serial baud rate.                               |

Example:

```json
{
  "id": "dev-server",
  "name": "Development Server",
  "connection_type": "ssh",
  "host": "192.168.1.10",
  "port": 22,
  "username": "admin",
  "serial_port": null,
  "baud_rate": null
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
  "default_model": "llama3.1"
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
  "default_model": "my-gpt4o"
}
```

Save the Azure OpenAI API key from the Settings screen. ExaTerm sends requests to the configured endpoint URL without modifying it.

### Disable Session Logs

```json
"terminal": {
  "auto_session_log": false
}
```

Keep the other fields in the actual `terminal` object. The snippet above only shows the field being changed.

## Troubleshooting

| Symptom                          | Action                                                                                                                                                                                                                        |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ExaTerm cannot load settings     | Check the JSON syntax, especially extra commas, quotation marks, and braces.                                                                                                                                                  |
| Changes are not reflected        | Restart ExaTerm or save the settings again from the Settings screen.                                                                                                                                                          |
| An AI provider does not appear   | Cloud providers require API keys. For Azure OpenAI, also check `azure_openai_enabled`, `azure_openai_endpoint`, and `azure_openai_deployment`. For Ollama, check `ollama_enabled` and make sure the Ollama server is running. |
| Text is hard to read             | Adjust `terminal.font_size` or `terminal.font_family`.                                                                                                                                                                        |
| You do not want logs to be saved | Set `terminal.auto_session_log` to `false`. If logs were already created, delete them from `%AppData%\ExaTerm\logs` as needed.                                                                                                |
