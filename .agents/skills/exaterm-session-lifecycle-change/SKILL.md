---
name: exaterm-session-lifecycle-change
description: Develop ExaTerm session lifecycle changes across connection attempts, teardown, tab/window ownership, output restoration, and logging. Use for state transitions or async coordination, not pure styling or operating existing sessions.
---

# ExaTerm Session Lifecycle Change

## Establish Ownership

- Follow `AGENTS.md` for branch preparation and repository constraints. Preserve the requested investigation, proposal, or implementation scope.
- Start with `Reading by Ownership` and `Design Principles` in `docs/development/ARCHITECTURE.md`; read only the sections selected for the affected owners. Architecture remains the source of truth for durable ownership and behavior.
- Use the relevant sections of [lifecycle review cases](references/lifecycle-review.md) to locate current owners and select failure sequences. Trace each changed transition from its caller through the authoritative owner to its consumers.
- Identify the resource and its identity (attempt, prompt, session, tab, or window), who owns it before and after completion, the cancellation cutoff, and who cleans it up on failure or disposal. Keep view state and workspace metadata distinct from backend resource state.

## Change the Transition

- Preserve one authoritative transition owner. Avoid adding parallel flags or reducers that can disagree across an `await`; derive UI snapshots from the existing owner.
- For changed asynchronous paths, examine success, rejection, cancellation, retry, and disposal at the relevant suspension points. Revalidate the operation's identity and ownership before applying a late result or releasing a resource.
- Distinguish cancellation requests from cancellation completion, backend success from GUI registration, and disconnect from tab/window removal. Cleanup must release only resources still owned by the cancelled or disposed operation.
- Keep existing sessions, retained output, and active logs intact through ordinary UI updates and ownership transfer. A frontend remount can be valid during tab movement; assess backend identity and restoration rather than rejecting every remount.
- Select deterministic regression tests for the changed races using controlled promises, events, or I/O boundaries. Exercise the real controller/service and assert observable state, ordering, retained data, and resource ownership; do not reproduce the implementation inside a fake or rely on arbitrary sleeps.

## Review the Affected Boundaries

- For changes to cancellation/finalization, resource cleanup, cross-window ownership, output recovery, or log flush/state coordination, delegate one independent review to `exaterm-state-reviewer` when available. Also use it for an explicit user request. Local review is sufficient for changes with no lifecycle impact.
- Pass intended transitions, exact comparison base and working-tree scope (including relevant untracked files), affected owners, and available validation evidence. The reviewer reports findings; the parent owns fixes and final verification.
- If the custom role cannot be selected, give `.codex/agents/exaterm-state-reviewer.toml` as instructions to a general subagent and disclose the fallback. If delegation is unavailable, apply the same checks locally and report that independent review was not performed.
- Apply [ExaTerm External Control Change](../exaterm-external-control-change/SKILL.md) when CLI/MCP or shared external-control behavior changes. When both reviewers apply, give the state reviewer ownership/race questions and the contract reviewer public-contract/consumer questions; consolidate duplicate findings and avoid recursive delegation.
- Apply [ExaTerm UI Change](../exaterm-ui-change/SKILL.md) for affected React/UI behavior. A lifecycle change alone does not require loading styling guidance.

## Validate and Complete

- Use [ExaTerm Validate Change](../exaterm-validate-change/SKILL.md) to choose checks, including its optimized-runtime reference for protocol setup, cancellation, timeouts, or Tauri async call-chain changes. Keep validation commands there.
- Update architecture only when ownership or durable behavior changes, and apply the `AGENTS.md` changelog rule. Confirm any affected CLI operating guidance through the external-control Skill.
- Report preserved or deliberately changed transitions, actionable review findings and their resolution, executed checks, and unperformed GUI, real-device, or optimized-executable scenarios. Keep session content, connection details, and secrets out of diagnostics and reports.
- This Skill does not authorize live terminal operations or publication. Do not start or poll Codacy analysis as part of it; retrieve analysis when the user requests it.
