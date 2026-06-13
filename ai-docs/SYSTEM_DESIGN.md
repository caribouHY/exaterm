# ExaTerm System Design

This document is AI-facing implementation context. User-facing documentation belongs in
`docs/`.

## Purpose

Use this file as the durable entry point for ExaTerm design decisions. It should describe
the current shape of the application, the intended direction for cross-cutting features,
and the boundaries that future agents must preserve.

Keep short operational notes in `ai-docs/ARCHITECTURE.md`. Put feature-specific designs in
separate `ai-docs/*_DESIGN.md` files and link them from this document when they affect the
overall architecture.

## Design Principles

- Windows is the primary beta target. Cross-platform choices are welcome when they do not
  weaken the Windows experience.
- Terminal sessions, scrollback, session logs, prompts, connection targets, usernames, and
  API keys are sensitive data.
- Terminal sessions are long-lived runtime resources. UI updates, settings changes, and
  window management must not clear or recreate sessions unless the user explicitly closes
  or disconnects them.
- Rust owns device and network sessions. React owns presentation state unless a feature
  needs cross-window or external-client coordination.
- Config structs in `src-tauri/src/config.rs` and mirrored TypeScript types in
  `src/types/index.ts` must stay synchronized.
- User-facing text changes require both `src/locales/en.json` and
  `src/locales/ja.json`.
- AI-facing implementation guidance stays in `ai-docs/` or `AGENTS.md`; user-facing
  guides stay in `docs/`.

## Current Architecture

ExaTerm is a Tauri v2 desktop application.

- Frontend: React and TypeScript under `src/`.
- Backend: Rust modules and Tauri commands under `src-tauri/src/`.
- Command boundary: frontend calls Rust through `@tauri-apps/api/core` `invoke`.
- Event boundary: backend emits protocol and MCP events through Tauri events.
- Data storage: Windows app data directory, with secrets stored in the OS credential store.

### Frontend Runtime

`src/App.tsx` currently owns the main UI shell:

- terminal tab list, tab order, active tab, and close state
- utility tabs for Settings and Logs
- AI panel visibility, width, messages, provider, and selected model
- config refresh and startup CLI request handling
- MCP credential and log-control prompts

Terminal rendering lives under `src/components/Terminal/`.
`TerminalView` owns the xterm.js instance, backend output listeners, resize handling,
encoding selection, and frontend-side log flush behavior.

Connection creation lives under `src/components/Connection/`.
Settings, AI, Logs, TitleBar, and StatusBar are separate UI areas.

### Backend Runtime

Backend state is managed through Tauri `State` values created in `src-tauri/src/lib.rs`:

- `SshState`, `SerialState`, and `TelnetState` own active protocol sessions.
- `TerminalControlState` stores a readable decoded output buffer and status per session.
- `LoggerState` owns automatic and manual plaintext session log state.
- `McpCredentialState` and `McpLogControlState` bridge MCP requests that need UI action.
- `StartupCliState` carries one startup CLI request into the frontend.

The protocol modules start sessions, write input, resize where supported, disconnect, and
mark `TerminalControlState` as disconnected. Session IDs are UUID-like strings that join
frontend tabs, protocol sessions, terminal output buffers, logger state, and MCP calls.

### MCP Runtime

`src-tauri/src/mcp.rs` wires the MCP module tree. The current external entry point is the
`exaterm-mcp` stdio proxy, implemented in `src-tauri/src/bin/exaterm-mcp.rs` and
`src-tauri/src/mcp/stdio.rs`. The proxy is launched by local MCP clients, starts or
discovers the normal ExaTerm GUI process, and forwards tool calls through the GUI-local
control plane.

The GUI process owns the in-process MCP backend state. When `mcp.enabled` is true, startup
spawns the local control plane; the stdio proxy additionally requires
`mcp.stdio_enabled=true` before it serves MCP over stdio. Windows proxy launch
coordination uses a current-user named mutex so an abnormal proxy exit cannot leave a stale
lock file that blocks later GUI startup attempts.

Current MCP tools include:

- session listing and terminal output reads
- output delta and wait operations
- terminal input and command execution helpers
- manual log start and stop
- optional saved-profile and serial-console connection creation when
  `mcp.connect_enabled` is true

MCP-created SSH profile connections request secrets through the ExaTerm UI. MCP does not
read saved credentials, expose API keys, or read log file contents directly.

### Terminal CLI

