---
name: exaterm-external-control-change
description: Develop ExaTerm CLI, MCP, and shared external-control operations while preserving public contracts and GUI ownership. Use for implementation or contract changes, not for operating terminal sessions.
---

# ExaTerm External Control Change

## Scope the Change

- Follow `AGENTS.md` for branch preparation and repository constraints. Preserve the user's requested scope; an investigation request stays read-only.
- Start with `Reading by Ownership` in `docs/development/ARCHITECTURE.md`, then read `Backend Runtime` and `External Control, MCP, and CLI`. Add connection, workspace, logging, or storage sections only when their owners are affected.
- Read [contract surfaces](references/contracts.md) and identify the affected entry points, shared operation, permission gates, result/error shapes, and consumer documentation. Record the intended before/after behavior and any deliberate adapter differences.
- A CLI-only or MCP-only request does not authorize adding the operation to the other adapter. Preserve existing differences unless the task changes them.

## Implement Through the Shared Service

- Put shared permission and operation behavior in `src-tauri/src/external_control/service/`. Keep CLI/MCP adapters responsible for their input/output contracts; preserve concrete result structs or state-specific enums until the serialization boundary.
- Keep the GUI as the owner of sessions, prompts, credentials, and logging. For operations requiring GUI state changes, trace request delivery to the current owner, acknowledgement correlation, timeout/cancellation cleanup, and the actual success condition. Emitting an event alone does not prove completion.
- Evaluate wire compatibility when request/response serialization or handshake behavior changes. Follow the current protocol's compatibility policy and update affected callers and tests; do not bump the protocol merely because an internal implementation changed.
- Select regression cases from the behavior changed: permissions and invalid inputs, success/error serialization, repeated operations, stale owner or late acknowledgement, and cancellation where applicable. Replace narrow I/O boundaries in tests, keeping the production service's policy and finalization path under test.

## Synchronize and Review

- Use the applicable rows in [contract surfaces](references/contracts.md) to update consumers and documentation in the same change. For CLI changes, follow the existing [CLI contract synchronization](../exaterm-validate-change/references/cli-contract.md) procedure, including when concluding that no public contract changed.
- For public request/result/error, permission, protocol, or GUI acknowledgement changes, delegate one independent review to `exaterm-contract-reviewer` when subagents are available. Also use it when the user explicitly requests that review. Pure internal changes with no contract impact can use local review.
- Give the reviewer the intended behavior, exact comparison base and working-tree scope (or a bounded diff), affected entry points, and available validation evidence. Include new/untracked files when relevant. Review is read-only; the parent owns fixes and final verification.
- If the custom role cannot be selected but delegation is available, pass `.codex/agents/exaterm-contract-reviewer.toml` as the role instructions to a general subagent and identify that fallback. If delegation is unavailable, apply the same checks locally and report that independent review was not performed.

## Validate and Complete

- Use [ExaTerm Validate Change](../exaterm-validate-change/SKILL.md) for commands, CLI synchronization, optimized-runtime checks, and the changelog decision. Do not maintain a second command list here.
- Resolve actionable review findings within scope. Report contract changes, intentional adapter differences, completed validation, and remaining evidence gaps without terminal data or credentials.
- Keep automated, GUI, real-device, optimized-executable, and remote analysis evidence distinct. Do not start or poll Codacy analysis as part of this Skill; retrieve it when the user requests it.
- Publication follows the separately authorized PR workflow in `AGENTS.md`.
