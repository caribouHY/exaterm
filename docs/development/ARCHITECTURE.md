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

### GUI Connection Attempts

The connection-attempt controller owns the authoritative state of one GUI connection attempt. It applies transitions synchronously and publishes a read-only snapshot to React. The dialog owns editable form values and renders the snapshot; it does not maintain a second attempt reducer or execution flags. Connection inputs are captured when an attempt starts.

The controller issues request IDs and coordinates preparation, credential submission, cancellation, and session registration. Each pre-connection credential prompt also has its own ID, and submission consumes that prompt synchronously before asynchronous work starts. Secret values are excluded from the attempt snapshot and reducer; they remain in the credential input and short-lived execution state.

SSH credential preparation and protocol command construction remain separate from common attempt orchestration. Diagnostics subscribe using the controller's request ID and reject callbacks from obsolete subscriptions. Handshake authentication and host-key confirmation remain owned by Rust and the application-level SSH prompt queue.

Backend connection success starts finalization. A late cancellation response cannot reverse that transition. Logging starts at most once for the new session, terminal registration can retry using that same session, and history is recorded only after successful registration. A logging or history failure does not discard a successful connection. Existing sessions, terminal buffers, and logs are not reset by attempt transitions.

When the dialog is disposed, pending preparation is abandoned and an in-flight connection is cancelled where possible. A late successful connection that has not entered terminal registration is released individually. If registration is already pending, the controller waits for its result: successful registration transfers ownership to the workspace, while failure releases the attempt's session after disposal. Normal dialog closure caused by successful registration does not disconnect that session.

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

The desktop application runs as a single GUI process. A later `exaterm.exe` invocation forwards its arguments to the existing process, which restores and focuses the most recently focused workspace window. SSH and Telnet startup requests are retained in a backend FIFO until that window can process them; an active connection dialog is never replaced by a later request.

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

GUI SSH connections verify and, when necessary, confirm the host key within the active handshake so authentication continues on the same TCP connection. Saved-profile external-control SSH connections require an already trusted key. Direct external-control SSH connections may confirm an unknown key in the preferred GUI window, but reject a mismatch with an existing known-hosts entry.

## Logging

Logging is opt-in. A terminal session has at most one active log, regardless of whether it was started automatically or manually.

- Automatic and manually started logs share the same active, paused, and stopped lifecycle.
- The backend logger is authoritative for whether a log is active; workspace pause metadata never makes an inactive logger appear active.
- Frontend sanitizer buffers must be flushed before operations that require all rendered output to be persisted.
- Moving a tab preserves the backend log state and does not stop logging.
- MCP and the terminal CLI can control an allowed session log but cannot read log files directly.

`src/features/terminal-logging/` owns session-scoped manual-log operation coordination for GUI and external-control entry points. It resolves the current owner tab, serializes start, stop, pause, and resume per session, flushes frontend buffers before pause or stop, invokes the logger backend, and then updates workspace metadata. GUI entry points retain save-dialog and status-message behavior; external-control entry points retain request acknowledgements and wait for changed metadata to reach the UI before replying. External log-control events are delivered only to the session's current owner window.

Plaintext logs can contain commands, output, prompts, hostnames, usernames, device data, and accidental secrets. Changes to capture, storage, or control behavior must preserve the existing privacy boundary.

## External Control, MCP, and CLI

`src-tauri/src/external_control/` owns the transport-neutral terminal-operation service, local control protocol, GUI discovery, and connection permissions used by external clients.

The service keeps each operation result as a concrete Rust struct or state-specific enum through permission checks, connection preparation, terminal operations, and response assembly. Serialization to dynamic JSON occurs only in the CLI and MCP adapter boundary; the local control protocol serializes the typed response envelope directly.

Configuration and port discovery, protocol connection and writes, GUI credential/log requests and workspace notifications, and logger access are narrow runtime I/O boundaries. Production adapters call the existing Tauri, protocol, configuration, workspace, credential, host-key, and logger APIs. Tests replace only those boundary effects, while permission evaluation, credential policy, host-key mode selection, connection finalization, workspace registration, and result construction follow the production service path.

- The normal ExaTerm GUI process remains the single owner of sessions, logs, credentials, and UI prompts.
- `exaterm-cli` exposes typed subcommands and JSON output for local automation.
- `exaterm-mcp` is a bundled stdio MCP proxy. It discovers or launches the GUI and forwards tool calls over the current-user local control plane.
- Windows uses a current-user named pipe and protocol handshake. The non-Windows fallback uses a local TCP transport.
- External clients serialize GUI startup with a current-user launch lock and recheck the control plane after acquiring it. The Windows named-pipe listener retries transient instance-creation failures instead of permanently stopping the control plane.
- HTTP MCP has been removed and is not a compatibility target.

External control requires `external_control.enabled`. The CLI and MCP compatibility adapter additionally require their respective `cli_enabled` or `mcp_enabled` flags. Creating new connections also requires `connect_enabled`, and saved profiles must individually allow external-control access. Direct SSH/Telnet targets additionally require `direct_connect_enabled`; a saved SSH profile used as a direct connection's jump host must also allow external control.

The local control protocol is version 5. It carries typed session disconnect and log status, pause, resume, start, and stop operations. CLI start requests may include an absolute destination with overwrite or append mode, while the MCP start schema remains session-ID-only. The local control plane rejects invalid protocol versions and requests without the negotiated nonce. MCP stdout is reserved for JSON-RPC; diagnostics belong on stderr or in privacy-safe logs.

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
