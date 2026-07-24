# ExaTerm AI Agent Guide

Keep user-facing documentation in `docs/`.

## Project Snapshot

ExaTerm is a Windows-focused Tauri v2 desktop app with:

- React + TypeScript frontend under `src/`
- Rust backend commands under `src-tauri/src/`
- Tauri command registration in `src-tauri/src/lib.rs`
- User-facing documentation under `docs/`
- AI-facing implementation notes under `ai-docs/`

Read `ai-docs/ARCHITECTURE.md` before changing behavior across frontend/backend boundaries.
Use `ai-docs/CHANGE_CHECKLIST.md` before and after non-trivial code changes.
Read `docs/DEVELOPMENT_GUIDE.en.md` before code changes or Git operations, especially branch creation, staging, committing, pushing, or opening pull requests.

## Non-Negotiable Rules

- Do not clear, recreate, or reset terminal sessions as a side effect of settings changes or ordinary UI updates.
- Treat terminal buffers, session logs, connection targets, usernames, prompts, command output, and API keys as sensitive.
- Do not change log capture, log storage, API key storage, or secret handling without explicitly preserving privacy expectations.
- Do not place AI-only implementation guidance in `docs/`; that folder is for user-facing documentation.

## Codebase Conventions

- Keep Rust config structs in `src-tauri/src/config.rs` synchronized with TypeScript config types in `src/types/index.ts`.
- When adding or renaming Tauri commands, update both the Rust command implementation and the registration list in `src-tauri/src/lib.rs`.
- When frontend text changes, update both `src/locales/en.json` and `src/locales/ja.json`.
- Keep SSH, Serial, AI, config, logger, and known-host behavior in their existing backend modules unless a change clearly requires moving boundaries.
- Prefer narrow changes that preserve existing UI state, active tabs, terminal scrollback, and connection lifecycle.
- Windows is the primary beta target. Use Windows paths and behavior as the default unless the task says otherwise.

## Validation Commands

When choosing validation or test commands, use `.agents/skills/exaterm-validate-change/SKILL.md`.
