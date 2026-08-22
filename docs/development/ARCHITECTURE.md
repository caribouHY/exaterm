# ExaTerm Architecture

This document describes the current runtime architecture and durable ownership boundaries of ExaTerm. Update it when a change alters responsibilities across the React frontend, Rust backend, workspace model, protocol sessions, logging, or external-control interfaces.

## System Shape

ExaTerm is a Windows-focused Tauri v2 desktop application.

- `src/` contains the React and TypeScript frontend.
- `src-tauri/src/` contains the Rust backend and Tauri commands.
- `src-tauri/src/lib.rs` creates shared backend state and registers commands exposed through `invoke`.
- `src/types/index.ts` contains frontend types that mirror selected backend payloads.
- `src/locales/en.json` and `src/locales/ja.json` contain user-visible UI text.

Windows is the primary beta target. Cross-platform changes must preserve the Windows experience and security model.

## Design Principles

- Treat terminal sessions as long-lived runtime resources. Settings changes and ordinary UI updates must not recreate or disconnect them.
- Treat terminal buffers, scrollback, logs, connection targets, usernames, prompts, command output, and API keys as sensitive.
- Keep network and device sessions in Rust. Keep presentation state in React unless cross-window or external-client coordination requires backend ownership.
- Keep configuration structures in `src-tauri/src/config.rs` synchronized with their mirrored TypeScript types in `src/types/index.ts`.
- Update both locale files when user-visible frontend text changes.
- Implement Tauri commands in the responsible backend module and register them in `src-tauri/src/lib.rs`.

## Frontend Runtime

`src/App.tsx` composes the application shell, terminal views, utility views, AI panel, configuration refresh, and UI prompts.

Major frontend areas are:

- `src/features/workspace-tabs/`: window-local workspace projection, revision filtering, terminal and utility tab ordering, workspace event subscriptions, tab lifecycle, and cross-window movement requests.
- `src/components/Terminal/`: xterm.js rendering, terminal input and output, resize handling, encoding support, scrollback restoration, and frontend log-buffer flushing.
- `src/components/Connection/`: SSH, Serial, and Telnet forms, saved profiles, connection history, SSH host-key confirmation, and session creation.
- `src/components/Settings/`: configuration editing and AI API-key save or clear flows.
- `src/components/AI/`: provider and model selection and AI chat presentation.
- `src/components/Log/`: session-log listing.
- `src/components/TitleBar/`, `src/components/Terminal/TerminalTabs`, and `src/components/StatusBar/`: application navigation and terminal controls.

Terminal views may remount when a tab moves between windows, but a move must not disconnect or recreate the backend session. The destination restores bounded recent output from the backend and resumes live output handling.

## Backend Runtime

Backend state is created in `src-tauri/src/lib.rs` and managed through Tauri `State` values.

- `ssh.rs`, `serial.rs`, and `telnet.rs` own active protocol sessions, writes, resize behavior where supported, and disconnect handling.
- `ssh_known_hosts.rs` owns the ExaTerm known-hosts file and fingerprint trust decisions.
- `terminal_control.rs` retains decoded output and session status for external control and cross-window snapshot recovery.
- `logger.rs` owns at most one active plaintext log per session and distinguishes automatic and manual logging behavior.
- `config.rs` loads, defaults, migrates, and saves user configuration.
- `ai.rs` and `ai/` own provider catalogs, provider calls, error mapping, and secret lookup.
- `command_error.rs` defines stable structured Tauri command errors. React localizes known GUI errors through `src/features/backend-errors/`.

The backend does not retain GUI language state. External-control, MCP, and terminal CLI errors remain English and machine-readable.

## Workspace and Window Ownership

The Rust workspace subsystem uses `src-tauri/src/workspace.rs` as its facade:

- `workspace/model.rs` owns terminal-tab placement, order, active terminal tabs, window focus history, drag state, and invariants.
- `workspace/state.rs` exposes the async shared-state API.
- `workspace/commands.rs` owns Tauri commands, window creation, localization-independent command results, and workspace events.

