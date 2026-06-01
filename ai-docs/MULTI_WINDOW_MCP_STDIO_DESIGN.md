# Multi-Window Tabs and External MCP Design

This document is AI-facing implementation context. User-facing documentation belongs in
`docs/`.

## Summary

ExaTerm will support moving terminal tabs between multiple windows and external MCP entry
points without requiring the user to manually start the GUI first.

Accepted product direction:

- Tabs use single ownership: one terminal tab is visible in exactly one window at a time.
- Cross-window movement is drag-first.
- stdio MCP uses an executable proxy that can be launched by MCP clients.
- HTTP MCP must move out of the GUI process into a sibling proxy/sidecar.
- The proxy auto-starts the GUI in normal visible mode when needed.
- The GUI process remains the owner of terminal sessions, logs, credentials, and UI prompts.
- The GUI process must not host an HTTP MCP listener in the target architecture.
- Future extensibility is more important than preserving the smallest possible change set.

## Goals

- Move terminal tabs between ExaTerm windows without reconnecting or losing backend session
  state.
- Preserve terminal output buffers, connection lifecycle, logging state, encoding, terminal
  mode, and MCP visibility during tab moves.
- Add stdio MCP support for local MCP clients that expect to spawn a command.
- Replace the GUI-hosted HTTP MCP server with an external HTTP MCP proxy while preserving
  existing URL and tool compatibility.
- Avoid making users manually start ExaTerm before using MCP.
- Keep HTTP MCP client compatibility through the external proxy.
- Create boundaries that can later support daemon mode, additional transports, workspace
  persistence, and richer MCP tools.

## Non-Goals

- Do not support simultaneous multi-window display of the same terminal session in v1.
- Do not expose saved secrets, API keys, or log file contents through MCP.
- Do not move Settings or Logs utility tabs between windows in v1.
- Do not require a frontend framework rewrite before the feature can ship.
- Do not make HTTP MCP depend on stdio MCP.
- Do not keep an in-GUI HTTP MCP server as a long-term fallback.
- Do not let an external HTTP MCP process own SSH, Serial, Telnet, logging, or credential
  state directly.

## Current Constraints

- `src/App.tsx` currently owns tab placement, tab order, active tab, and utility tabs.
- Backend protocol sessions already live outside React and can survive a React remount.
- `TerminalControlState` keeps decoded output snapshots and deltas, so a moved tab can
  restore recent output after its `TerminalView` remounts.
- `TerminalView` owns xterm.js and protocol event listeners, so moving a tab means
  unmounting the old view and creating a new view in another window.
- The MCP HTTP server currently runs inside the GUI process and starts only when the GUI is
  already running.
- Current release builds use `windows_subsystem = "windows"` for the main GUI binary, which
  is not a good stdio MCP entry point.

## Target Architecture

### Workspace State

Introduce a backend workspace state that owns cross-window placement.

Use this initial module:

- `src-tauri/src/workspace.rs`

Use this initial core model:

```rust
struct WorkspaceState {
    windows: HashMap<WindowId, WindowWorkspace>,
    tabs: HashMap<TabId, WorkspaceTab>,
    last_focused_window: Option<WindowId>,
}

struct WindowWorkspace {
    window_id: WindowId,
    tab_order: Vec<TabId>,
    active_tab_id: Option<TabId>,
}

struct WorkspaceTab {
    tab_id: TabId,
    session_id: String,
    connection_type: TerminalProtocol,
    title: String,
    owner_window_id: WindowId,
    encoding: String,
    terminal_mode: String,
    is_connected: bool,
    is_auto_logging: bool,
    is_manual_logging: bool,
    is_logging_paused: bool,
    manual_log_file_path: Option<String>,
}
```

Rules:

- `owner_window_id` is required for terminal tabs.
- A terminal tab can appear in only one `WindowWorkspace.tab_order`.
- Moving a tab is an atomic backend operation.
- Frontend windows render a projection of their own workspace state.
- Utility tabs remain local frontend state in v1.

### Window Lifecycle

Each Tauri webview window registers itself on startup.

v1 commands:

- `workspace_window_register(window_id, label, focused) -> WorkspaceSnapshot`
- `workspace_window_focus(window_id)`
- `workspace_window_unregister(window_id)`
- `workspace_snapshot_get(window_id) -> WorkspaceSnapshot`

v1 events:

