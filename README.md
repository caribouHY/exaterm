# ExaTerm

ExaTerm is a Windows terminal app for SSH, Telnet, and serial communication with AI assistant and MCP integration.

[日本語版 README](README.ja.md)

![window image](docs/images/window.png)

## Features

- SSH, Telnet, and serial communication
- MCP server feature for external AI client integration
- AI assistant support for OpenAI, Gemini, Anthropic, OpenRouter, Ollama, and Azure OpenAI
- Session logging with manual and automatic modes
- Network-device color highlighting with Cisco IOS support

## Supported OS

ExaTerm currently supports Windows only.

macOS and Linux are not supported release targets at this time.

## Installation

1. Open the ExaTerm Releases page.
2. Download the exe installer for the version you want to install.
3. Run the exe installer.
4. Launch ExaTerm from the Start menu or the installed application shortcut.

ExaTerm does not include automatic updates. To update, download and run a newer exe installer from the Releases page.

## First Launch

On first launch, ExaTerm creates its settings directory under:

```text
%AppData%\ExaTerm
```

The default configuration does not create terminal session logs. Session logging starts only after you enable Auto Session Log in Settings.

## Privacy and Local Storage

ExaTerm stores user data locally on Windows:

| Data                  | Location                          |
| --------------------- | --------------------------------- |
| Settings              | `%AppData%\ExaTerm\config.json`   |
| Optional session logs | `%AppData%\ExaTerm\logs`          |
| SSH known hosts       | `%AppData%\ExaTerm\known_hosts`   |
| Cloud AI API keys     | Operating system credential store |

API keys for cloud AI providers are not stored in `config.json`. They are saved in the operating system credential store.

## Session Logs

Session logging is off by default. New installs do not create terminal session logs unless you explicitly enable Auto Session Log in Settings.

When Auto Session Log is enabled, ExaTerm records SSH, Telnet, and serial terminal input and output as plaintext log files. These logs can include commands, command output, prompts, hostnames, usernames, device output, and other sensitive terminal content.

Logs are stored under:

```text
%AppData%\ExaTerm\logs
```

The same location is shown in the Logs view. To remove saved logs, close ExaTerm and delete the files in that folder.

## AI Assistant

The AI assistant can use terminal context from the active tab to help explain output and suggest commands. Review any suggested command before running it in a terminal session.

OpenAI, Azure OpenAI, Anthropic, Gemini, and OpenRouter require provider API keys or endpoint settings. Save or clear provider credentials from Settings.

Ollama usually does not require an API key, but it does require a running Ollama server that ExaTerm can reach.

## MCP Integration

ExaTerm can run an opt-in local MCP server for external AI clients. MCP access is disabled unless you enable it in Settings.

When enabled, MCP clients can interact with visible ExaTerm terminal sessions, including reading recent output, sending input, waiting for output, and running commands with captured results. MCP clients can also open saved SSH/Telnet profiles or serial consoles as visible ExaTerm tabs. SSH credentials are entered only in the ExaTerm UI.

Only enable MCP access for clients you trust. Terminal output, commands, prompts, hostnames, usernames, and device output can be sensitive.

## Terminal CLI

`exaterm-cli.exe` provides JSON-based terminal control for local scripts and AI agents
without requiring an MCP client. See the [Terminal CLI guide](docs/CLI_GUIDE.en.md).

## Common Recovery Steps

If ExaTerm does not launch, reinstall it with the latest exe installer and try launching it again.

If settings appear broken, close ExaTerm and inspect:

```text
%AppData%\ExaTerm\config.json
```

You can rename or remove `config.json` to let ExaTerm recreate default settings on the next launch.

If AI requests fail, check that the selected provider is available, the API key is saved in Settings, and your network can reach the provider. For Ollama, confirm that the Ollama server is running and that the configured base URL is correct.

If session logs are missing, confirm that Auto Session Log is enabled before starting a new SSH, Telnet, or serial session. ExaTerm does not retroactively create logs for sessions that started while logging was disabled.

For manual configuration details, see:

- [Config guide](docs/CONFIG_JSON_GUIDE.en.md)
- [Terminal CLI guide](docs/CLI_GUIDE.en.md)

## Developer Setup

Developer setup requires Rust and Node.js.

1. Clone the repository.
2. Install pnpm:

```powershell
npm install -g pnpm@10.33.2
```

3. Install dependencies:

```powershell
pnpm install
```

4. Start the Tauri development app:

```powershell
pnpm run tauri dev
```

5. Build the frontend:

```powershell
pnpm run build
```

6. Build the Windows exe package:

```powershell
pnpm run tauri build
```

## License

ExaTerm is released under the MIT License. See [LICENSE](LICENSE).