Each terminal tab has exactly one owner window. Closing a window rehomes its tabs when another window remains. Moving or detaching a tab changes visible ownership without disconnecting the underlying protocol session or replacing its session ID.

Settings and Logs are window-local utility views. Rust owns terminal placement; React combines the backend terminal projection with local utility-tab state.

## Terminal Session Flow

1. A connection dialog calls the matching backend connection command.
2. Rust establishes the SSH, Serial, or Telnet session and registers its output state.
3. The frontend workspace lifecycle registers the returned session as a terminal tab.
4. `TerminalView` subscribes to protocol output and writes it to xterm.js.
5. User input is sent to the matching backend write command.
6. Disconnect state is projected through the workspace without discarding the visible terminal buffer.

GUI SSH connections verify and, when necessary, confirm the host key within the active handshake so authentication continues on the same TCP connection. External-control SSH connections require an already trusted key because they cannot open a GUI host-key prompt in that flow.

## Logging

Logging is opt-in. A terminal session has at most one active log, regardless of whether it was started automatically or manually.

- Automatic logging continues independently of manual log pause state.
- Manual pause and resume controls only manual logging behavior.
- Frontend sanitizer buffers must be flushed before operations that require all rendered output to be persisted.
- Moving a tab preserves the backend log state and does not stop logging.
- MCP and the terminal CLI can control an allowed session log but cannot read log files directly.

Plaintext logs can contain commands, output, prompts, hostnames, usernames, device data, and accidental secrets. Changes to capture, storage, or control behavior must preserve the existing privacy boundary.

## External Control, MCP, and CLI

`src-tauri/src/external_control/` owns the transport-neutral terminal-operation service, local control protocol, GUI discovery, and connection permissions used by external clients.

- The normal ExaTerm GUI process remains the single owner of sessions, logs, credentials, and UI prompts.
- `exaterm-cli` exposes typed subcommands and JSON output for local automation.
- `exaterm-mcp` is a bundled stdio MCP proxy. It discovers or launches the GUI and forwards tool calls over the current-user local control plane.
- Windows uses a current-user named pipe and protocol handshake. The non-Windows fallback uses a local TCP transport.
- HTTP MCP has been removed and is not a compatibility target.

External control requires `external_control.enabled`. The CLI and MCP compatibility adapter additionally require their respective `cli_enabled` or `mcp_enabled` flags. Creating new connections also requires `connect_enabled`, and saved profiles must individually allow external-control access.

The local control plane rejects invalid protocol versions and requests without the negotiated nonce. MCP stdout is reserved for JSON-RPC; diagnostics belong on stderr or in privacy-safe logs.

See [ADR 0001](decisions/0001-local-external-control-and-mcp-stdio.md) for the durable transport and ownership decision.

## Data and Storage

Windows user data is stored under the ExaTerm application-data directory:

- `%AppData%/ExaTerm/config.json`: user settings.
- `%AppData%/ExaTerm/connection_history.json`: recent successful GUI SSH and Telnet settings without passwords or passphrases.
- `%AppData%/ExaTerm/logs/`: optional plaintext terminal logs.
- `%AppData%/ExaTerm/logs/index.json`: log-session index.
- `%AppData%/ExaTerm/known_hosts`: SSH trust entries.
- Operating-system credential store: cloud AI API keys.

Do not move cloud API keys into `config.json`, logs, terminal buffers, or external-control responses.

## Documentation and Agent Workflows

- `docs/` contains human-facing user and contributor documentation.
- This file records current architecture, not an implementation plan or task checklist.
- `CSS_ARCHITECTURE.md` records styling ownership and the contracts for the staged CSS migration.
- Durable decisions that benefit from rationale belong in `docs/development/decisions/`.
- Repository-local agent procedures belong in `.agents/skills/`.
- Always-on constraints and routing belong in `AGENTS.md`.

Use `.agents/skills/exaterm-validate-change/SKILL.md` to select validation commands. Use `.agents/skills/exaterm-ui-change/SKILL.md` for React, CSS, layout, or other visual changes. Use `.agents/skills/exaterm-release-prep/SKILL.md` for release preparation.