- `workspace://updated` with a full `WorkspaceSnapshot` in v1
- `workspace://window-closed`

Versioned deltas are a future optimization and must not be required for the first
multi-window implementation.

Close behavior:

- If a non-last window closes, its owned terminal tabs move to the last focused remaining
  window.
- If the last window closes, preserve the current app exit behavior and disconnect through
  existing close paths.
- If a tab move destination disappears before `workspace_tab_move` commits, reject the move,
  keep tab ownership on the source window, and do not disconnect the session.
- If both the destination and source windows disappear before move commit, rehome the tab to
  the last focused remaining window.
- If no windows remain, fall back to the current app exit flow.

### Tab Creation

All terminal tab creation paths must register a workspace tab:

- user-created SSH, Serial, Telnet sessions
- startup CLI-created sessions
- MCP-created saved-profile sessions
- MCP-created serial console sessions

The backend must assign an owner window with this priority:

1. foreground/focused ExaTerm window
2. last focused ExaTerm window
3. main window
4. newly created normal visible main window if no ExaTerm window exists

For UI-created sessions, the focused window wins. For MCP-created sessions, the same
priority order applies. If an MCP proxy auto-starts the GUI, the created main window owns
the new tab. Minimized windows can own MCP-created tabs, but if a credential prompt is
required the owning window must be restored and brought to the foreground.

The backend must emit one workspace update instead of relying on each frontend window to
infer tab creation from protocol events.

### Tab Move Flow

v1 command:

- `workspace_tab_move(tab_id, from_window_id, to_window_id, target_index) -> WorkspaceSnapshot`

Flow:

1. Source window starts dragging a terminal tab.
2. Backend records drag metadata and tab identity.
3. Destination window tab bar reports hover/drop target.
4. Before committing the move, the source `TerminalView` flushes auto and manual log
   sanitizers and waits for the append requests to complete.
5. On drop, frontend calls `workspace_tab_move`.
6. Backend validates single ownership and updates both window orders.
7. Backend emits `workspace://updated` to affected windows.
8. Source window unmounts the old `TerminalView`.
9. Destination window mounts a new `TerminalView`.
10. New `TerminalView` calls `terminal_output_snapshot_get` and resumes delta/event handling.

Do not call protocol disconnect commands during a move.

### Scrollback and Log Handling During Moves

v1 preserves the backend-retained recent terminal output, not the full xterm.js scrollback.
The destination `TerminalView` restores output through `terminal_output_snapshot_get` and
then resumes live event/delta handling.

Backend output retention must be configurable from terminal scrollback settings and bounded:

- derive a character limit from `terminal.scrollback * 160`
- minimum: `64 * 1024` characters
- maximum: `2 * 1024 * 1024` characters

If a restored snapshot returns `truncated=true`, the UI can show or store that state, but
v1 does not reconstruct content that has already been trimmed from
`TerminalControlState`.

Before a tab moves, the source `TerminalView` must flush both automatic and manual log
sanitizers. If this flush fails, the move still proceeds and the error is logged as a
warning. Manual log stop keeps the stricter existing behavior: it must wait for flush before
stopping the log.

### Drag-First Interaction

The first implementation must support:

- reorder within the same window
- drag to another ExaTerm window tab bar
- drag outside the current window to detach into a new window

Recommended implementation detail:

- Keep local pointer logic for same-window reorder.
- Add a Rust-side drag coordinator for cross-window detection on Windows.
- Use Tauri commands/events for drag start, hover, cancel, and commit.
- If cross-window pointer tracking is unreliable on a platform, fall back to a tab context
  menu later without changing workspace ownership rules.

v1 drag commands/events:

- `workspace_tab_drag_start(window_id, tab_id, pointer_screen_position)`
- `workspace_tab_drag_update(pointer_screen_position)`
- `workspace_tab_drag_drop(pointer_screen_position)`
- `workspace_tab_drag_cancel()`
- `workspace://drag-preview`

### Frontend Refactor

Refactor `App.tsx` from owner of all tab placement into renderer of one window projection.

Frontend responsibilities after refactor:

- request/register its window workspace
- render local terminal tab projection
- render local utility tabs
- keep xterm.js instances and UI-only refs for visible tabs
- request backend workspace operations for move, close, and activation
- keep AI panel and Settings/Logs presentation local to each window

Backend responsibilities after refactor:

