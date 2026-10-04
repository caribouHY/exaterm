# ExaTerm AI Agent Guide

Keep human-facing project documentation in `docs/`. Keep agent execution procedures in repository-local Skills under `.agents/skills/`.

## Project Snapshot

ExaTerm is a Windows-focused Tauri v2 desktop app with:

- React + TypeScript frontend under `src/`
- Rust backend commands under `src-tauri/src/`
- Tauri command registration in `src-tauri/src/lib.rs`
- User and contributor documentation under `docs/`
- Agent workflows under `.agents/skills/`

Read only the documentation sections relevant to the task; expand to related sections when the change affects their ownership or contracts.

- For runtime ownership or frontend/backend boundary changes, read `Design Principles` and the affected sections of `docs/development/ARCHITECTURE.md`: connection attempts, workspace/windows, terminal sessions, logging, external control, or data storage.
- Before Git operations, read `Branches and Commits` in `docs/development/DEVELOPMENT_GUIDE.md`; also read `Pull Request Checks` for publication. Read `Setup` only for dependency setup and `Updater Signing` only for updater/release signing changes.
- For validation, use the validation Skill below. Protocol setup, cancellation, timeout, or Tauri async call-chain changes additionally require `Optimized Tauri Runtime Checks` in the development guide.

## Non-Negotiable Rules

- Do not clear, recreate, or reset terminal sessions as a side effect of settings changes or ordinary UI updates.
- Treat terminal buffers, session logs, connection targets, usernames, prompts, command output, and API keys as sensitive.
- Do not change log capture, log storage, API key storage, or secret handling without explicitly preserving privacy expectations.
- Keep human-facing documentation in `docs/`; keep task procedures and agent-only decision criteria in `.agents/skills/` or `AGENTS.md`.

## Codebase Conventions

- Record user-facing features, fixes, behavior changes, and compatibility changes in `CHANGELOG.md` under `Unreleased` in the same change. Internal-only refactors, tests, documentation, and Skills normally need no entry; confirm the reason before completing the task.
- Keep Rust config structs in `src-tauri/src/config.rs` synchronized with TypeScript config types in `src/types/index.ts`.
- When adding or renaming Tauri commands, update both the Rust command implementation and the registration list in `src-tauri/src/lib.rs`.
- When frontend text changes, update both `src/locales/en.json` and `src/locales/ja.json`.
- Keep SSH, Serial, AI, config, logger, and known-host behavior in their existing backend modules unless a change clearly requires moving boundaries.
- Prefer narrow changes that preserve existing UI state, active tabs, terminal scrollback, and connection lifecycle.
- Windows is the primary beta target. Use Windows paths and behavior as the default unless the task says otherwise.

## Workflow Routing

- Use `.agents/skills/exaterm-ui-change/SKILL.md` for React, CSS, layout, dialog, menu, design-token, or visual changes.
- For the existing ExaTerm UI, use the project UI Skill rather than generic design, Tailwind, or shadcn Skills. Use generic creative Skills only for separately requested assets or independent designs.
- Use `.agents/skills/exaterm-validate-change/SKILL.md` to choose and report validation commands.
- Use `.agents/skills/exaterm-release-prep/SKILL.md` for release version and changelog preparation.
- Use `.agents/skills/publish-exaterm-pr/SKILL.md` for PR publication; this repository procedure takes precedence over the user-level publication Skill.
