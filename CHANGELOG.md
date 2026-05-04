# Changelog

## Unreleased

### Added

- Added SSH public key authentication.
- Added per-SSH-profile terminal display encoding defaults.
- Added Telnet connection profiles.
- Added a setting to include or omit session log headers.
- Added OpenRouter as an AI provider.
- Added terminal modes with General and Cisco IOS options.
- Added command-line startup support for SSH and Telnet.
- Added per-tab pause and resume controls for session logging.

### Changed

- Changed GUI layout.
- Rendered AI assistant responses as Markdown.
- Relaxed the minimum window size so ExaTerm is easier to arrange in small tiled layouts.
- Enabled selecting and copying text from AI chat messages.
- Cached cloud AI model lists to reduce repeated provider API requests.

### Fixed

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