- own terminal tab placement and connected/disconnected status projection
- choose owner window for externally created sessions
- maintain tab move invariants
- broadcast workspace updates

### MCP stdio Proxy

Add a new binary for MCP clients:

- command name: `exaterm-mcp.exe`
- crate location: `src-tauri/src/bin/exaterm-mcp.rs`
- distribution: Tauri sidecar in user-facing builds
- transport: stdio using `rmcp` `transport-io`
- stdout: JSON-RPC only
- stderr or file logs: diagnostics only

The proxy must not own terminal sessions directly. It must:

1. start as a stdio MCP server process
2. try to discover a running ExaTerm GUI control endpoint for up to 2 seconds
3. start `exaterm.exe` in normal visible mode if no endpoint is available
4. wait up to 30 seconds for the GUI control endpoint after launching the GUI
5. forward MCP tool calls to the GUI control endpoint
6. return structured MCP results to the client

This keeps credential prompts, log-control UI, terminal ownership, and config behavior in
the GUI process.

Proxy discovery retries every 250 ms. If multiple proxies start at once on Windows, they
must use a current-user named mutex so only one process launches the GUI. Proxies that do
not hold the mutex wait for the control plane to appear. Timeout errors return MCP
`internal_error` with the message `ExaTerm GUI control plane is unavailable`.

### MCP HTTP Proxy

HTTP MCP must be externalized into a sibling proxy/sidecar process instead of being hosted
inside the GUI process.

v1 command name:

- `exaterm-mcp-http.exe`
- distribution: Tauri sidecar in user-facing builds

Recommended behavior:

1. listen on the configured HTTP address, such as `127.0.0.1:8765`
2. expose the same Streamable HTTP `/mcp` endpoint and tool schema as the legacy in-GUI
   HTTP server
3. try to discover a running ExaTerm GUI control endpoint for up to 2 seconds
4. start `exaterm.exe` in normal visible mode if no endpoint is available
5. wait up to 30 seconds for the GUI control endpoint after launching the GUI
6. forward MCP tool calls to the GUI control endpoint
7. return HTTP MCP responses with the same compatibility expectations as the current server

The HTTP proxy must not own terminal sessions directly. It is a network-facing MCP
transport adapter over the GUI control plane.

HTTP proxy discovery uses the same 250 ms retry interval, current-user named mutex, and
`ExaTerm GUI control plane is unavailable` error message as the stdio proxy.

Important difference from stdio:

- stdio MCP clients usually spawn the configured command, so `exaterm-mcp.exe` can solve
  startup by being the command.
- HTTP MCP clients usually connect to an already listening URL, so `exaterm-mcp-http.exe`
  uses user-level autostart to remove the manual-start requirement.

Required startup behavior:

- `mcp.http_enabled=true` means ExaTerm must provide HTTP MCP through `exaterm-mcp-http.exe`.
- Add `mcp.http_autostart_enabled`; default it to the effective value of
  `mcp.http_enabled`.
- When `mcp.http_autostart_enabled=true`, register user-level login startup for
  `exaterm-mcp-http.exe`.
- When `mcp.http_autostart_enabled=false`, do not register login startup; HTTP MCP still
  works when the proxy is launched manually or by the GUI.
- When the GUI starts and `mcp.http_enabled=true`, it must start the HTTP proxy if the
  proxy is not already running. Login autostart remains the primary way to make HTTP MCP
  available before the GUI is manually launched.
- If autostart registration fails during migration or Settings save, show a Settings
  warning and keep the GUI running.

Implementation order:

- Implement stdio proxy first because MCP clients can spawn it directly.
- Implement HTTP proxy second using the same GUI control plane and remove the GUI-hosted
  HTTP listener in the same migration.
- Preserve client-facing compatibility through the external HTTP proxy, not by keeping the
  GUI process as an HTTP server.

### GUI Control Plane

Add a local control plane in the GUI process for external MCP proxy calls.

Preferred Windows transport:

- named pipe under the current user scope
- ACL restricted to the current user
- instance identity or nonce handshake to avoid accidental cross-user attachment

Future transport abstraction:

```rust
trait McpControlTransport {
    async fn serve(control_service: McpControlService) -> Result<(), String>;
}

trait McpControlClient {
    async fn call_tool(&self, name: String, args: serde_json::Value)
        -> Result<serde_json::Value, String>;
}
```

