# ExaTerm AI Agent Guide

Keep human-facing project documentation in `docs/`. Keep agent execution procedures in repository-local Skills under `.agents/skills/` and custom agent definitions under `.codex/agents/`.

## Project Snapshot

ExaTerm is a Windows-focused Tauri v2 desktop app with:

- React + TypeScript frontend under `src/`
- Rust backend commands under `src-tauri/src/`
- Tauri command registration in `src-tauri/src/lib.rs`
- User and contributor documentation under `docs/`
- Agent workflows under `.agents/skills/`

Read only the documentation sections relevant to the task; expand to related sections when the change affects their ownership or contracts.

- For runtime ownership or frontend/backend boundary changes, start with `Reading by Ownership` in `docs/development/ARCHITECTURE.md`, then read its selected sections. Expand when another owner or contract is affected; do not default to the whole document for a local change.
- Before code edits or Git operations, read `Branches and Commits` in `docs/development/DEVELOPMENT_GUIDE.md` and confirm the working branch; also read `Pull Request Checks` for publication. Read `Setup` only for dependency setup and `Updater Signing` only for updater/release signing changes.
- For validation, use the validation Skill below, including its optimized-runtime reference for protocol setup, cancellation, timeout, or Tauri async call-chain changes.

## Non-Negotiable Rules

- Do not clear, recreate, or reset terminal sessions as a side effect of settings changes or ordinary UI updates.
- Treat terminal buffers, session logs, connection targets, usernames, prompts, command output, and API keys as sensitive.
- Do not change log capture, log storage, API key storage, or secret handling without explicitly preserving privacy expectations.
- Keep human-facing documentation in `docs/`; keep task procedures and agent-only decision criteria in `.agents/skills/` or `AGENTS.md`, and custom agent definitions in `.codex/agents/`.

## Codebase Conventions

- Record user-facing features, fixes, behavior changes, and compatibility changes in `CHANGELOG.md` under `Unreleased` in the same change. Internal-only refactors, tests, documentation, and Skills normally need no entry; confirm the reason before completing the task.
- For release preparation, verify those notes under the target release heading and leave `Unreleased` empty; do not duplicate the moved notes in `Unreleased`.
- Keep Rust config structs in `src-tauri/src/config.rs` synchronized with TypeScript config types in `src/types/index.ts`.
- When adding or renaming Tauri commands, update both the Rust command implementation and the registration list in `src-tauri/src/lib.rs`.
- When frontend text changes, update both `src/locales/en.json` and `src/locales/ja.json`.
- Keep SSH, Serial, AI, config, logger, and known-host behavior in their existing backend modules unless a change clearly requires moving boundaries.
- Prefer narrow changes that preserve existing UI state, active tabs, terminal scrollback, and connection lifecycle.
- Windows is the primary beta target. Use Windows paths and behavior as the default unless the task says otherwise.

## Workflow Routing

- Use `.agents/skills/exaterm-session-lifecycle-change/SKILL.md` for connection/cancellation/finalization, session teardown, tab/window ownership, output restoration, or logging lifecycle changes. Pure styling and operating existing sessions use their respective Skills.
- For an independent lifecycle review, delegate to `exaterm-state-reviewer` in `.codex/agents/exaterm-state-reviewer.toml`; pass the intended transitions, comparison base or diff, and validation evidence. The lifecycle Skill defines review triggers and coordination with the contract reviewer.
- Use `.agents/skills/exaterm-external-control-change/SKILL.md` for CLI, MCP, or shared external-control implementation and contract changes. Use `skills/exaterm-cli/SKILL.md` to operate ExaTerm instead.
- For an independent external-control contract review, delegate to `exaterm-contract-reviewer` in `.codex/agents/exaterm-contract-reviewer.toml`; pass the intended behavior, comparison base or diff, and validation evidence. The development Skill defines when to request this review.
- Use `.agents/skills/exaterm-ui-change/SKILL.md` for React, CSS, layout, dialog, menu, design-token, or visual changes.
- For the existing ExaTerm UI, use the project UI Skill rather than generic design, Tailwind, or shadcn Skills. Use generic creative Skills only for separately requested assets or independent designs.
- Use `.agents/skills/exaterm-validate-change/SKILL.md` to choose and report validation commands.
- Use `.agents/skills/exaterm-release-prep/SKILL.md` for release version and changelog preparation.
- Use `.agents/skills/exaterm-publish-pr/SKILL.md` for PR publication; this repository procedure takes precedence over the user-level publication Skill.
