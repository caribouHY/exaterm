# Changelog

## Unreleased

### Added

- Added early multi-window support so terminal tabs can be moved between ExaTerm windows.
- Added drag-and-drop terminal tab detaching to open a tab in a new ExaTerm window.
- Added a Settings option for enabling the local stdio MCP proxy transport.
- Added SSH connection diagnostics in the connection dialog to help identify where jump-host connections stall.
- Added per-profile MCP access controls for saved SSH and Telnet profiles.

### Changed

- Improved terminal tab handling so tabs are preserved and reassigned safely when secondary windows are closed.
- Reworked local MCP integration to use a stdio proxy instead of the previous GUI-hosted HTTP server.

### Removed

- Removed the HTTP-only MCP host and port settings because the GUI no longer hosts the local HTTP MCP transport.

### Fixed

- Fixed intermittent stdio MCP proxy startup failures on Windows after a previous ExaTerm launch left stale lock files behind.
- Updated the bundled SSH client stack to a current russh release so SSH-1.99 servers can complete handshake negotiation without an app-side workaround.

## v0.5.0

### Added

- Added memos for saved SSH and Telnet profiles, including MCP profile listing support for non-empty memos.
- Added one-hop SSH jump host support for saved SSH profiles, including UI profile selection, host-key checks, MCP profile connections, and config documentation.
- Added MCP tools to start and stop manual terminal session logs in ExaTerm's log directory while keeping log file contents private.

### Fixed

- Improved manual log history and cleanup so repeated manual logging and closed sessions keep accurate log state.
- Improved serial sessions so read and write errors reliably show the session as disconnected in the UI and MCP state.
- Fixed terminal output snapshots and MCP output reads for Shift-JIS and EUC-JP sessions.

## v0.4.0

### Added

- Added an opt-in local MCP server that lets external AI clients inspect terminal sessions, read output, send input, wait for output, and run commands with captured results.
- Added MCP tools for opening saved SSH/Telnet profiles and Serial consoles as visible ExaTerm tabs, with SSH credentials entered only in the ExaTerm UI.
- Added drag-and-drop reordering for terminal, settings, and log tabs during the current app session.

### Fixed

- Fixed Azure OpenAI chat requests so saved endpoint settings are reliably used when sending messages.
- Fixed AI chat history so UI-only provider error messages are not resent as conversation context.
- Improved application loading by lazy-loading optional panels.
- Skipped SSH key passphrase prompts for unencrypted private keys in both manual and MCP profile connections.
- Improved serial terminal input responsiveness in release builds.
- Fixed Cisco IOS mode command highlighting for long, delayed-echo, or resized input lines.

## v0.3.0

### Added

- Added SSH public key authentication, including private key path handling for saved SSH profiles.
- Added per-SSH-profile terminal display encoding defaults.
- Added Telnet connection profiles and command-line startup support for SSH and Telnet.
- Added terminal modes, including a Cisco IOS mode with decorated error messages.
- Added OpenRouter as an AI provider.
- Added pause and resume controls for active session logging.
- Added manual log append mode and a setting to include or omit session log headers.
- Added bulk deletion for session log history, with an option to remove auto log files.

### Changed

- Reworked the GUI layout around the title menu bar, terminal tabs, unified settings/log tabs, and compact status areas.
- Rendered AI assistant responses as Markdown and enabled selecting and copying text from AI chat messages.
- Relaxed the minimum window size so ExaTerm is easier to arrange in small tiled layouts.
- Cached cloud AI model lists to reduce repeated provider API requests.
- Split pull request CI checks and added Rust caching to reduce GitHub Actions wait time.
- Migrated frontend package management from npm to pnpm.

### Fixed

- Fixed manual screen-display logs so stopping the log saves the final unterminated line.
- Fixed the terminal scrollbar disappearing when the window height is reduced.

## v0.2.0

### Added

- Added Telnet connections with basic option negotiation, terminal resizing, and optional session logging.
- Added an opt-in setting to allow legacy SSH algorithms for older devices.
- Added SSH connection profiles in the new connection dialog.
- Added Escape key cancellation for the new connection dialog.
- Added Ctrl+Enter shortcuts for starting new connections.
- Added structured AI command suggestions with buttons to insert reviewed commands into the active terminal.
- Added config-only AI chat debug logs for troubleshooting provider requests and responses.
- Added selectable session log formatting, defaulting to screen-display logs that apply common line edits before saving.
- Added manual session logging with user-selected log files, including parallel saving while auto logging is enabled.

### Fixed

- Improved serial input responsiveness.
- Improved the AI assistant system prompt to prioritize terminal evidence and reduce unsupported assumptions.
- Fixed Azure OpenAI request endpoint handling.
- Fixed AI terminal context so it follows the active tab instead of mixing output from other tabs.
- Preserved AI assistant conversation history and model selection while the panel is closed during the current app session.
- Stopped fetching AI model lists before every chat request.

## v0.1.0

Initial release.
