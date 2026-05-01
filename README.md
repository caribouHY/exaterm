# ExaTerm

ExaTerm is a terminal for SSH, Telnet, and serial communication with an AI assistant.

## Features

- SSH terminal sessions with multiple tabs
- Telnet terminal sessions with multiple tabs
- Serial terminal sessions with multiple tabs
- AI assistant support for OpenAI, Anthropic, Gemini, and Ollama
- Optional plaintext session logging
- User settings stored under the Windows application data folder

## Supported OS

ExaTerm is currently distributed for Windows.

macOS and Linux are not supported release targets at this time.

## Installation

1. Open the ExaTerm Releases page.
2. Download the MSI installer for the version you want to install.
3. Run the MSI installer.
4. Launch ExaTerm from the Start menu or the installed application shortcut.

ExaTerm does not include automatic updates. To update, download and run a newer MSI installer from the Releases page.

## First Launch

On first launch, ExaTerm creates its settings directory under:

```text
%AppData%\ExaTerm
```

The default configuration does not create terminal session logs. Session logging starts only after you enable Auto Session Log in Settings.

## Privacy and Local Storage

ExaTerm stores user data locally on Windows:

| Data | Location |
| --- | --- |
| Settings | `%AppData%\ExaTerm\config.json` |
| Optional session logs | `%AppData%\ExaTerm\logs` |
| SSH known hosts | `%AppData%\ExaTerm\known_hosts` |
| Cloud AI API keys | Operating system credential store |

API keys for cloud AI providers are not stored in `config.json`. They are saved in the operating system credential store.

## Session Logs

Session logging is off by default. New installs do not create terminal session logs unless you explicitly enable Auto Session Log in Settings.

When Auto Session Log is enabled, ExaTerm records SSH, Telnet, and serial terminal input and output as plaintext log files. These logs can include commands, command output, prompts, hostnames, usernames, device output, and other sensitive terminal content.

Logs are stored under:

```text
%AppData%\ExaTerm\logs
```

The same location is shown in the Logs view. To remove saved logs, close ExaTerm and delete the files in that folder.

## AI API Keys

OpenAI, Anthropic, and Gemini require provider API keys. Save or clear them from Settings.

Ollama usually does not require an API key, but it does require a running Ollama server that ExaTerm can reach.

## Common Recovery Steps

If ExaTerm does not launch, reinstall it with the latest MSI installer and try launching it again.

If settings appear broken, close ExaTerm and inspect:

```text
%AppData%\ExaTerm\config.json
```

You can rename or remove `config.json` to let ExaTerm recreate default settings on the next launch.

If AI requests fail, check that the selected provider is available, the API key is saved in Settings, and your network can reach the provider. For Ollama, confirm that the Ollama server is running and that the configured base URL is correct.

If session logs are missing, confirm that Auto Session Log is enabled before starting a new SSH, Telnet, or serial session. ExaTerm does not retroactively create logs for sessions that started while logging was disabled.

For manual configuration details, see:

- [Config guide](docs/CONFIG_JSON_GUIDE.en.md)

## Developer Setup

Developer setup requires Rust and Node.js.

1. Clone the repository.
2. Install dependencies:

```powershell
npm install
```

3. Start the Tauri development app:

```powershell
npm run tauri -- dev
```

4. Build the frontend:

```powershell
npm run build
```

5. Build the Windows MSI package:

```powershell
npm run tauri -- build
```

## License

ExaTerm is released under the MIT License. See [LICENSE](LICENSE).