The control protocol must use transport-neutral JSON messages so a future Unix socket or
local TCP implementation can reuse the same service.

Windows v1 uses a current-user ACL named pipe. The first message on every connection must
be a newline-delimited JSON `hello` message:

```json
{
  "type": "hello",
  "protocol_version": 1,
  "client": "exaterm-mcp",
  "client_pid": 1234
}
```

The GUI validates the current-user connection and `protocol_version`, then replies with a
`session_nonce`. Every following control request must include that nonce. Unknown protocol
versions, missing nonce values, and failed ACL/current-user checks are rejected before any
tool call is executed. Diagnostics must go to stderr or log files, never to stdio MCP
stdout.

### Shared MCP Backend

Split `src-tauri/src/mcp.rs` into transport-agnostic tool logic and transport adapters.

v1 module shape:

- `mcp/service.rs`: `ExaTermMcpServer`, tool definitions, shared result shaping
- `mcp/backend.rs`: trait implemented by in-process and proxy backends
- `mcp/http_transport.rs`: Streamable HTTP server and HTTP response compatibility helpers
  used by the external proxy
- `mcp/stdio.rs`: stdio proxy server
- `mcp/http_proxy.rs`: external HTTP MCP proxy/sidecar
- `mcp/control.rs`: GUI local control plane

Backend variants:

- `InProcessMcpBackend`: used by the GUI control plane only
- `ProxyMcpBackend`: used by `exaterm-mcp.exe` and `exaterm-mcp-http.exe` to call the
  GUI control plane

Tool names and JSON result shapes must stay identical across HTTP and stdio.

Both proxy executables are distributed as Tauri sidecars in user-facing builds. Development
and test builds can also expose Cargo bins, but end-user documentation and MCP client
examples must use the bundled sidecar paths.

### Config Evolution

Keep existing config compatible.

Current:

```json
{
  "mcp": {
    "enabled": false,
    "connect_enabled": false,
    "host": "127.0.0.1",
    "port": 8765
  }
}
```

Target:

```json
{
  "mcp": {
    "enabled": false,
    "http_enabled": false,
    "http_autostart_enabled": false,
    "stdio_enabled": false,
    "connect_enabled": false,
    "host": "127.0.0.1",
    "port": 8765
  }
}
```

Migration default:

- Existing `mcp.enabled` remains the master MCP permission flag.
- Existing installs keep HTTP behavior by treating missing `http_enabled` as the old
  `enabled` behavior, but the serving process changes from GUI-hosted HTTP to the external
  HTTP proxy during migration.
- Missing `http_autostart_enabled` defaults to the effective value of `http_enabled`.
- Existing configs do not automatically enable `stdio_enabled`; users must opt in.
- New installs default to `mcp.enabled=false`, `http_enabled=false`,
  `http_autostart_enabled=false`, and `stdio_enabled=false`.
- Settings UI must describe transport-specific behavior and restart requirements.
- Enabling the MCP master flag in Settings must not automatically enable HTTP or stdio.

Transport gating rules:

- `mcp.enabled=false` disables all MCP transports.
- `mcp.enabled=true` permits MCP generally, but each transport still needs its own flag.
- `mcp.http_enabled=false` disables only HTTP MCP and must not disable stdio MCP.
- `mcp.http_autostart_enabled=false` disables only login startup for the HTTP proxy and must
  not disable HTTP MCP itself.
- `mcp.stdio_enabled=false` disables only stdio MCP and must not disable HTTP MCP.
- `mcp.connect_enabled` gates new connection tools for every transport; it is intentionally
  shared because it controls tool capability, not transport startup.

### Security and Privacy

- stdio proxy must not log JSON-RPC payloads by default.
- HTTP proxy must preserve current Host and Origin protections and must not widen the bind
  address by default.
- GUI control plane must accept only current-user local clients.
- MCP-created connections must keep the existing `mcp.connect_enabled` gate.
- SSH credentials and key passphrases must still be entered in the GUI.
- API keys remain in the OS credential store.
- Log file contents remain unavailable through MCP.
- Terminal output remains sensitive and must be returned only through explicitly enabled MCP
  tools.

## Implementation Phases

### Phase 1: Workspace Foundation

- Add backend workspace state and tests for single ownership.
- Register main window and render from `WorkspaceSnapshot`.
- Keep behavior visually equivalent for one window.
- Move tab creation and disconnect status updates into workspace projection.

