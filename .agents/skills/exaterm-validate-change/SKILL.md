---
name: exaterm-validate-change
description: Standardize ExaTerm validation command selection and reporting. Use when Codex needs to choose, run, or report tests, formatting checks, builds, or PR-ready validation for changes in the ExaTerm repository, including frontend, backend, Tauri, documentation, AI-facing guidance, and skill-only changes.
---

# ExaTerm Validate Change

## Overview

Use this skill to select the smallest correct validation set for ExaTerm changes and to avoid inconsistent commands. The skill is self-contained; do not rely on other project documents to choose validation commands.

Run commands from the repository root in Windows PowerShell unless a user explicitly directs otherwise.

## Non-Negotiable Command Rules

- Do not use `corepack`.
- Do not use `npm run ...`; use `pnpm run ...` for Node/package scripts.
- Do not use `cd src-tauri; cargo test` as the standard Rust validation command.
- Run Rust validation from the repository root with `--manifest-path`.
- Do not replace a failing standard command with a different command to make validation look green.
- Do not run a full application build for documentation-only, AI-guidance-only, or skill-only changes unless the user explicitly asks or the change affects application behavior.

## Standard Commands

Use these exact commands:

```powershell
pnpm run format
pnpm run build
pnpm run format:check
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
pnpm run tauri build --debug
```

## Select Validation By Change Type

| Change type | Required validation |
| --- | --- |
| Frontend, UI, React, TypeScript, CSS, locale text | `pnpm run format`, `pnpm run build`; before PR also run `pnpm run format:check` |
| Rust backend, Tauri command, config, logger, SSH, Serial, Telnet, AI provider | `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo test --manifest-path src-tauri/Cargo.toml` |
| Frontend plus backend boundary, shared payloads, Tauri invoke surface | Frontend validation plus Rust validation |
| Installer, sidecar, CLI binary, runtime packaging, Tauri bundle integration | Relevant frontend/Rust validation plus `pnpm run tauri build --debug` |
| Documentation-only, AI-facing guidance-only, skill-only | Run `pnpm run format` for changed Markdown/YAML when practical; no full app build by default; check paths, frontmatter, and exact content requirements |

## PR-Ready Validation

Before publishing or reporting a PR-ready change:

- Run `pnpm run format` first to apply project formatting to changed files.
- Always run `pnpm run format:check`.
- If the change touches frontend, TypeScript, UI, CSS, or locale files, run `pnpm run build`.
- If the change touches Rust, backend commands, Tauri command registration, config, logger, SSH, Serial, Telnet, AI provider logic, or Cargo files, run `cargo test --manifest-path src-tauri/Cargo.toml`.
- If installer/runtime packaging is in scope, run `pnpm run tauri build --debug`.
- For documentation-only, AI-guidance-only, or skill-only changes, do not run full app builds by default; verify the edited Markdown/frontmatter/path references instead.

## Failure Handling

When a standard command fails:

- Report the exact command that was run.
- Summarize the failing error or exit condition.
- Distinguish likely environment failures from code failures when there is evidence.
- Keep the failed command in the validation report; do not substitute a different command as if it were equivalent.
- If a retry requires permissions, network access, dependency installation, or another external state change, request it explicitly instead of silently changing commands.

## Reporting Format

Report validation results with one bullet per command:

```markdown
Validation:
- `pnpm run format:check`: passed
- `pnpm run build`: skipped, documentation-only change
- `cargo test --manifest-path src-tauri/Cargo.toml`: failed, <short reason>
```

Do not include secrets, terminal output content, connection targets, usernames, prompts, or API keys in validation reports.
