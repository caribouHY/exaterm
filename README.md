# ExaTerm

SSH and Serial communication terminal with AI assistant.

## Features

- SSH and Serial communication
  - Multiple tabs
- AI assistant
  - OpenAI
  - Anthropic
  - Gemini
  - Ollama
- Optional session logging

## Session Logs

Session logging is off by default. New installs do not create terminal session logs unless you explicitly enable Auto Session Log in Settings.

When Auto Session Log is enabled, ExaTerm records SSH and Serial terminal input/output as plaintext log files. These logs can include commands, command output, prompts, hostnames, usernames, device output, and other sensitive terminal content.

Logs are stored under `%AppData%/ExaTerm/logs` on Windows. The same location is shown in the Logs view.

## Getting Started

### Prerequisites

- Rust
- Node.js

### Installation

1. Clone the repository
2. Run `npm install` to install dependencies
3. Run `cargo run` to start the application