### Phase 2: Multi-Window Shell

- Add commands to create a new Tauri window.
- Support window register, focus, close, and rehome behavior.
- Render terminal tabs per window projection.
- Ensure moved or rehomed tabs remount from terminal output snapshot without reconnecting.

### Phase 3: Drag-First Tab Movement

- Extend `TerminalTabs` drag behavior for cross-window and detach flows.
- Add Rust drag coordinator for Windows screen-position tracking.
- Support detach to new window on outside drop.
- A fallback tab context menu is allowed later only for accessibility or platform limits.

### Phase 4: MCP Refactor

- Split MCP tool logic from HTTP transport.
- Add backend trait and in-process implementation.
- Move GUI-owned tool execution behind the GUI control plane.
- Keep HTTP MCP tool schemas and client-visible result behavior unchanged.

### Phase 5: stdio Proxy and Control Plane

- Add `exaterm-mcp.exe` sidecar/bin.
- Add `rmcp` `transport-io` feature.
- Add GUI control plane with current-user local transport.
- Implement GUI discovery, normal visible auto-start, 2-second discovery timeout, 30-second
  post-launch timeout, 250 ms retry interval, current-user launch lock, and proxy forwarding.
- Add smoke tests for stdio initialize and tools/list.

### Phase 6: External HTTP Proxy

- Add `exaterm-mcp-http.exe` sidecar/bin.
- Move HTTP serving into the external proxy when `mcp.http_enabled=true`.
- Reuse `ProxyMcpBackend` and the GUI control plane.
- Preserve the existing `/mcp` Streamable HTTP behavior, allowed-host behavior, and JSON
  response compatibility.
- Add `mcp.http_autostart_enabled` and user-level autostart registration for the HTTP proxy.
- Remove `spawn_mcp_server` startup from the GUI process and ensure the GUI no longer binds
  the HTTP MCP port.

### Phase 7: Documentation and Settings

- Update `docs/CONFIG_JSON_GUIDE.en.md` and `docs/CONFIG_JSON_GUIDE.ja.md` for user-facing
  MCP transport settings.
- Update settings UI locale strings in both languages.
- Add changelog entry for user-visible behavior.

## Test Plan

Rust unit tests:

- workspace register/unregister keeps tab ownership valid
- moving a tab removes it from source and inserts it into destination
- moving a tab to a missing destination fails without disconnecting
- moving a tab after source and destination vanish rehomes to the last focused remaining
  window
- closing a non-last window rehomes tabs to last focused window
- disconnected status updates appear in the owning window projection
- MCP backend returns identical results through in-process and proxy backends where practical
- config migration preserves HTTP client behavior by setting `http_enabled=true` and
  `http_autostart_enabled=true` for old `mcp.enabled=true` configs
- existing configs do not auto-enable `stdio_enabled`
- GUI startup no longer binds the HTTP MCP port
- HTTP proxy preserves current `/mcp` compatibility and Host/Origin protections
- HTTP proxy returns a clear MCP/HTTP error if the GUI cannot start or the control plane is
  unavailable
- control plane rejects missing nonce, wrong protocol version, and non-current-user clients
- proxy launch lock prevents duplicate GUI launches when multiple proxies start together

Frontend tests or manual verification:

- same-window reorder still works
- drag to another window moves the tab once
- outside drop creates a new window and moves the tab
- moved tab restores recent output through snapshot and continues receiving new output
- input after a move goes only to the destination window
- automatic and manual log buffers flush before a move, and log state and pause state remain
  visible after a move
- MCP-created tab appears in the last focused window
- credential prompt appears in the GUI started by stdio proxy

Manual MCP scenarios:

- GUI not running: MCP client launches `exaterm-mcp.exe`, GUI starts normally visible, tools/list
  succeeds
- GUI running: proxy attaches to existing GUI control endpoint
- HTTP MCP remains available when configured
- HTTP proxy listening before GUI startup can start the GUI and forward tool calls
- `mcp.http_autostart_enabled=false` avoids login startup but does not disable HTTP MCP
- `mcp.connect_enabled=false` still blocks new connection tools
- proxy timeout returns a clear MCP error if GUI cannot start or cannot expose control plane

## Open Implementation Notes

- Decide exact named pipe crate or Tauri-compatible IPC crate during implementation.
- Keep these decisions recorded in this file or a follow-up ADR before implementation
  starts.
