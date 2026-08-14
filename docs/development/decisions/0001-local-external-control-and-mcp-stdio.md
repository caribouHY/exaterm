# ADR 0001: Keep Sessions in the GUI and Expose MCP over stdio

- Status: Accepted

## Context

ExaTerm terminal sessions, logs, credential prompts, and visible tabs are owned by the desktop application. External automation needs to control those sessions without creating a second session owner or exposing a network service that outlives the GUI security model.

The former HTTP MCP transport required a listener and transport-specific settings. It also encouraged coupling MCP tool behavior to one server transport.

## Decision

- Keep the normal ExaTerm GUI process as the single owner of terminal sessions, log state, credentials, and prompts.
- Expose terminal operations through a transport-neutral service in `src-tauri/src/external_control/`.
- Provide `exaterm-mcp` as a stdio MCP proxy that discovers or visibly launches the GUI and forwards requests to its current-user local control plane.
- Use a current-user named pipe with a version and nonce handshake on Windows. Keep non-Windows transport behind the same protocol boundary.
- Keep `exaterm-cli` and `exaterm-mcp` as separate adapters over the same external-control service.
- Remove HTTP MCP rather than maintaining transport compatibility.
- Require explicit configuration gates for external control, individual adapters, and new-connection capability.

## Consequences

- Terminal sessions remain available to the GUI and external clients without duplicate ownership or reconnection.
- Credential and log-control prompts remain visible and GUI-owned.
- MCP clients can launch a stdio process without requiring users to start ExaTerm manually.
- MCP stdout must contain JSON-RPC only; diagnostics must use stderr or privacy-safe logs.
- HTTP MCP settings and endpoints are not supported.
- External clients must be local, current-user processes and cannot read stored API keys or plaintext log files.