`exaterm-cli` is a console executable for local scripts and AI agents. It uses the same
current-user GUI control plane and backend tool calls as `exaterm-mcp`, but exposes typed
subcommands and JSON stdout instead of MCP JSON-RPC. The CLI requires both `mcp.enabled`
and `mcp.cli_enabled`; new profile and Serial connections additionally require
`mcp.connect_enabled`.

The external control client, GUI discovery, launch coordination, and local transport live
in `src-tauri/src/mcp/client.rs` and are shared by both executables. Terminal sessions and
credential prompts remain owned by the GUI.

## Important Data Flows

### User-Created Terminal Session

1. React connection dialog calls the matching backend command.
2. Backend opens SSH, Serial, or Telnet and registers the session in
   `TerminalControlState`.
3. Backend emits a protocol-specific connected event.
4. React creates a tab from the returned session ID.
5. `TerminalView` listens for protocol output events and writes to xterm.js.
6. Protocol output is also appended to `TerminalControlState` for MCP reads and snapshot
   recovery.

### Terminal Input

1. xterm.js emits user input.
2. `TerminalView` calls the protocol write command.
3. Backend sends bytes to the session.
4. Remote output returns through protocol event listeners.

### Logging

- Automatic logging is opt-in through terminal config and starts at connection creation.
- Manual logging can be started from UI or MCP, but UI flush is required to ensure rendered
  output has reached the log buffer.
- Plaintext logs stay under `%AppData%/ExaTerm/logs`.
- Log contents may include secrets and must not be exposed through MCP.

### MCP HTTP Removal

- HTTP MCP is removed rather than moved to a sidecar.
- The GUI process no longer hosts or binds the HTTP MCP listener.
- No `exaterm-mcp-http.exe` sidecar is planned.
- Existing HTTP MCP settings are treated as removal targets and are not migrated to stdio
  automatically.
- The GUI process remains the owner of terminal sessions, logs, credentials, and UI prompts.

## State Ownership Rules

Current ownership:

- Backend owns network/device sessions, decoded output buffers, and log state.
- Frontend owns visible tab placement and active tab state.
- MCP exposes backend runtime state but depends on the GUI process being alive.

Future ownership direction:

- Cross-window tab placement should move from `App.tsx` to a backend workspace state.
- External control should connect through a local control plane instead of assuming a
  manually launched GUI.
- Terminal sessions should remain single-owner runtime resources even when their visible
  tab moves between windows.

## Planned Architecture Direction

The next cross-cutting design direction is documented in
`ai-docs/MULTI_WINDOW_MCP_STDIO_DESIGN.md`.

The intended end state is:

- a backend workspace model that owns window registration, tab ownership, tab order, and
  active tab per window
- single-ownership terminal tabs that can move between windows without reconnecting
- drag-first tab movement across windows, with command/menu fallbacks allowed later
- a stdio MCP proxy executable that can be launched by MCP clients, starts the GUI normally
  when needed, and forwards MCP calls through a local GUI control plane
- Tauri sidecar distribution for `exaterm-mcp.exe` in user-facing builds
- shared MCP tool definitions behind the stdio proxy and GUI control plane
- removal of GUI-hosted HTTP MCP and any planned external HTTP MCP sidecar

This direction keeps the GUI as the single owner of terminal sessions while removing the
need for users to manually start the app before using MCP from a local stdio client.

## Documentation Model

Use this structure as the repository grows:

- `ai-docs/ARCHITECTURE.md`: compact current implementation map for agents.
- `ai-docs/SYSTEM_DESIGN.md`: durable system-wide design and direction.
- `ai-docs/*_DESIGN.md`: detailed feature or subsystem designs.
- `ai-docs/CHANGE_CHECKLIST.md`: before/after checklist for code changes.
- `ai-docs/RELEASE_CHECKLIST.md`: release-specific checklist.
- `docs/`: user-facing documentation only.

When adding a feature design, include:

- current-state constraints from the codebase
- goals, non-goals, and accepted defaults
- public interfaces, config, commands, events, or schemas
- data flow and ownership boundaries
- privacy and secret-handling implications
- phased implementation plan
- tests and manual acceptance scenarios

## Validation Expectations

Documentation-only changes do not require a full app build. At minimum:

- check Markdown formatting with the project formatter when practical
- confirm referenced paths exist
- confirm implementation guidance stayed in `ai-docs/`
- confirm user-facing documentation stayed in `docs/`

For implementation work that follows this design:

- use `pnpm run build` for frontend and TypeScript changes
- use `cargo test` for Rust backend changes
- use `pnpm run tauri build --debug` when installer, binary, or runtime integration matters
