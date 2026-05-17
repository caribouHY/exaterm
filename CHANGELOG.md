# Changelog

## Unreleased

### Added

- Added a local MCP server that lets external AI clients list terminal sessions, read recent terminal output, and send input to connected sessions.
- Added MCP terminal automation tools for cursor-based output deltas, output waiting, and command execution with returned output.
- Added an opt-in MCP profile connection flow for listing saved SSH/Telnet profiles and opening them as visible ExaTerm tabs, with SSH credentials entered only in the ExaTerm UI.

### Fixed

- Fixed Azure OpenAI chat requests so saved endpoint settings stay in sync with the AI panel and are used as a backend fallback when sending.
- Fixed AI chat history so UI-only provider error messages are not resent as conversation context.
- Fixed the production build chunk-size warning by lazy-loading optional frontend panels.
- Improved serial terminal input responsiveness in release builds.

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
