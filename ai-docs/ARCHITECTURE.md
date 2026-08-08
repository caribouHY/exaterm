# ExaTerm Architecture Notes for AI Agents

This document is AI-facing implementation context. Do not treat it as user documentation.
User-facing documentation belongs in `docs/`.

For the durable system-wide design and future direction, also read
`ai-docs/SYSTEM_DESIGN.md`. For the planned multi-window tab move and external MCP design,
read `ai-docs/MULTI_WINDOW_MCP_STDIO_DESIGN.md`.

## Top-Level Shape

ExaTerm is a Tauri v2 app:

- `src/` contains the React + TypeScript frontend.
- `src-tauri/src/` contains the Rust backend and Tauri commands.
- `src-tauri/src/lib.rs` registers all commands exposed to the frontend through `invoke`.
- `src/types/index.ts` defines frontend types that mirror selected backend payloads.
- `src/locales/en.json` and `src/locales/ja.json` provide UI text.

The app is currently planned and documented around a Windows desktop beta.

## Frontend Areas

- `src/App.tsx` composes the main shell, terminal views, utility views, AI panel, config refresh, and UI prompts.
- `src/features/workspace-tabs/` owns the frontend window projection, terminal and utility tab ordering, workspace event subscriptions, tab lifecycle workflows, and cross-window movement requests.
- `src/components/Terminal/` owns xterm.js rendering, terminal input/output, resize handling, encoding selection support, and optional log append calls.
- `src/components/Connection/` owns SSH, Serial, and Telnet connection forms, SSH host-key confirmation, serial port listing, and session start.
- `src/components/Settings/` owns config editing and AI API key save/clear UI.
- `src/components/AI/` owns model/provider selection and chat calls.
- `src/components/Log/` owns session log listing.
- `src/components/TitleBar/`, `Terminal/TerminalTabs`, and `StatusBar/` own navigation and shell controls.

Avoid changes that remount terminal views or drop active tab state unless the task explicitly requires that behavior.

## Backend Areas

- `src-tauri/src/ssh.rs` handles SSH sessions, writes, resizes, disconnects, and host-key probing/trust flow integration.
- `src-tauri/src/ssh_known_hosts.rs` handles the ExaTerm known-hosts file and fingerprint trust decisions.
- `src-tauri/src/serial.rs` handles serial port listing and serial sessions.
- `src-tauri/src/telnet.rs` handles Telnet sessions, minimal option negotiation, writes, resizes, and disconnects.
- `src-tauri/src/ai.rs` and `src-tauri/src/ai/` handle provider catalogs, provider calls, errors, and secret lookup.
- `src-tauri/src/ai/secrets.rs` stores cloud provider API keys in the operating system credential store, not in `config.json`.
- `src-tauri/src/config.rs` loads, defaults, migrates legacy `mcp` settings into
  `external_control`, and saves user settings.
- `src-tauri/src/logger.rs` handles optional plaintext terminal session logs and the log index.
- `src-tauri/src/workspace.rs` is the workspace facade; `src-tauri/src/workspace/model.rs` owns workspace data and invariants, `state.rs` provides the async shared-state API, and `commands.rs` owns Tauri commands, window creation, and workspace events.
- `src-tauri/src/external_control/` owns the typed terminal-operation API, GUI-local control protocol, client discovery, and GUI startup flow used by `exaterm-mcp` and `exaterm-cli`.
- `src-tauri/src/mcp/` adapts the typed external-control API to the existing MCP tool behavior.
- `src-tauri/src/terminal_cli.rs` defines the typed JSON terminal CLI.
- `src-tauri/src/command_error.rs` defines the structured Tauri command error contract. Rust returns stable error codes, interpolation parameters, and English fallback messages; React translates known GUI errors through `src/features/backend-errors/` and i18next.

Backend code does not keep GUI language state. The `language` argument passed to `ai_chat` controls the requested AI response language only; it does not select backend error wording. External control, MCP, and CLI errors remain English and use their existing machine-readable adapters.

When adding a Tauri command, implement it in the responsible backend module and register it in `src-tauri/src/lib.rs`.

## Data and Storage

Windows user data is stored under the ExaTerm application data directory:

- `%AppData%/ExaTerm/config.json` stores user settings.
- `%AppData%/ExaTerm/connection_history.json` stores up to 10 recent successful GUI SSH and Telnet connection settings per protocol without passwords or passphrases.
- `%AppData%/ExaTerm/logs/` stores optional plaintext terminal session logs.
- `%AppData%/ExaTerm/logs/index.json` stores the log session index.
- `%AppData%/ExaTerm/known_hosts` stores SSH host trust entries.
- Cloud AI API keys are stored in the OS credential store.

Session logs can contain commands, command output, prompts, hostnames, usernames, device output, and accidental secrets.
Preserve the opt-in logging model unless the task explicitly changes it.

## Important Data Flow

- Frontend calls Rust through `@tauri-apps/api/core` `invoke`.
- SSH, Serial, and Telnet connection dialogs start backend sessions, then the frontend workspace-tab lifecycle registers tabs from returned session IDs.
- Terminal input writes to the matching backend command (`ssh_write`, `serial_write`, or `telnet_write`); terminal output is rendered by xterm.js and may be appended to logs only when auto session logging is enabled.
- Settings are loaded through `config_load` and saved through `config_save`.
- AI chat loads configured provider/model preferences, checks secret status for cloud providers, and calls `ai_chat`.
- SSH host keys are probed before trust decisions and stored through the known-hosts module.

## Documentation Boundaries

- `docs/CONFIG_JSON_GUIDE.en.md` and `docs/CONFIG_JSON_GUIDE.ja.md` are user-facing config guides.
- Do not depend on or create a root-level `CONFIG_JSON_GUIDE.md` unless a user-facing documentation task explicitly asks for it.
- Keep implementation guidance, architecture notes, and coding checklists in `ai-docs/`.
