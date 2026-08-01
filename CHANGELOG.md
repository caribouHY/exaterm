# Changelog

## Unreleased

### Added

- Added an Exit menu action that closes all ExaTerm windows, with confirmation when terminal sessions are still connected.

## v0.8.1

### Added

- Added signed in-app updates for published stable releases, with startup and manual checks, release notes, download progress, and confirmation before connected terminal sessions are closed.
- Added editable keyboard shortcuts for new connections, windows, and Settings, with immediate updates across open windows.
- Added configurable terminal shortcuts for selecting the full buffer, copying selected text, and pasting clipboard text without overriding common remote control keys by default.
- Added configurable terminal shortcuts for starting, appending, stopping, pausing, and resuming session logs.

### Fixed

- Fixed MCP tool discovery in strict clients by declaring the `read_terminal_output` input schema as an object.

## v0.8.0

### Changed

- Improved the desktop Settings experience with clearer categories, accessibility, save and revert feedback, and safer dependent-control behavior.
- Made AI provider setup and saved API-key management easier to understand and safer to use.
- Refined the dark desktop interface for more consistent visuals.

### Added

- Added global SSH algorithm selection for key exchange, host keys, ciphers, MACs, and compression.
- Added a terminal tab action for opening a prefilled new SSH or Telnet connection to the same destination without retaining credentials.
- Added a terminal tab context menu for closing tabs or moving terminal sessions to a new window.
- Added PowerShell-style right-click copy and paste, with confirmation before multi-line terminal pastes.

### Fixed

- Improved terminal-tab ordering, focus, and visibility when opening, closing, moving, and reordering tabs, including across Settings and Logs.
- Fixed terminal input encoding so the selected UTF-8, Shift_JIS, or EUC-JP setting is applied to outgoing SSH, Serial, Telnet, and external-control input.
- Improved SSH private-key validation, including DSA and EC PEM headers.
- Improved Settings reliability with an anchored save bar, clearer load and save errors, and validated terminal input ranges.
- Made SSH output loss during overload visible in the terminal.
- Fixed SSH sessions so remote disconnects are reflected more reliably in the UI.
- Improved SSH connections so stalled connection setup no longer leaves the connection dialog waiting indefinitely.
- Improved SSH sessions so stalled input sends and terminal resizes time out instead of waiting indefinitely.
- Made desktop error messages respect the selected app language.

## v0.7.1

### Fixed

- Fixed SSH sessions sometimes stopping terminal input and echo during repeated input.

## v0.7.0

### Changed

- Reorganized External Control settings around the Terminal CLI, with `exaterm-mcp` retained as a compatibility option for MCP clients.
- Renamed external-control configuration from `mcp` to `external_control`. Existing settings and per-profile access permissions are migrated automatically.
- Saved SSH and Telnet profile connections now identify both the profile ID and connection type, so profiles can safely share the same ID.
- Consolidated MCP terminal output reading into one `read_terminal_output` tool with `recent`, `delta`, and `wait` modes.

### Added

- Added the opt-in `exaterm-cli` command-line interface for trusted local programs and AI agents to control ExaTerm terminal sessions and receive JSON results.
- Added `exaterm-cli` to the Windows installer alongside `exaterm-mcp`.
- Added optional SSH or Telnet filtering when listing saved profiles from the Terminal CLI.
- Added an installable Agent Skill for using `exaterm-cli` with Codex, Claude Code, GitHub Copilot, and other compatible clients.
- Added English and Japanese guides for the Terminal CLI and its external-control settings.

### Removed

- Removed the MCP `read_terminal_output_delta` and `wait_terminal_output` tools. MCP client configurations and prompts must use `read_terminal_output` with `mode: "delta"` or `mode: "wait"` instead.

## v0.6.1

### Added

- Added drag-and-drop tab detaching so terminal tabs can be opened in separate ExaTerm windows.
- Added opt-in stdio MCP support for local MCP clients, including launching ExaTerm when needed.
- Added SSH connection diagnostics in the connection dialog to help identify where jump-host connections stall.
- Added per-profile MCP access controls for saved SSH and Telnet profiles.

### Changed

- Terminal tabs are now preserved and returned to another ExaTerm window when a secondary window is closed.
- Changed the default UI language setting to follow the system language, with English fallback when the OS language is unsupported.
- Changed the default session log settings so automatic logging stays off and new logs omit the ExaTerm header unless explicitly enabled.

### Removed

- Removed HTTP MCP support. Existing HTTP MCP users must enable stdio MCP and configure their MCP client to launch `exaterm-mcp`.

### Fixed

- Fixed intermittent MCP startup failures on Windows after ExaTerm did not shut down cleanly.
- Improved connection compatibility with SSH-1.99 servers.

## v0.6.0

Release error: v0.5.0 was mistakenly released as v0.6.0.

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
