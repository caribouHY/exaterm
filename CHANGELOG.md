# Changelog

## Unreleased

### Added

- Added SSH public key authentication.
- Added per-SSH-profile terminal display encoding defaults.
- Added a setting to include or omit session log headers.

### Changed

- Changed GUI layout.
- Relaxed the minimum window size so ExaTerm is easier to arrange in small tiled layouts.

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
